mod common;

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use common::audio::pcm16_wav;
use common::{
    assert_close, assert_rejects_edits, contains, examples_dir, parse, run_cli_with_stdin,
};
use spinel_sonic_lab::model::{Oscillator, Score};
use spinel_sonic_lab::osc::encode_playback_bundle;
use spinel_sonic_lab::sample::SampleAsset;
use spinel_sonic_lab::sampler::{SampleBackend, SampleBuffers};
use spinel_sonic_lab::schedule::{
    LiveScheduler, PlaybackEvent, PlaybackSound, ScheduledBar, Scheduler, midi_to_frequency,
};
use spinel_sonic_lab::source::{prepare_score, with_sample_snapshot};
use spinel_sonic_lab::watch::ScoreWatcher;

const SAMPLE_SCORE: &str = concat!(
    "{\"type\":\"tempo\",\"bpm\":120}\n",
    "{\"type\":\"sample\",\"name\":\"kick\",\"path\":\"../../samples/kick.wav\"}\n",
    "{\"type\":\"sample_note\",\"sample\":\"kick\",\"beat\":0,\"velocity\":0.5,\"track\":\"drums\"}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":36,\"beat\":0,\"duration_beats\":0.5,\"velocity\":0.5}\n"
);
const KICK_WAV: &[u8] = include_bytes!("../../samples/kick.wav");
const RELATIVE_KICK: &str = "../../samples/kick.wav";

/// Tempo, the kick declaration and its trigger, without the synth note.
fn sample_only_score() -> String {
    SAMPLE_SCORE.lines().take(3).collect::<Vec<_>>().join("\n")
}

/// Validates `source` against the example directory, where relative WAV paths resolve.
fn prepared(source: &str) -> Score {
    prepare_score(source, &examples_dir(), None).unwrap()
}

fn example(name: &str) -> Score {
    prepared(&fs::read_to_string(examples_dir().join(name)).unwrap())
}

fn absolute_kick() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../samples/kick.wav");
    path.canonicalize().unwrap().to_str().unwrap().to_owned()
}

fn kick_asset(path: &str) -> SampleAsset {
    SampleAsset::from_wav(path.into(), KICK_WAV.to_vec()).unwrap()
}

/// Start time and size of each group of simultaneous events.
fn group_sizes(plan: &[PlaybackEvent]) -> Vec<(Duration, usize)> {
    plan.chunk_by(|a, b| a.at == b.at).map(|group| (group[0].at, group.len())).collect()
}

fn at_millis<const N: usize>(millis: [u64; N]) -> [Duration; N] {
    millis.map(Duration::from_millis)
}

fn kick_beats(score: &Score) -> Vec<f64> {
    let kicks = score.sample_events.iter().filter(|event| event.sample == "kick");
    kicks.map(|event| event.beat).collect()
}

#[test]
fn sample_declarations_and_triggers_accept_mixed_and_sample_only_scores() {
    let score = parse(SAMPLE_SCORE);
    assert!(Score::parse_ndjson(&sample_only_score()).is_ok());
    assert!(score.without_track("drums").is_ok());
    assert!(score.without_track("main").is_ok());
    assert!(score.without_track("unknown").is_err());
}

const INVALID_SAMPLE_EDITS: [(&str, &str); 7] = [
    ("\"sample\":\"kick\"", "\"sample\":\"missing\""),
    ("\"velocity\":0.5", "\"velocity\":1.1"),
    ("\"beat\":0", "\"beat\":-1"),
    ("\"name\":\"kick\"", "\"name\":\"bad/name\""),
    ("\"path\":\"../../samples/kick.wav\"", "\"path\":\"\""),
    ("\"sample\":\"kick\"", "\"sample\":\"kick\",\"rate\":2"),
    (
        "{\"type\":\"sample_note\"",
        "{\"type\":\"sample\",\"name\":\"kick\",\"path\":\"x.wav\"}\n{\"type\":\"sample_note\"",
    ),
];

