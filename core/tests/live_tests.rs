mod common;

use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use common::parse;
use spinel_sonic_lab::model::{BassTone, Oscillator, Score};
use spinel_sonic_lab::schedule::{LiveScheduler, ScheduledBar, midi_to_frequency};
use spinel_sonic_lab::watch::ScoreWatcher;

const SCORE: &str = "{\"type\":\"tempo\",\"bpm\":120}\n\
{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":36,\"beat\":0.0,\"duration_beats\":0.5,\"velocity\":0.8}\n";

fn secs(seconds: u64) -> Duration {
    Duration::from_secs(seconds)
}

fn hz(midi: u8) -> f32 {
    midi_to_frequency(midi)
}

/// Each note's start in milliseconds and its frequency.
fn timeline(bar: &ScheduledBar) -> Vec<(u128, f32)> {
    bar.notes.iter().map(|note| (note.at.as_millis(), note.frequency)).collect()
}

#[test]
fn declared_four_bar_loop_keeps_silent_bar_time() {
    let source = SCORE.replace("\"bpm\":120", "\"bpm\":120,\"bars\":4");
    let mut live = LiveScheduler::new(parse(&source));
    let counts: Vec<_> = (0..5).map(|_| live.next_bar().unwrap().notes.len()).collect();
    assert_eq!(counts, vec![1, 0, 0, 0, 1]);
}

const FOUR_BAR_KICK_BEATS: [f64; 16] =
    [0.0, 1.0, 2.0, 2.75, 4.0, 5.0, 6.0, 6.75, 8.0, 9.0, 10.0, 10.75, 12.0, 13.0, 14.0, 14.75];

#[test]
fn spinel_four_bar_kick_fixture_has_expected_beat_positions() {
    let score = parse(include_str!("../../composer/examples/four_bar_kick.ndjson"));
    assert_eq!(score.loop_bars, Some(4));
    assert!(score.sample_events.iter().all(|event| event.track == "kick"));
    let beats: Vec<_> = score.sample_events.iter().map(|event| event.beat).collect();
    assert_eq!(beats, FOUR_BAR_KICK_BEATS);
}

#[test]
fn declared_loop_rejects_invalid_lengths() {
    for bars in ["0", "-1", "1.5", "\"4\""] {
        let source = SCORE.replace("\"bpm\":120", &format!("\"bpm\":120,\"bars\":{bars}"));
        assert!(Score::parse_ndjson(&source).is_err(), "accepted bars={bars}");
    }
}

#[test]
fn declared_loop_rejects_events_after_its_last_bar() {
    let looped = SCORE.replace("\"bpm\":120", "\"bpm\":120,\"bars\":4");
    let source = looped.replace("\"beat\":0.0", "\"beat\":16.0");
    assert!(Score::parse_ndjson(&source).is_err());
}

struct ScoreFile(std::path::PathBuf);

impl ScoreFile {
    fn new(source: &str) -> Self {
        Self::in_directory(&std::env::temp_dir(), "ndjson", source)
    }

    fn in_directory(directory: &Path, extension: &str, source: &str) -> Self {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let name = format!("spinel_reload_test_{}_{nanos}.{extension}", std::process::id());
        let file = Self(directory.join(name));
        file.write(source);
        file
    }

    fn write(&self, source: &str) {
        std::fs::write(&self.0, source).unwrap();
    }

    fn watcher(&self) -> ScoreWatcher {
        ScoreWatcher::from_path(&self.0, None).unwrap()
    }
}

impl Drop for ScoreFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Plays bar 0, then stages a valid edit followed by an invalid one before bar 1.
fn watcher_with_staged_edits(file: &ScoreFile) -> ScoreWatcher {
    let mut watcher = file.watcher();
    watcher.next_bar().unwrap();
    file.write(&SCORE.replace("\"note\":36", "\"note\":48"));
    watcher.stage_reload();
    file.write("invalid JSON");
    watcher.stage_reload();
    watcher
}

#[test]
fn staged_valid_edit_survives_a_later_invalid_edit_before_the_boundary() {
    let file = ScoreFile::new(SCORE);
    let (bar, warning) = watcher_with_staged_edits(&file).next_bar().unwrap();
    assert_eq!(bar.start, secs(2));
    assert_eq!(timeline(&bar), [(2000, hz(48))]);
    assert!(warning.is_some());
}

#[test]
fn staged_invalid_edit_is_reported_once() {
    let file = ScoreFile::new(SCORE);
    let mut watcher = watcher_with_staged_edits(&file);
    watcher.next_bar().unwrap();
    assert!(watcher.next_bar().unwrap().1.is_none());
}

#[test]
fn tempo_edit_keeps_the_boundary_and_changes_the_following_bar_length() {
    let mut live = LiveScheduler::new(parse(SCORE));
    live.next_bar().unwrap();
    live.reload_ndjson(&SCORE.replace("120", "60"), None).unwrap();
    let bar = live.next_bar().unwrap();
    assert_eq!((bar.start, bar.duration), (secs(2), secs(4)));
    assert_eq!(bar.notes[0].duration_seconds, 0.5);
    assert_eq!(live.next_bar().unwrap().start, secs(6));
}

