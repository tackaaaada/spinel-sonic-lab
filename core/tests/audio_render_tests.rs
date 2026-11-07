//! Renders scores through the live encoders with `scsynth -N` and measures the WAVs.
//! Requires scsynth and generated SynthDefs: `cargo test -- --ignored` (`mise run audio-check`).

mod common;

use std::path::{Path, PathBuf};

use common::TempDir;
use common::audio::{Stereo, db, pcm16_wav, peak, read_wav};
use spinel_sonic_lab::nrt::score_commands;
use spinel_sonic_lab::sample::SampleAsset;
use spinel_sonic_lab::source::{prepare_score, render_nrt};

/// One PCM16 quantization step.
const ONE_STEP: f64 = 1.0 / 32768.0;
/// Clearly above rounding noise.
const AUDIBLE: f64 = 100.0 * ONE_STEP;

fn project_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Renders one score whose sample paths are relative to `base`.
fn render_in(ndjson: &str, base: &Path) -> Stereo {
    let score = prepare_score(ndjson, base, None).unwrap();
    let commands = score_commands(&score, &project_dir().join("synthdefs"), None).unwrap();
    let dir = TempDir::create("spinel-nrt");
    Stereo::from_asset(&render_nrt(&commands, &dir.0.join("render.wav")).unwrap())
}

fn render(ndjson: &str) -> Stereo {
    render_in(ndjson, &project_dir())
}

/// A 0.4 s C3 note at 60 bpm; velocity 0.5 is amplitude 0.1.
fn bass_note(oscillator: &str, velocity: f64, channel: &str) -> String {
    let tone = format!(
        r#"{{"type":"tone","instrument":"analog_bass","oscillator":"{oscillator}","cutoff_hz":1800.0,"attack_seconds":0.005,"decay_seconds":0.04,"sustain_level":0.6,"release_seconds":0.06,"gain":1.0}}"#
    );
    let note = format!(
        r#"{{"type":"note","instrument":"analog_bass","note":48,"beat":0.0,"duration_beats":0.4,"velocity":{velocity}}}"#
    );
    format!("{{\"type\":\"tempo\",\"bpm\":60}}\n{tone}\n{channel}{note}\n")
}

/// A `main` channel line at unity H/A and Fader, with optional processing objects
/// such as `,"compressor":{...}`.
fn channel(pan: f64, processing: &str) -> String {
    channel_with(0.0, pan, 0.0, processing)
}

fn channel_with(ha_db: f64, pan: f64, fader_db: f64, processing: &str) -> String {
    format!(
        r#"{{"type":"channel","track":"main","ha_gain_db":{ha_db},"pan":{pan},"fader_db":{fader_db}{processing}}}"#
    ) + "\n"
}

#[test]
#[ignore = "requires scsynth and generated SynthDefs"]
fn every_oscillator_renders_audibly_with_the_legacy_center_balance() {
    for oscillator in ["saw", "sine", "pulse", "noise"] {
        let audio = render(&bass_note(oscillator, 0.5, ""));
        assert_eq!(audio.left, audio.right, "{oscillator}: center must keep both sides equal");
        let (peak, rms) = (audio.peak(), audio.rms());
        assert!(0.001 < rms && rms < peak && peak < 0.95, "{oscillator}: {peak} / {rms}");
        println!("{oscillator}: peak {:.2} dBFS, RMS {:.2} dBFS", db(peak), db(rms));
    }
}

#[test]
#[ignore = "requires scsynth"]
fn a_command_scsynth_rejects_fails_the_render_instead_of_writing_silence() {
    let score = prepare_score(&bass_note("saw", 0.5, ""), &project_dir(), None).unwrap();
    let missing = project_dir().join("missing-synthdefs");
    let commands = score_commands(&score, &missing, None).unwrap();
    let dir = TempDir::create("spinel-nrt");
    let error = render_nrt(&commands, &dir.0.join("render.wav")).unwrap_err();
    assert!(error.contains("FAILURE IN SERVER"), "{error}");
}

#[test]
#[ignore = "requires scsynth and generated SynthDefs"]
fn channel_pan_moves_a_synth_hard_left_and_velocity_scales_its_level() {
    let left = render(&bass_note("saw", 0.5, &channel(-1.0, "")));
    assert_hard_left(&left, "synth");
    let full = render(&bass_note("saw", 0.5, "")).rms();
    let half = render(&bass_note("saw", 0.25, "")).rms();
    assert!((0.49..0.51).contains(&(half / full)), "half velocity gave {}", half / full);
}

fn assert_hard_left(audio: &Stereo, label: &str) {
    assert_eq!(peak(&audio.right), 0.0, "{label}: hard left must silence the right side");
    assert!(peak(&audio.left) > AUDIBLE, "{label}: the left side must stay audible");
}