#[test]
fn sample_errors_are_rejected_even_on_a_muted_track() {
    assert_rejects_edits(SAMPLE_SCORE, &INVALID_SAMPLE_EDITS);
}

#[test]
fn sample_declarations_after_triggers_are_rejected() {
    let late =
        format!("{SAMPLE_SCORE}{{\"type\":\"sample\",\"name\":\"late\",\"path\":\"x.wav\"}}\n");
    assert!(Score::parse_ndjson(&late).is_err());
}

#[test]
fn wav_format_and_length_are_read_from_the_header() {
    let asset = kick_asset("kick.wav");
    assert_eq!((asset.channels, asset.frames, asset.sample_rate), (2, 16832, 48000));
    assert!(asset.duration() > Duration::from_millis(350));
}

#[test]
fn truncated_wavs_are_rejected() {
    for bytes in [&[][..], &KICK_WAV[..40], &KICK_WAV[..KICK_WAV.len() - 1]] {
        assert!(SampleAsset::from_wav("kick.wav".into(), bytes.to_vec()).is_err());
    }
}

fn kick_with_u16(offset: usize, value: u16) -> Vec<u8> {
    let mut wav = KICK_WAV.to_vec();
    wav[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    wav
}

#[test]
fn float_and_multichannel_wavs_are_rejected() {
    let float_format = kick_with_u16(20, 3);
    assert!(SampleAsset::from_wav("kick.wav".into(), float_format).is_err());
    let three_channels = kick_with_u16(22, 3);
    assert!(SampleAsset::from_wav("kick.wav".into(), three_channels).is_err());
}

#[test]
fn mixed_events_share_bundle_time_and_use_full_wav_length() {
    let plan = Scheduler::plan_playback(&prepared(SAMPLE_SCORE)).unwrap();
    assert_eq!(group_sizes(&plan), [(Duration::ZERO, 2)]);
    assert!(matches!(plan[0].sound, PlaybackSound::Sample { .. }));
    assert!(plan[0].duration > Duration::from_millis(350));
    assert_eq!(plan[0].amplitude, 0.5);
}

#[test]
fn muting_the_sample_track_keeps_the_synth_note() {
    let muted = prepare_score(SAMPLE_SCORE, &examples_dir(), Some("drums")).unwrap();
    assert_eq!(Scheduler::plan_playback(&muted).unwrap().len(), 1);
}

#[test]
fn missing_wavs_are_rejected_even_on_a_muted_track() {
    let missing = SAMPLE_SCORE.replace(RELATIVE_KICK, "missing.wav");
    let error = prepare_score(&missing, &examples_dir(), Some("drums")).unwrap_err();
    assert!(error.contains("sample"), "{error}");
}

type BackendLog = Rc<RefCell<Vec<String>>>;

/// Records buffer commands; loading any path containing "fail" fails.
#[derive(Clone, Default)]
struct FakeSampleBackend(BackendLog);

impl SampleBackend for FakeSampleBackend {
    fn load(&self, id: i32, asset: &SampleAsset) -> Result<(), String> {
        self.0.borrow_mut().push(format!("load {id}"));
        if asset.path.to_string_lossy().contains("fail") {
            return Err("load failed".into());
        }
        Ok(())
    }
    fn free(&self, id: i32) -> Result<(), String> {
        self.0.borrow_mut().push(format!("free {id}"));
        Ok(())
    }
}

fn kick_pool(backend: &FakeSampleBackend) -> SampleBuffers<FakeSampleBackend> {
    let mut pool = SampleBuffers::new(backend.clone());
    pool.prepare([&kick_asset("kick.wav")]).unwrap();
    pool
}

#[test]
fn sample_buffers_load_each_asset_once() {
    let backend = FakeSampleBackend::default();
    let mut pool = kick_pool(&backend);
    pool.prepare([&kick_asset("kick.wav")]).unwrap();
    assert_eq!(*backend.0.borrow(), ["load 0"]);
}

#[test]
fn sample_buffers_stay_loaded_until_their_last_reserved_end() {
    let kick = kick_asset("kick.wav");
    let mut pool = kick_pool(&FakeSampleBackend::default());
    pool.mark_used(&kick, Duration::from_secs(3)).unwrap();
    pool.prune(Duration::from_secs(2), &[]).unwrap();
    assert_eq!(pool.buffer_id(&kick).unwrap(), 0);
    pool.prune(Duration::from_secs(3), &[]).unwrap();
    assert!(pool.buffer_id(&kick).is_err());
}

#[test]
fn failed_buffer_preparation_rolls_back_new_buffers() {
    let mut pool = kick_pool(&FakeSampleBackend::default());
    let (another, failing) = (kick_asset("another.wav"), kick_asset("fail.wav"));
    assert!(pool.prepare([&another, &failing]).is_err());
    assert!(pool.buffer_id(&another).is_err());
    assert_eq!(pool.buffer_id(&kick_asset("kick.wav")).unwrap(), 0);
}

fn mixed_bundle() -> Vec<u8> {
    let plan = Scheduler::plan_playback(&prepared(SAMPLE_SCORE)).unwrap();
    encode_playback_bundle(123, &plan, |_| Ok(7)).unwrap()
}

#[test]
fn mixed_bundle_holds_the_sample_then_the_synth_message() {
    let packet = mixed_bundle();
    assert_eq!(&packet[..8], b"#bundle\0");
    assert_eq!(&packet[8..16], &123_u64.to_be_bytes());
    let first_len = u32::from_be_bytes(packet[16..20].try_into().unwrap()) as usize;
    assert!(packet[20 + first_len + 4..].starts_with(b"/s_new\0"));
}

#[test]
fn mixed_bundle_encodes_sample_buffer_but_no_track_or_path() {
    let packet = mixed_bundle();
    assert!(contains(&packet, b"sample_stereo\0"));
    assert!(!contains(&packet, b"drums"));
    assert!(!contains(&packet, b".wav"));
}

#[test]
fn compiled_drum_example_expands_steps_and_merges_with_bass() {
    let score = example("samples.ndjson");
    let groups = group_sizes(&Scheduler::plan_playback(&score).unwrap());
    let millis = [(0, 1), (250, 2), (500, 2), (750, 2), (1000, 1), (1250, 2), (1500, 2), (1750, 2)];
    assert_eq!(groups, millis.map(|(ms, n)| (Duration::from_millis(ms), n)));
    let frequencies: Vec<_> =
        Scheduler::plan(&score).unwrap().iter().map(|n| n.frequency).collect();
    assert_eq!(frequencies[..4], [36, 40, 43, 48].map(midi_to_frequency));
}

fn techno_beat(beat: f64) -> Duration {
    Duration::from_secs_f64(beat * 60.0 / 133.0)
}

#[test]
fn techno_example_keeps_four_on_the_floor() {
    let score = example("techno.ndjson");
    assert_eq!(score.bpm, 133.0);
    assert_eq!(kick_beats(&score), [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]);
}

#[test]
fn techno_bass_plays_offbeats_with_a_two_bar_turnaround() {
    let bass = Scheduler::plan(&example("techno.ndjson")).unwrap();
    let times: Vec<_> = bass.iter().map(|note| note.at).collect();
    assert_eq!(times, [0.5, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5].map(techno_beat));
    let frequencies: Vec<_> = bass.iter().map(|note| note.frequency).collect();
    assert_eq!(frequencies, [36, 36, 36, 36, 36, 36, 36, 39].map(midi_to_frequency));
}

#[test]
fn techno_layers_share_bundles_on_the_downbeats() {
    let groups = group_sizes(&Scheduler::plan_playback(&example("techno.ndjson")).unwrap());
    let expected = [(1.0, 2), (7.0, 3), (7.5, 1), (7.75, 1)];
    for (beat, size) in expected {
        assert!(groups.contains(&(techno_beat(beat), size)), "beat {beat}: {groups:?}");
    }
}

#[test]
fn per_bar_grids_expand_rhythm_and_melody() {
    let score = example("grid_demo.ndjson");
    assert_eq!(score.loop_bars, Some(4));
    assert_eq!((score.sample_events.len(), score.events.len()), (19, 16));
    assert_eq!((score.events[0].note, score.events[2].note), (48, 52));
    let last = &score.events[15];
    assert_eq!((last.beat, last.duration_beats), (15.9375, 0.0625));
}

#[test]
fn per_bar_grids_merge_sample_and_melody_on_the_last_64th_step() {
    let plan = Scheduler::plan_playback(&example("grid_demo.ndjson")).unwrap();
    let last_step = Duration::from_secs_f64(15.9375 * 0.5);
    assert_eq!(plan.iter().filter(|event| event.at == last_step).count(), 2);
}

#[test]
fn grid_expression_sets_sample_velocity() {
    let score = example("grid_expression.ndjson");
    assert_eq!((score.sample_events.len(), score.events.len()), (8, 12));
    assert_eq!(score.sample_events[0].velocity, 0.4);
}

#[test]
fn grid_expression_preserves_note_length_and_velocity() {
    let score = example("grid_expression.ndjson");
    let note = |i: usize| {
        let event = &score.events[i];
        (event.note, event.beat, event.duration_beats, event.velocity)
    };
    assert_eq!(note(0), (48, 0.0, 0.5, 0.7));
    assert_eq!(note(2), (52, 2.0, 1.0, 0.8));
}

#[test]
fn grid_expression_plays_a_simultaneous_sample_hit() {
    let plan = Scheduler::plan_playback(&example("grid_expression.ndjson")).unwrap();
    let start: Vec<_> = plan.iter().filter(|event| event.at == Duration::ZERO).collect();
    assert_eq!(start.len(), 2);
    assert_close(start[0].amplitude, 0.4, 0.00001);
    assert_close(start[1].amplitude, 0.14, 0.00001);
    assert_eq!(start[1].duration, Duration::from_millis(250));
    let held = plan.iter().find(|event| event.at == Duration::from_secs(1)).unwrap();
    assert_eq!(held.duration, Duration::from_millis(500));
}

#[test]
fn console_grid_declares_two_channels_over_four_bars() {
    let score = example("console_grid.ndjson");
    assert_eq!((score.loop_bars, score.channels.len()), (Some(4), 2));
}

#[test]
fn console_grid_applies_channel_level_and_pan_to_sample_and_synth() {
    let plan = Scheduler::plan_playback(&example("console_grid.ndjson")).unwrap();
    let start: Vec<_> = plan.iter().filter(|event| event.at == Duration::ZERO).collect();
    assert_eq!(start.len(), 2);
    assert_eq!((start[0].pan, start[1].pan), (Some(-1.0), Some(1.0)));
    assert_close(start[0].amplitude, 0.4, 0.00001);
    assert_close(start[1].amplitude, 0.0501187, 0.00001);
}

#[test]
fn console_grid_encodes_pan_but_no_track_name() {
    let plan = Scheduler::plan_playback(&example("console_grid.ndjson")).unwrap();
    let packet = encode_playback_bundle(1, &plan[..2], |_| Ok(7)).unwrap();
    assert_eq!(packet.windows(4).filter(|part| *part == b"pan\0").count(), 2);
    assert!(!contains(&packet, b"kick"));
}

/// A score file with an absolute kick path, removed on drop.
struct TempScore {
    path: PathBuf,
    source: String,
}

impl TempScore {
    fn new() -> Self {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("spinel-sample-watch-{nanos}.ndjson"));
        let source = SAMPLE_SCORE.replace(RELATIVE_KICK, &absolute_kick());
        fs::write(&path, &source).unwrap();
        Self { path, source }
    }

    fn write_replacing(&self, from: &str, to: &str) {
        fs::write(&self.path, self.source.replace(from, to)).unwrap();
    }
}

