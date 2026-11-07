use std::fs;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
};
use std::thread;
use std::time::Duration;

use crate::model::Score;
use crate::schedule::{LiveScheduler, ScheduledBar};
use crate::source::compile_and_validate_score;

type Prepare<'a> = dyn FnMut(&Score) -> Result<(), String> + 'a;
type PollSender = Sender<Result<Score, String>>;

/// The watched score file and the track muted on every compile.
#[derive(Clone)]
struct WatchedFile {
    path: PathBuf,
    mute_track: Option<String>,
}

impl WatchedFile {
    fn compile(&self, source: &str) -> Result<Score, String> {
        compile_and_validate_score(&self.path, source, self.mute_track.as_deref())
    }
}

pub struct ScoreWatcher {
    file: WatchedFile,
    last_source: String,
    first_bar: bool,
    staged_warning: Option<String>,
    background: Option<BackgroundPoller>,
    live: LiveScheduler,
}

impl ScoreWatcher {
    pub fn from_path(path: impl Into<PathBuf>, mute_track: Option<String>) -> Result<Self, String> {
        let file = WatchedFile { path: path.into(), mute_track };
        let source = fs::read_to_string(&file.path)
            .map_err(|error| format!("score read failed: {error}"))?;
        let score = file.compile(&source)?;
        Ok(Self::from_score(file, source, score))
    }

    fn from_score(file: WatchedFile, last_source: String, score: Score) -> Self {
        let live = LiveScheduler::new(score);
        Self { file, last_source, first_bar: true, staged_warning: None, background: None, live }
    }

    pub fn start_background_poller(&mut self, poll_interval: Duration) {
        if self.background.is_none() {
            self.background = Some(BackgroundPoller::start(self.poll_request(poll_interval)));
        }
    }

    fn poll_request(&self, poll_interval: Duration) -> PollRequest {
        PollRequest {
            file: self.file.clone(),
            last_source: self.last_source.clone(),
            poll_interval,
        }
    }

    pub fn stage_reload(&mut self) {
        self.stage_reload_with(&mut |_| Ok(()));
    }

    fn stage_reload_with(&mut self, prepare: &mut Prepare<'_>) {
        if let Some(background) = &self.background {
            let results: Vec<_> = background.receiver.try_iter().collect();
            for result in results {
                self.apply_reload_result(result, prepare);
            }
            return;
        }
        self.stage_file_reload(prepare);
    }

    fn stage_file_reload(&mut self, prepare: &mut Prepare<'_>) {
        match fs::read_to_string(&self.file.path) {
            Ok(source) if source != self.last_source => self.stage_source(source, prepare),
            Ok(_) => {}
            Err(error) => self.staged_warning = Some(format!("score read failed: {error}")),
        }
    }

    fn stage_source(&mut self, source: String, prepare: &mut Prepare<'_>) {
        self.last_source = source;
        let result = self.file.compile(&self.last_source);
        self.apply_reload_result(result, prepare);
    }

    fn apply_reload_result(&mut self, result: Result<Score, String>, prepare: &mut Prepare<'_>) {
        let result = result.and_then(|score| {
            prepare(&score)?;
            Ok(score)
        });
        self.commit_reload_result(result);
    }

    fn commit_reload_result(&mut self, result: Result<Score, String>) {
        match result {
            Ok(score) => {
                self.live.apply_score(score);
                self.staged_warning = None;
            }
            Err(error) => self.staged_warning = Some(error),
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
        prepare: &mut Prepare<'_>,
    ) -> Result<(ScheduledBar, Option<String>), String> {
        let warning = self.prepare_next_bar(prepare)?;
        let bar = self.live.next_bar()?;
        Ok((bar, warning))
    }

    fn prepare_next_bar(&mut self, prepare: &mut Prepare<'_>) -> Result<Option<String>, String> {
        if self.first_bar {
            prepare(self.live.score())?;
            self.first_bar = false;
            Ok(None)
        } else {
            self.stage_reload_with(prepare);
            Ok(self.staged_warning.take())
        }
    }
}

struct BackgroundPoller {
    receiver: Receiver<Result<Score, String>>,
    stop: Arc<AtomicBool>,
}

struct PollRequest {
    file: WatchedFile,
    last_source: String,
    poll_interval: Duration,
}

impl BackgroundPoller {
    fn start(request: PollRequest) -> Self {
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        thread::spawn(move || request.run(tx, thread_stop));
        Self { receiver: rx, stop }
    }
}

impl PollRequest {
    fn run(mut self, tx: PollSender, stop: Arc<AtomicBool>) {
        while !stop.load(Ordering::Relaxed) {
            if !self.poll_and_send(&tx, &stop) {
                break;
            }
        }
    }