/// Hard left keeps sqrt(2) times the center level on the left side.
fn assert_equal_power(left: &Stereo, center: &Stereo, label: &str) {
    let ratio = peak(&left.left) / peak(&center.left);
    assert!((1.40..1.42).contains(&ratio), "{label}: pan law ratio {ratio}");
}

fn sample_hit(path: &Path, channel: &str) -> String {
    let declare = format!(r#"{{"type":"sample","name":"hit","path":"{}"}}"#, path.display());
    let hit = r#"{"type":"sample_note","sample":"hit","beat":0.0,"velocity":0.4}"#;
    format!("{{\"type\":\"tempo\",\"bpm\":60}}\n{declare}\n{channel}{hit}\n")
}

/// A 0.1 s mono WAV at a constant level.
fn constant_mono_wav(dir: &Path) -> PathBuf {
    let path = dir.join("mono.wav");
    std::fs::write(&path, pcm16_wav(1, &[6000; 4800], &[])).unwrap();
    path
}

#[test]
#[ignore = "requires scsynth and generated SynthDefs"]
fn mono_and_stereo_samples_pan_hard_left_with_the_equal_power_law() {
    let dir = TempDir::create("spinel-mono");
    let kick = project_dir().join("samples/kick.wav").canonicalize().unwrap();
    for path in [constant_mono_wav(&dir.0), kick] {
        let center = render_in(&sample_hit(&path, ""), &dir.0);
        let left = render_in(&sample_hit(&path, &channel(-1.0, "")), &dir.0);
        assert_hard_left(&left, &path.display().to_string());
        assert_equal_power(&left, &center, &path.display().to_string());
    }
}

/// A steady 1 s G3 (196 Hz) sine at amplitude 0.2 through `main`, measured while it holds.
fn sine_through(channel_line: &str) -> Stereo {
    render(&sine_score(channel_line, "")).window(0.3, 0.8)
}

fn sine_score(channel_line: &str, returns: &str) -> String {
    let tone = r#"{"type":"tone","instrument":"analog_bass","oscillator":"sine","cutoff_hz":18000.0,"attack_seconds":0.005,"decay_seconds":0.0,"sustain_level":1.0,"release_seconds":0.02,"gain":1.0}"#;
    let note = r#"{"type":"note","instrument":"analog_bass","note":55,"beat":0.0,"duration_beats":1.0,"velocity":1.0}"#;
    format!("{{\"type\":\"tempo\",\"bpm\":60}}\n{tone}\n{returns}{channel_line}{note}\n")
}

/// Ratio 1 creates a strip without changing the signal.
const BYPASSED_COMPRESSOR: &str =
    r#","compressor":{"threshold_db":0,"ratio":1,"attack_ms":10,"release_ms":100}"#;

#[test]
#[ignore = "requires scsynth and generated SynthDefs"]
fn a_bypassed_strip_keeps_the_direct_level() {
    let direct = sine_through("").rms();
    let ratio = sine_through(&channel(0.0, BYPASSED_COMPRESSOR)).rms() / direct;
    assert!((ratio - 1.0).abs() < 0.005, "bypassed strip changed level: {ratio}");
}

/// Threshold 14 dB below the 0.2 sine; H/A feeds the compressor.
fn compressed(ratio: f64, ha_db: f64) -> f64 {
    let compressor = format!(
        r#","compressor":{{"threshold_db":-28,"ratio":{ratio},"attack_ms":5,"release_ms":50}}"#
    );
    sine_through(&channel_with(ha_db, 0.0, 0.0, &compressor)).rms()
}

#[test]
#[ignore = "requires scsynth and generated SynthDefs"]
fn the_compressor_reduces_gain_above_threshold_after_the_ha_trim() {
    let squashed = compressed(4.0, 0.0);
    let reduction = squashed / compressed(1.0, 0.0);
    let trim_gain = compressed(4.0, 12.0) / squashed;
    assert!((0.2..0.5).contains(&reduction), "4:1 gave {:.2} dB", db(reduction));
    assert!((1.1..2.0).contains(&trim_gain), "+12 dB H/A gave {:.2} dB out", db(trim_gain));
    println!("Compressor 4:1: {:.2} dB; +12 dB H/A: {:.2} dB out", db(reduction), db(trim_gain));
}

fn bell_at(frequency_hz: f64) -> f64 {
    let bell = format!(r#","eq_bell":{{"frequency_hz":{frequency_hz},"gain_db":6,"q":1}}"#);
    sine_through(&channel(0.0, &bell)).rms()
}

#[test]
#[ignore = "requires scsynth and generated SynthDefs"]
fn a_bell_boosts_only_near_its_frequency() {
    let direct = sine_through("").rms();
    let (boosted, away) = (bell_at(196.0) / direct, bell_at(5000.0) / direct);
    assert!((1.9..2.1).contains(&boosted), "+6 dB bell at the tone gave {:.2} dB", db(boosted));
    assert!((0.95..1.05).contains(&away), "+6 dB bell far away gave {:.2} dB", db(away));
    println!("Bell +6 dB at 196 Hz: {:.2} dB; at 5 kHz: {:.2} dB", db(boosted), db(away));
}

const ROOM: &str = r#"{"type":"aux_return","name":"room","reverb":{"room":0.8,"damp":0.3,"wet":1.0},"fader_db":0}
"#;

/// Fader closed to its -96 dB floor, sending to the room at `send_db`.
fn closed_fader_sending(send_db: f64) -> Stereo {
    let send = format!(r#","aux_send":{{"return":"room","level_db":{send_db}}}"#);
    render(&sine_score(&channel_with(0.0, 0.0, -96.0, &send), ROOM))
}

#[test]
#[ignore = "requires scsynth and generated SynthDefs"]
fn the_aux_send_taps_before_the_fader_and_the_return_leaves_a_tail() {
    // -96 dB is the protocol's floor, not a mute: allow one PCM16 step of residue.
    let dry = closed_fader_sending(-96.0).peak();
    assert!(dry <= ONE_STEP, "closed fader and send must be inaudible: {:.1} dBFS", db(dry));
    let wet = closed_fader_sending(0.0);
    let (held, tail) = (wet.window(0.3, 0.8).rms(), wet.window(1.2, 2.5).rms());
    assert!(held > 0.001, "the Aux tap must precede the channel Fader");
    assert!(tail > 0.0005, "the return reverb must leave a tail");
    println!("Aux return, fader closed: {:.2} dBFS RMS, tail {:.2} dBFS", db(held), db(tail));
}

#[test]
#[ignore = "requires scsynth and generated SynthDefs"]
fn strip_pan_follows_the_source_pan_law() {
    let left = sine_through(&channel(-1.0, BYPASSED_COMPRESSOR));
    let center = sine_through(&channel(0.0, BYPASSED_COMPRESSOR));
    assert_hard_left(&left, "strip");
    assert_equal_power(&left, &center, "strip");
}

/// Renders `samples/recipes/{name}.ndjson` with the `render_nrt` tool into `dir`.
fn render_recipe(name: &str, seconds: &str, dir: &Path) -> SampleAsset {
    let (recipe, output) =
        (format!("samples/recipes/{name}.ndjson"), dir.join(format!("{name}.wav")));
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_render_nrt"));
    let run = command.current_dir(project_dir()).arg(recipe).arg(&output).arg(seconds).output();
    let run = run.unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    read_wav(&output)
}

fn committed(name: &str) -> Stereo {
    Stereo::from_asset(&read_wav(&project_dir().join(format!("samples/{name}.wav"))))
}

fn largest_difference(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).fold(0.0, |max, (x, y)| max.max((x - y).abs()))
}

#[test]
#[ignore = "requires scsynth and generated SynthDefs"]
fn the_kick_recipe_reproduces_the_committed_wav_within_one_step() {
    // The committed kick predates a SynthDef rebuild that moved one frame by one step.
    let dir = TempDir::create("spinel-recipe");
    let (rendered, committed) =
        (Stereo::from_asset(&render_recipe("kick", "0.35", &dir.0)), committed("kick"));
    assert_eq!(rendered.left.len(), committed.left.len());
    assert!(largest_difference(&rendered.left, &committed.left) <= ONE_STEP);
    assert!(largest_difference(&rendered.right, &committed.right) <= ONE_STEP);
}

/// Noise drums differ per render, so compare their length and RMS instead: over ten
/// renders the peak moved by 1.6 dB but the RMS by under 0.5 dB.
#[test]
#[ignore = "requires scsynth and generated SynthDefs"]
fn noise_drum_recipes_match_the_committed_length_and_level() {
    let dir = TempDir::create("spinel-recipe");
    for (name, seconds) in [("snare", "0.30"), ("hat", "0.18")] {
        let (rendered, committed) =
            (Stereo::from_asset(&render_recipe(name, seconds, &dir.0)), committed(name));
        assert_eq!(rendered.left.len(), committed.left.len(), "{name}");
        let (new, old) = (db(rendered.rms()), db(committed.rms()));
        assert!((new - old).abs() < 1.0, "{name}: RMS {new:.2} vs {old:.2} dBFS");
    }
}
