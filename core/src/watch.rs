use std::fs;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
};
use std::thread;
use std::time::Duration;

use crate::model::Score;
use crate::schedule::{LiveScheduler, ScheduledBar};
use crate::source::compile_and_validate_score;

pub struct ScoreWatcher {
    path: PathBuf,
    mute_track: Option<String>,
    last_source: String,
    first_bar: bool,
    staged_warning: Option<String>,
    background: Option<BackgroundPoller>,
    live: LiveScheduler,
}

impl ScoreWatcher {
    pub fn from_path(path: impl Into<PathBuf>, mute_track: Option<String>) -> Result<Self, String> {
        let path = path.into();
        let source =
            fs::read_to_string(&path).map_err(|error| format!("score read failed: {error}"))?;
        let score = compile_and_validate_score(&path, &source, mute_track.as_deref())?;
        Ok(Self {
            path,
            mute_track,
            last_source: source,
            first_bar: true,
            staged_warning: None,
            background: None,
            live: LiveScheduler::new(score),
        })
    }

    pub fn start_background_poller(&mut self, poll_interval: Duration) {
        if self.background.is_none() {
            self.background = Some(BackgroundPoller::start(
                self.path.clone(),
                self.mute_track.clone(),
                self.last_source.clone(),
                poll_interval,
            ));
        }
    }

    pub fn stage_reload(&mut self) {
        self.stage_reload_with(&mut |_| Ok(()));
    }

    fn stage_reload_with(&mut self, prepare: &mut dyn FnMut(&Score) -> Result<(), String>) {
        if let Some(background) = &self.background {
            let results: Vec<_> = background.receiver.try_iter().collect();
            for result in results {
                self.apply_reload_result(result, prepare);
            }
            return;
        }
        match fs::read_to_string(&self.path) {
            Ok(source) => {
                if source != self.last_source {
                    self.last_source = source;
                    let result = compile_and_validate_score(
                        &self.path,
                        &self.last_source,
                        self.mute_track.as_deref(),
                    );
                    self.apply_reload_result(result, prepare);
                }
            }
            Err(error) => {
                self.staged_warning = Some(format!("score read failed: {error}"));
            }
        }
    }

    fn apply_reload_result(
        &mut self,
        result: Result<Score, String>,
        prepare: &mut dyn FnMut(&Score) -> Result<(), String>,
    ) {
        let result = result.and_then(|score| {
            prepare(&score)?;
            Ok(score)
        });
        match result {
            Ok(score) => {
                self.live.apply_score(score);
                self.staged_warning = None;
            }
            Err(error) => {
                self.staged_warning = Some(error);
            }
        }
    }

    pub fn sample_assets(&self) -> Vec<crate::sample::SampleAsset> {
        self.live.sample_assets()
    }

    pub fn next_bar(&mut self) -> Result<(ScheduledBar, Option<String>), String> {
        self.next_bar_with(&mut |_| Ok(()))
    }

    pub fn next_bar_with(
        &mut self,
        prepare: &mut dyn FnMut(&Score) -> Result<(), String>,
    ) -> Result<(ScheduledBar, Option<String>), String> {
        let warning = if self.first_bar {
            prepare(self.live.score())?;
            self.first_bar = false;
            None
        } else {
            self.stage_reload_with(prepare);
            self.staged_warning.take()
        };
        let bar = self.live.next_bar()?;
        Ok((bar, warning))
    }
}

struct BackgroundPoller {
    receiver: Receiver<Result<Score, String>>,
    stop: Arc<AtomicBool>,
}

impl BackgroundPoller {
    fn start(
        path: PathBuf,
        mute_track: Option<String>,
        last_source: String,
        poll_interval: Duration,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let mut last_source = last_source;

        thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                thread::sleep(poll_interval);
                if thread_stop.load(Ordering::Relaxed) {
                    break;
                }
                let read_result = match fs::read_to_string(&path) {
                    Ok(text) if text.trim().is_empty() => {
                        thread::sleep(Duration::from_millis(15));
                        fs::read_to_string(&path)
                    }
                    other => other,
                };
                match read_result {
                    Ok(source) => {
                        if source != last_source {
                            last_source = source;
                            let result = compile_and_validate_score(
                                &path,
                                &last_source,
                                mute_track.as_deref(),
                            );
                            if tx.send(result).is_err() {
                                break;
                            }
                        }
                    }
                    Err(error) => {
                        if tx.send(Err(format!("score read failed: {error}"))).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        Self { receiver: rx, stop }
    }
}

impl Drop for BackgroundPoller {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = concat!(
        "{\"type\":\"tempo\",\"bpm\":120}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":36,\"beat\":0,\"duration_beats\":0.5,\"velocity\":0.8}\n"
    );

    struct ScoreFile(PathBuf);

    impl ScoreFile {
        fn new() -> Self {
            let file = Self(std::env::temp_dir().join(format!(
                "spinel_background_test_{}_{}.ndjson",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )));
            fs::write(&file.0, SOURCE).unwrap();
            file
        }
    }

    impl Drop for ScoreFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    #[test]
    fn background_worker_delivers_valid_scores_and_validation_errors() {
        let file = ScoreFile::new();
        let worker = BackgroundPoller::start(
            file.0.clone(),
            None,
            SOURCE.to_owned(),
            Duration::from_millis(10),
        );
        fs::write(&file.0, SOURCE.replace("\"note\":36", "\"note\":48")).unwrap();
        let score = worker
            .receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("background worker did not deliver the edit")
            .unwrap();
        assert_eq!(score.events[0].note, 48);

        fs::write(&file.0, "invalid JSON").unwrap();
        let result = worker
            .receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("background worker did not report the invalid edit");
        assert!(result.unwrap_err().contains("invalid JSON"));
    }

    #[test]
    fn queued_updates_keep_the_latest_valid_score_and_report_a_later_error() {
        let file = ScoreFile::new();
        let mut watcher = ScoreWatcher::from_path(&file.0, None).unwrap();
        watcher.next_bar().unwrap();
        let (sender, receiver) = mpsc::channel();
        watcher.background = Some(BackgroundPoller {
            receiver,
            stop: Arc::new(AtomicBool::new(false)),
        });
        let updated = Score::parse_ndjson(&SOURCE.replace("\"note\":36", "\"note\":48")).unwrap();
        sender.send(Ok(updated)).unwrap();
        sender.send(Err("invalid edit".to_owned())).unwrap();

        let (bar, warning) = watcher.next_bar().unwrap();
        assert_eq!(bar.start, Duration::from_secs(2));
        assert_eq!(
            bar.notes[0].frequency,
            crate::schedule::midi_to_frequency(48)
        );
        assert_eq!(warning.as_deref(), Some("invalid edit"));
        let (next, warning) = watcher.next_bar().unwrap();
        assert_eq!(next.notes[0].frequency, bar.notes[0].frequency);
        assert!(warning.is_none());
    }
}