impl Drop for TempScore {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn bar_start_and_pitch(bar: &ScheduledBar) -> (Duration, f32) {
    (bar.start, bar.notes[0].frequency)
}

/// Plays bar 0, then rejects a C3 edit because its engine preparation fails.
fn watcher_after_failed_preparation(file: &TempScore) -> ScoreWatcher {
    let mut watcher = ScoreWatcher::from_path(&file.path, None).unwrap();
    watcher.next_bar().unwrap();
    file.write_replacing("\"note\":36", "\"note\":48");
    let (bar, warning) = watcher.next_bar_with(&mut |_| Err("sample load failed".into())).unwrap();
    assert_eq!(warning.as_deref(), Some("sample load failed"));
    assert_eq!(bar_start_and_pitch(&bar), (Duration::from_secs(2), midi_to_frequency(36)));
    watcher
}

#[test]
fn failed_engine_preparation_keeps_the_active_score_at_the_next_bar() {
    watcher_after_failed_preparation(&TempScore::new());
}

#[test]
fn a_later_valid_edit_commits_once_engine_preparation_succeeds() {
    let file = TempScore::new();
    let mut watcher = watcher_after_failed_preparation(&file);
    file.write_replacing("\"note\":36", "\"note\":43");
    let (bar, warning) = watcher.next_bar_with(&mut |_| Ok(())).unwrap();
    assert!(warning.is_none());
    assert_eq!(bar_start_and_pitch(&bar), (Duration::from_secs(4), midi_to_frequency(43)));
}

#[test]
fn a_missing_wav_cannot_replace_the_active_score() {
    let file = TempScore::new();
    let mut watcher = ScoreWatcher::from_path(&file.path, None).unwrap();
    watcher.next_bar().unwrap();
    file.write_replacing(&absolute_kick(), "/missing.wav");
    let (bar, warning) = watcher.next_bar().unwrap();
    assert!(warning.is_some());
    assert_eq!(bar.notes[0].frequency, midi_to_frequency(36));
}

#[test]
fn sample_only_live_score_loops_bars_with_the_full_sample_length() {
    let score = prepared(&sample_only_score());
    let length = score.sample_assets["kick"].duration();
    let mut live = LiveScheduler::new(score);
    let first = live.next_bar().unwrap();
    assert_eq!((first.playback.len(), first.playback[0].duration), (1, length));
    let second = live.next_bar().unwrap().playback.remove(0);
    assert_eq!(second.at, Duration::from_secs(2));
    assert_eq!(second.end().unwrap(), Duration::from_secs(2) + length);
}

#[test]
fn tempo_change_keeps_the_sample_duration() {
    let score = prepared(&sample_only_score());
    let length = score.sample_assets["kick"].duration();
    let mut live = LiveScheduler::new(score);
    live.next_bar().unwrap();
    live.reload_ndjson(&sample_only_score().replace("120", "60"), None).unwrap();
    let bar = live.next_bar().unwrap();
    assert_eq!((bar.start, bar.duration), (Duration::from_secs(2), Duration::from_secs(4)));
    assert_eq!(bar.playback[0].duration, length);
}

#[test]
fn muting_the_only_sample_track_is_rejected() {
    assert!(prepare_score(&sample_only_score(), &examples_dir(), Some("drums")).is_err());
}

/// Checks the snapshot an engine would read, then rejects it like a failing engine.
fn reject_snapshot(
    path: &Path,
    asset: &SampleAsset,
    seen: &mut Option<PathBuf>,
) -> Result<(), String> {
    assert_ne!(path, asset.path);
    assert_eq!(fs::read(path).unwrap().as_slice(), asset.bytes.as_ref());
    *seen = Some(path.to_owned());
    Err("engine rejected it".into())
}

#[test]
fn validated_snapshot_is_cleaned_up_and_independent_of_original_file() {
    let score = prepared(SAMPLE_SCORE);
    let asset = &score.sample_assets["kick"];
    let mut seen = None;
    assert!(with_sample_snapshot(asset, |path| reject_snapshot(path, asset, &mut seen)).is_err());
    assert!(!seen.unwrap().exists());
}

#[test]
fn cli_validates_wavs_without_an_audio_server_and_accepts_sample_only_stdin() {
    let source = sample_only_score().replace(RELATIVE_KICK, &absolute_kick());
    let output = run_cli_with_stdin(&["--dry-run", "-"], &source);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout, "0.000s sample=kick dur=0.351s amp=0.500\n");
}