const BRIGHTER_SCORE: &str = concat!(
    "{\"type\":\"tempo\",\"bpm\":120}\n",
    "{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"oscillator\":\"sine\",\"cutoff_hz\":1200.0,\"attack_seconds\":0.01,\"release_seconds\":0.05,\"gain\":1.5}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":48,\"beat\":0.0,\"duration_beats\":0.5,\"velocity\":0.8}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":55,\"beat\":1.0,\"duration_beats\":0.5,\"velocity\":0.8}\n"
);

/// Plays bar 0 (0.0s..2.0s at 120 BPM), then reloads a brighter two-note score.
fn live_after_valid_reload() -> LiveScheduler {
    let mut live = LiveScheduler::new(parse(SCORE));
    live.next_bar().unwrap();
    live.reload_ndjson(BRIGHTER_SCORE, None).unwrap();
    live
}

#[test]
fn live_bar_zero_plays_the_initial_score() {
    let bar0 = LiveScheduler::new(parse(SCORE)).next_bar().unwrap();
    assert_eq!((bar0.start, bar0.duration), (Duration::ZERO, secs(2)));
    assert_eq!(timeline(&bar0), [(0, hz(36))]);
    assert_eq!(bar0.notes[0].tone, BassTone::default());
}

#[test]
fn live_reload_switches_at_the_next_bar_boundary() {
    let bar1 = live_after_valid_reload().next_bar().unwrap();
    assert_eq!((bar1.start, bar1.duration), (secs(2), secs(2)));
    assert_eq!(bar1.notes[0].tone.oscillator, Oscillator::Sine);
    assert_eq!(bar1.notes[0].tone.gain, 1.5);
    assert_eq!(timeline(&bar1), [(2000, hz(48)), (2500, hz(55))]);
}

#[test]
fn invalid_live_reload_keeps_the_current_score() {
    let mut live = live_after_valid_reload();
    live.next_bar().unwrap();
    assert!(live.reload_ndjson("{\"type\":\"tempo\",\"bpm\":0}\n", None).is_err());
    assert!(live.reload_ndjson("not json\n", None).is_err());
    let bar2 = live.next_bar().unwrap();
    assert_eq!((bar2.start, bar2.duration), (secs(4), secs(2)));
    assert_eq!(bar2.notes[0].tone.oscillator, Oscillator::Sine);
    assert_eq!(timeline(&bar2), [(4000, hz(48)), (4500, hz(55))]);
}

const TWO_BAR_SCORE: &str = concat!(
    "{\"type\":\"tempo\",\"bpm\":120}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"bass\",\"note\":36,\"beat\":0.0,\"duration_beats\":0.5,\"velocity\":0.8}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"lead\",\"note\":60,\"beat\":0.0,\"duration_beats\":0.5,\"velocity\":0.8}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"bass\",\"note\":43,\"beat\":4.5,\"duration_beats\":0.5,\"velocity\":0.8}\n"
);
const ONLY_LEAD: &str = concat!(
    "{\"type\":\"tempo\",\"bpm\":120}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"lead\",\"note\":64,\"beat\":0.0,\"duration_beats\":0.5,\"velocity\":0.8}\n"
);

fn two_bar_live_without_lead() -> LiveScheduler {
    LiveScheduler::new(parse(TWO_BAR_SCORE).without_track("lead").unwrap())
}

#[test]
fn muted_lead_leaves_only_the_first_bass_note_in_bar_zero() {
    let bar0 = two_bar_live_without_lead().next_bar().unwrap();
    assert_eq!(timeline(&bar0), [(0, hz(36))]);
}

#[test]
fn second_bar_plays_its_own_notes_at_absolute_times() {
    let mut live = two_bar_live_without_lead();
    live.next_bar().unwrap();
    let bar1 = live.next_bar().unwrap();
    // Beat 4.5 at 120 BPM is 2.25s.
    assert_eq!(bar1.start, secs(2));
    assert_eq!(timeline(&bar1), [(2250, hz(43))]);
}

#[test]
fn reload_leaving_no_notes_after_mute_fails_and_the_loop_wraps() {
    let mut live = two_bar_live_without_lead();
    live.next_bar().unwrap();
    live.next_bar().unwrap();
    assert!(live.reload_ndjson(ONLY_LEAD, Some("lead")).is_err());
    let bar2 = live.next_bar().unwrap();
    assert_eq!(bar2.start, secs(4));
    assert_eq!(timeline(&bar2), [(4000, hz(36))]);
}

#[test]
fn score_watcher_plays_the_file_from_bar_zero() {
    let file = ScoreFile::new(SCORE);
    let (bar0, warning) = file.watcher().next_bar().unwrap();
    assert!(warning.is_none());
    assert_eq!(bar0.start, Duration::ZERO);
    assert_eq!(timeline(&bar0), [(0, hz(36))]);
}