    fn poll_and_send(&mut self, tx: &PollSender, stop: &AtomicBool) -> bool {
        thread::sleep(self.poll_interval);
        if stop.load(Ordering::Relaxed) {
            return false;
        }
        match self.poll() {
            Some(result) => tx.send(result).is_ok(),
            None => true,
        }
    }

    fn poll(&mut self) -> Option<Result<Score, String>> {
        match self.read_source() {
            Ok(source) if source != self.last_source => Some(self.compile_source(source)),
            Ok(_) => None,
            Err(error) => Some(Err(format!("score read failed: {error}"))),
        }
    }

    fn compile_source(&mut self, source: String) -> Result<Score, String> {
        self.last_source = source;
        self.file.compile(&self.last_source)
    }

    fn read_source(&self) -> std::io::Result<String> {
        match fs::read_to_string(&self.file.path) {
            Ok(text) if text.trim().is_empty() => {
                thread::sleep(Duration::from_millis(15));
                fs::read_to_string(&self.file.path)
            }
            other => other,
        }
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
    use crate::schedule::midi_to_frequency;
    use std::time::{SystemTime, UNIX_EPOCH};

    const SOURCE: &str = concat!(
        "{\"type\":\"tempo\",\"bpm\":120}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":36,\"beat\":0,\"duration_beats\":0.5,\"velocity\":0.8}\n"
    );

    struct ScoreFile(PathBuf);

    impl ScoreFile {
        fn new() -> Self {
            let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let name = format!("spinel_background_test_{}_{nanos}.ndjson", std::process::id());
            let file = Self(std::env::temp_dir().join(name));
            fs::write(&file.0, SOURCE).unwrap();
            file
        }
    }

    impl Drop for ScoreFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    fn edited_source() -> String {
        SOURCE.replace("\"note\":36", "\"note\":48")
    }

    fn start_worker(file: &ScoreFile) -> BackgroundPoller {
        let file = WatchedFile { path: file.0.clone(), mute_track: None };
        let poll_interval = Duration::from_millis(10);
        BackgroundPoller::start(PollRequest { file, last_source: SOURCE.to_owned(), poll_interval })
    }

    /// Waits for the worker's next result with a bounded timeout, never a fixed sleep.
    fn next_result(worker: &BackgroundPoller) -> Result<Score, String> {
        let result = worker.receiver.recv_timeout(Duration::from_secs(2));
        result.expect("background worker did not report the edit")
    }

    #[test]
    fn background_worker_delivers_valid_scores_and_validation_errors() {
        let file = ScoreFile::new();
        let worker = start_worker(&file);
        fs::write(&file.0, edited_source()).unwrap();
        assert_eq!(next_result(&worker).unwrap().events[0].note, 48);
        fs::write(&file.0, "invalid JSON").unwrap();
        assert!(next_result(&worker).unwrap_err().contains("invalid JSON"));
    }

    /// Plays the first bar, then queues a valid edit followed by an invalid one.
    fn watcher_with_queued_edit_and_error(file: &ScoreFile) -> ScoreWatcher {
        let mut watcher = ScoreWatcher::from_path(&file.0, None).unwrap();
        watcher.next_bar().unwrap();
        let (sender, receiver) = mpsc::channel();
        watcher.background = Some(BackgroundPoller { receiver, stop: Arc::default() });
        sender.send(Ok(Score::parse_ndjson(&edited_source()).unwrap())).unwrap();
        sender.send(Err("invalid edit".to_owned())).unwrap();
        watcher
    }

    #[test]
    fn queued_updates_keep_the_latest_valid_score_and_report_a_later_error() {
        let file = ScoreFile::new();
        let mut watcher = watcher_with_queued_edit_and_error(&file);
        let (bar, warning) = watcher.next_bar().unwrap();
        assert_eq!(bar.start, Duration::from_secs(2));
        assert_eq!(bar.notes[0].frequency, midi_to_frequency(48));
        assert_eq!(warning.as_deref(), Some("invalid edit"));
    }

    #[test]
    fn a_reported_reload_error_keeps_the_score_and_is_not_repeated() {
        let file = ScoreFile::new();
        let mut watcher = watcher_with_queued_edit_and_error(&file);
        watcher.next_bar().unwrap();
        let (next, warning) = watcher.next_bar().unwrap();
        assert_eq!(next.notes[0].frequency, midi_to_frequency(48));
        assert!(warning.is_none());
    }
}