#[test]
fn cli_watch_dry_run_loops_sampled_bars() {
    let args = ["--dry-run", "--watch", "--bars", "2", "../composer/examples/samples.ndjson"];
    let output = run_cli_with_stdin(&args, "");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 28);
    assert!(stdout.lines().nth(14).unwrap().starts_with("2.000s sample=kick"));
}

#[test]
fn muted_sample_still_gets_its_complete_musical_timing_validation() {
    let bad = SAMPLE_SCORE.replacen("\"beat\":0", "\"beat\":1e308", 1);
    assert!(spinel_sonic_lab::schedule::parse_and_validate_score(&bad, Some("drums")).is_err());
}

#[test]
fn wav_data_must_follow_its_format_chunk() {
    let mut reordered = KICK_WAV[..12].to_vec();
    reordered.extend_from_slice(&KICK_WAV[36..]);
    reordered.extend_from_slice(&KICK_WAV[12..36]);
    assert!(SampleAsset::from_wav("reordered.wav".into(), reordered).is_err());
}

#[test]
fn pure_live_reload_cannot_commit_an_unprepared_sample() {
    let mut live = LiveScheduler::new(prepared(SAMPLE_SCORE));
    live.next_bar().unwrap();
    let changed = SAMPLE_SCORE.replace(RELATIVE_KICK, "new.wav");
    assert!(live.reload_ndjson(&changed, None).is_err());
    assert_eq!(live.next_bar().unwrap().start, Duration::from_secs(2));
}

