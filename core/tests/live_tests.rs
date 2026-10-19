use std::process::Command;
use std::time::Duration;

use spinel_sonic_lab::model::{BassTone, Oscillator, Score};
use spinel_sonic_lab::schedule::{LiveScheduler, midi_to_frequency};

use spinel_sonic_lab::watch::ScoreWatcher;

const SCORE: &str = "{\"type\":\"tempo\",\"bpm\":120}\n\
{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":36,\"beat\":0.0,\"duration_beats\":0.5,\"velocity\":0.8}\n";

struct ScoreFile(std::path::PathBuf);

impl ScoreFile {
    fn new(source: &str) -> Self {
        Self::in_directory(&std::env::temp_dir(), "ndjson", source)
    }

    fn in_directory(directory: &std::path::Path, extension: &str, source: &str) -> Self {
        let path = directory.join(format!(
            "spinel_reload_test_{}_{}.{extension}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, source).unwrap();
        Self(path)
    }

    fn write(&self, source: &str) {
        std::fs::write(&self.0, source).unwrap();
    }
}

impl Drop for ScoreFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn staged_valid_edit_survives_a_later_invalid_edit_before_the_boundary() {
    let file = ScoreFile::new(SCORE);
    let mut watcher = ScoreWatcher::from_path(&file.0, None).unwrap();
    watcher.next_bar().unwrap();
    file.write(&SCORE.replace("\"note\":36", "\"note\":48"));
    watcher.stage_reload();
    file.write("invalid JSON");
    watcher.stage_reload();

    let (bar, warning) = watcher.next_bar().unwrap();
    assert_eq!(bar.start, Duration::from_secs(2));
    assert_eq!(bar.notes[0].frequency, midi_to_frequency(48));
    assert!(warning.is_some());

    let (_, warning) = watcher.next_bar().unwrap();
    assert!(warning.is_none());
}

#[test]
fn tempo_edit_keeps_the_boundary_and_changes_the_following_bar_length() {
    let mut live = LiveScheduler::new(Score::parse_ndjson(SCORE).unwrap());
    live.next_bar().unwrap();
    live.reload_ndjson(&SCORE.replace("120", "60"), None)
        .unwrap();
    let bar = live.next_bar().unwrap();
    assert_eq!(bar.start, Duration::from_secs(2));
    assert_eq!(bar.duration, Duration::from_secs(4));
    assert_eq!(bar.notes[0].duration_seconds, 0.5);
    assert_eq!(live.next_bar().unwrap().start, Duration::from_secs(6));
}

#[test]
fn live_reload_switches_on_next_bar_and_keeps_current_score_on_error() {
    let initial = Score::parse_ndjson(SCORE).unwrap();
    let mut live = LiveScheduler::new(initial);

    // Bar 0 (0.0s .. 2.0s at 120 BPM): plays the initial C2 (MIDI 36) at 0.0s.
    let bar0 = live.next_bar().unwrap();
    assert_eq!(bar0.start, Duration::ZERO);
    assert_eq!(bar0.duration, Duration::from_secs(2));
    assert_eq!(bar0.notes.len(), 1);
    assert_eq!(bar0.notes[0].at, Duration::ZERO);
    assert_eq!(bar0.notes[0].tone, BassTone::default());
    assert!((bar0.notes[0].frequency - midi_to_frequency(36)).abs() < 0.001);

    // Reload with a valid brighter score before Bar 1; it takes effect at the next bar boundary (2.0s).
    let updated_source = concat!(
        "{\"type\":\"tempo\",\"bpm\":120}\n",
        "{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"oscillator\":\"sine\",\"cutoff_hz\":1200.0,\"attack_seconds\":0.01,\"release_seconds\":0.05,\"gain\":1.5}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":48,\"beat\":0.0,\"duration_beats\":0.5,\"velocity\":0.8}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":55,\"beat\":1.0,\"duration_beats\":0.5,\"velocity\":0.8}\n"
    );
    live.reload_ndjson(updated_source, None).unwrap();

    let bar1 = live.next_bar().unwrap();
    assert_eq!(bar1.start, Duration::from_secs(2));
    assert_eq!(bar1.duration, Duration::from_secs(2));
    assert_eq!(bar1.notes[0].tone.oscillator, Oscillator::Sine);
    assert_eq!(bar1.notes[0].tone.gain, 1.5);
    assert_eq!(
        bar1.notes.iter().map(|note| note.at).collect::<Vec<_>>(),
        vec![Duration::from_secs(2), Duration::from_millis(2500)]
    );
    assert!((bar1.notes[0].frequency - midi_to_frequency(48)).abs() < 0.001);
    assert!((bar1.notes[1].frequency - midi_to_frequency(55)).abs() < 0.001);

    // Reload with an invalid score before Bar 2; the error is returned and the previous valid score continues at 4.0s.
    assert!(
        live.reload_ndjson("{\"type\":\"tempo\",\"bpm\":0}\n", None)
            .is_err()
    );
    assert!(live.reload_ndjson("not json\n", None).is_err());

    let bar2 = live.next_bar().unwrap();
    assert_eq!(bar2.start, Duration::from_secs(4));
    assert_eq!(bar2.duration, Duration::from_secs(2));
    assert_eq!(bar2.notes[0].tone.oscillator, Oscillator::Sine);
    assert_eq!(
        bar2.notes.iter().map(|note| note.at).collect::<Vec<_>>(),
        vec![Duration::from_secs(4), Duration::from_millis(4500)]
    );
}

#[test]
fn live_reload_preserves_muted_track_and_switches_multi_bar_score_at_bar_boundary() {
    let two_bar_source = concat!(
        "{\"type\":\"tempo\",\"bpm\":120}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"bass\",\"note\":36,\"beat\":0.0,\"duration_beats\":0.5,\"velocity\":0.8}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"lead\",\"note\":60,\"beat\":0.0,\"duration_beats\":0.5,\"velocity\":0.8}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"bass\",\"note\":43,\"beat\":4.5,\"duration_beats\":0.5,\"velocity\":0.8}\n"
    );
    let initial = Score::parse_ndjson(two_bar_source)
        .unwrap()
        .without_track("lead")
        .unwrap();
    let mut live = LiveScheduler::new(initial);

    // Bar 0 (beats 0..4, 0.0s..2.0s): only note 36 at 0.0s (lead is muted, note 43 belongs to Bar 1).
    let bar0 = live.next_bar().unwrap();
    assert_eq!(bar0.notes.len(), 1);
    assert_eq!(bar0.notes[0].at, Duration::ZERO);
    assert!((bar0.notes[0].frequency - midi_to_frequency(36)).abs() < 0.001);

    // Without reload, Bar 1 (beats 4..8, 2.0s..4.0s) plays note 43 at beat 4.5 -> 2.25s.
    let bar1 = live.next_bar().unwrap();
    assert_eq!(bar1.start, Duration::from_secs(2));
    assert_eq!(bar1.notes.len(), 1);
    assert_eq!(bar1.notes[0].at, Duration::from_millis(2250));
    assert!((bar1.notes[0].frequency - midi_to_frequency(43)).abs() < 0.001);

    // Reloading a score where muting "lead" leaves no notes fails and keeps the two-bar score wrapping to Bar 0 at 4.0s.
    let only_lead = concat!(
        "{\"type\":\"tempo\",\"bpm\":120}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"lead\",\"note\":64,\"beat\":0.0,\"duration_beats\":0.5,\"velocity\":0.8}\n"
    );
    assert!(live.reload_ndjson(only_lead, Some("lead")).is_err());

    let bar2 = live.next_bar().unwrap();
    assert_eq!(bar2.start, Duration::from_secs(4));
    assert_eq!(bar2.notes.len(), 1);
    assert_eq!(bar2.notes[0].at, Duration::from_secs(4));
    assert!((bar2.notes[0].frequency - midi_to_frequency(36)).abs() < 0.001);
}

#[test]
fn score_watcher_reloads_modified_file_on_next_bar_and_keeps_score_when_file_is_invalid() {
    let file = ScoreFile::new(SCORE);
    let mut watcher = ScoreWatcher::from_path(&file.0, None).unwrap();

    let (bar0, warning0) = watcher.next_bar().unwrap();
    assert!(warning0.is_none());
    assert_eq!(bar0.start, Duration::ZERO);
    assert!((bar0.notes[0].frequency - midi_to_frequency(36)).abs() < 0.001);

    // Edit the file on disk with a valid new pitch (C3 = 48).
    let next_score = SCORE.replace("\"note\":36", "\"note\":48");
    file.write(&next_score);

    let (bar1, warning1) = watcher.next_bar().unwrap();
    assert!(warning1.is_none());
    assert_eq!(bar1.start, Duration::from_secs(2));
    assert_eq!(bar1.notes[0].at, Duration::from_secs(2));
    assert!((bar1.notes[0].frequency - midi_to_frequency(48)).abs() < 0.001);

    // Edit the file on disk with invalid content; Bar 2 keeps MIDI 48 and surfaces the warning.
    file.write("{\"type\":\"tempo\",\"bpm\":-10}\n");

    let (bar2, warning2) = watcher.next_bar().unwrap();
    assert!(warning2.is_some());
    assert_eq!(bar2.start, Duration::from_secs(4));
    assert_eq!(bar2.notes[0].at, Duration::from_secs(4));
    assert!((bar2.notes[0].frequency - midi_to_frequency(48)).abs() < 0.001);
}

#[test]
fn cli_watch_dry_run_loops_bars_and_rejects_stdin() {
    let output = Command::new(env!("CARGO_BIN_EXE_spinel-sonic-lab"))
        .args([
            "--dry-run",
            "--watch",
            "--bars",
            "2",
            "../composer/examples/bass.ndjson",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<_> = stdout.lines().collect();
    assert_eq!(lines.len(), 8);
    assert!(lines[0].starts_with("0.000s "));
    assert!(lines[4].starts_with("2.000s "));

    let bad_stdin = Command::new(env!("CARGO_BIN_EXE_spinel-sonic-lab"))
        .args(["--dry-run", "--watch", "-"])
        .output()
        .unwrap();
    assert!(!bad_stdin.status.success());
}

#[test]
fn score_watcher_compiles_spinel_rb_and_keeps_playing_when_edit_fails() {
    let initial_rb = concat!(
        "require_relative \"../lib/score_writer\"\n",
        "score = ScoreWriter.new\n",
        "score.tempo(120)\n",
        "score.note(\"C2\", 0.0, 0.5, 0.8)\n"
    );
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../composer/bin");
    let file = ScoreFile::in_directory(&directory, "rb", initial_rb);

    let mut watcher = ScoreWatcher::from_path(&file.0, None).unwrap();
    let (bar0, warning0) = watcher.next_bar().unwrap();
    assert!(warning0.is_none());
    assert_eq!(bar0.start, Duration::ZERO);
    assert_eq!(bar0.notes[0].tone, BassTone::default());
    assert!((bar0.notes[0].frequency - midi_to_frequency(36)).abs() < 0.001);

    // Edit the Spinel .rb source with a new preset and pitch; stage and switch at the next bar boundary (2.0s).
    let updated_rb = concat!(
        "require_relative \"../lib/score_writer\"\n",
        "score = ScoreWriter.new\n",
        "score.tempo(120)\n",
        "score.preset(\"soft\")\n",
        "score.note(\"C3\", 0.0, 0.5, 0.8)\n"
    );
    file.write(updated_rb);
    watcher.stage_reload();

    let (bar1, warning1) = watcher.next_bar().unwrap();
    assert!(warning1.is_none());
    assert_eq!(bar1.start, Duration::from_secs(2));
    assert_eq!(bar1.notes[0].tone.oscillator, Oscillator::Sine);
    assert_eq!(bar1.notes[0].at, Duration::from_secs(2));
    assert!((bar1.notes[0].frequency - midi_to_frequency(48)).abs() < 0.001);

    // Edit the Spinel .rb source with an invalid pitch; Bar 2 keeps the previous valid score and reports a warning.
    let broken_rb = concat!(
        "require_relative \"../lib/score_writer\"\n",
        "score = ScoreWriter.new\n",
        "score.tempo(120)\n",
        "score.note(\"H4\", 0.0, 0.5, 0.8)\n"
    );
    file.write(broken_rb);

    let (bar2, warning2) = watcher.next_bar().unwrap();
    assert!(warning2.is_some());
    assert_eq!(bar2.start, Duration::from_secs(4));
    assert_eq!(bar2.notes[0].tone.oscillator, Oscillator::Sine);
    assert!((bar2.notes[0].frequency - midi_to_frequency(48)).abs() < 0.001);
}