/// Plays bar 0, then edits the file to C3 (MIDI 48).
fn watcher_after_pitch_edit(file: &ScoreFile) -> ScoreWatcher {
    let mut watcher = file.watcher();
    watcher.next_bar().unwrap();
    file.write(&SCORE.replace("\"note\":36", "\"note\":48"));
    watcher
}

#[test]
fn score_watcher_reloads_a_modified_file_on_the_next_bar() {
    let file = ScoreFile::new(SCORE);
    let (bar1, warning) = watcher_after_pitch_edit(&file).next_bar().unwrap();
    assert!(warning.is_none());
    assert_eq!(bar1.start, secs(2));
    assert_eq!(timeline(&bar1), [(2000, hz(48))]);
}

#[test]
fn score_watcher_keeps_the_score_and_warns_when_the_file_is_invalid() {
    let file = ScoreFile::new(SCORE);
    let mut watcher = watcher_after_pitch_edit(&file);
    watcher.next_bar().unwrap();
    file.write("{\"type\":\"tempo\",\"bpm\":-10}\n");
    let (bar2, warning) = watcher.next_bar().unwrap();
    assert!(warning.is_some());
    assert_eq!(bar2.start, secs(4));
    assert_eq!(timeline(&bar2), [(4000, hz(48))]);
}

fn run_cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_spinel-sonic-lab")).args(args).output().unwrap()
}

#[test]
fn cli_watch_dry_run_loops_bars() {
    let output =
        run_cli(&["--dry-run", "--watch", "--bars", "2", "../composer/examples/bass.ndjson"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<_> = stdout.lines().collect();
    assert_eq!(lines.len(), 8);
    assert!(lines[0].starts_with("0.000s ") && lines[4].starts_with("2.000s "), "{lines:?}");
}

#[test]
fn cli_watch_rejects_stdin() {
    assert!(!run_cli(&["--dry-run", "--watch", "-"]).status.success());
}

const INITIAL_RB: &str = concat!(
    "require_relative \"../lib/score_writer\"\n",
    "score = ScoreWriter.new\n",
    "score.tempo(120)\n",
    "score.note(\"C2\", 0.0, 0.5, 0.8)\n"
);
const SOFT_C3_RB: &str = concat!(
    "require_relative \"../lib/score_writer\"\n",
    "score = ScoreWriter.new\n",
    "score.tempo(120)\n",
    "score.preset(\"soft\")\n",
    "score.note(\"C3\", 0.0, 0.5, 0.8)\n"
);
const INVALID_PITCH_RB: &str = concat!(
    "require_relative \"../lib/score_writer\"\n",
    "score = ScoreWriter.new\n",
    "score.tempo(120)\n",
    "score.note(\"H4\", 0.0, 0.5, 0.8)\n"
);

/// A Spinel source in `composer/bin`, so its `require_relative` resolves.
fn spinel_score_file() -> ScoreFile {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../composer/bin");
    ScoreFile::in_directory(&directory, "rb", INITIAL_RB)
}

#[test]
fn score_watcher_compiles_a_spinel_rb_score() {
    let file = spinel_score_file();
    let (bar0, warning) = file.watcher().next_bar().unwrap();
    assert!(warning.is_none());
    assert_eq!(bar0.start, Duration::ZERO);
    assert_eq!(bar0.notes[0].tone, BassTone::default());
    assert_eq!(timeline(&bar0), [(0, hz(36))]);
}

/// Plays bar 0, then stages a Spinel edit with the soft preset and C3.
fn spinel_watcher_after_edit(file: &ScoreFile) -> ScoreWatcher {
    let mut watcher = file.watcher();
    watcher.next_bar().unwrap();
    file.write(SOFT_C3_RB);
    watcher.stage_reload();
    watcher
}

#[test]
fn score_watcher_switches_a_spinel_edit_at_the_next_bar() {
    let file = spinel_score_file();
    let (bar1, warning) = spinel_watcher_after_edit(&file).next_bar().unwrap();
    assert!(warning.is_none());
    assert_eq!(bar1.start, secs(2));
    assert_eq!(bar1.notes[0].tone.oscillator, Oscillator::Sine);
    assert_eq!(timeline(&bar1), [(2000, hz(48))]);
}

#[test]
fn score_watcher_keeps_playing_when_a_spinel_edit_fails() {
    let file = spinel_score_file();
    let mut watcher = spinel_watcher_after_edit(&file);
    watcher.next_bar().unwrap();
    file.write(INVALID_PITCH_RB);
    let (bar2, warning) = watcher.next_bar().unwrap();
    assert!(warning.is_some());
    assert_eq!((bar2.start, bar2.notes[0].tone.oscillator), (secs(4), Oscillator::Sine));
    assert_eq!(timeline(&bar2), [(4000, hz(48))]);
}