#[test]
fn sample_mix_finishes_each_bass_note_before_the_next_kick() {
    let bass = Scheduler::plan(&example("samples.ndjson")).unwrap();
    assert_eq!(bass.iter().map(|n| n.at).collect::<Vec<_>>(), at_millis([250, 750, 1250, 1750]));
    for (index, note) in bass.iter().enumerate() {
        let end = note.at.as_secs_f64() + f64::from(note.duration_seconds);
        assert!(end < (index + 1) as f64 * 0.5, "note {index} ends at {end}");
    }
}

#[test]
fn sample_mix_gives_kick_its_own_attack() {
    let score = example("samples.ndjson");
    let bass = Scheduler::plan(&score).unwrap();
    assert!(bass.iter().all(|n| n.tone.oscillator == Oscillator::Saw));
    assert!(bass.iter().all(|n| n.tone.attack_seconds <= 0.003));
    assert_eq!(kick_beats(&score), [0.0, 1.0, 2.0, 3.0]);
}

#[test]
fn pcm16_samples_are_read_interleaved_from_the_data_chunk() {
    let samples = [1, -2, i16::MAX, i16::MIN];
    // A LIST chunk before `data` keeps the samples away from byte 44.
    let wav = pcm16_wav(2, &samples, b"LIST\x03\x00\x00\x00abc\x00");
    let asset = SampleAsset::from_wav("list.wav".into(), wav).unwrap();
    assert_eq!(asset.frames, 2);
    assert_eq!(asset.pcm16_samples(), samples);
}
