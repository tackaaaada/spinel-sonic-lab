mod common;

use std::time::Duration;

use common::{
    assert_close, assert_rejects_edits, contains, examples_dir, parse, run_cli_with_stdin,
};
use spinel_sonic_lab::model::{BassTone, Oscillator, Score};
use spinel_sonic_lab::osc::{
    OSC_IMMEDIATE_TIMETAG, encode_bundle, encode_load, encode_s_new, encode_sync,
    system_time_to_ntp,
};
use spinel_sonic_lab::schedule::{
    LiveScheduler, ScheduledNote, Scheduler, Transport, group_notes_by_time, midi_to_frequency,
};

const SCORE: &str = "{\"type\":\"tempo\",\"bpm\":120}\n\
{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":36,\"beat\":0.0,\"duration_beats\":0.5,\"velocity\":0.8}\n";

fn plan(source: &str) -> Vec<ScheduledNote> {
    Scheduler::plan(&parse(source)).unwrap()
}

fn millis(notes: &[ScheduledNote]) -> Vec<u128> {
    notes.iter().map(|note| note.at.as_millis()).collect()
}

fn notes_of(score: &Score) -> Vec<u8> {
    score.events.iter().map(|event| event.note).collect()
}

#[test]
fn parses_legacy_score_with_the_default_tone() {
    let score = parse(SCORE);
    assert_eq!(score.bpm, 120.0);
    assert_eq!(score.bass_tone, BassTone::default());
    assert_eq!(score.bass_tone.gain, 2.8);
    assert_eq!(score.events[0].note, 36);
}

#[test]
fn schedules_a_note_from_beats_and_velocity() {
    let notes = plan(SCORE);
    assert_eq!(notes[0].at, Duration::ZERO);
    assert_eq!(notes[0].duration_seconds, 0.25);
    assert_close(notes[0].amplitude, 0.16, 0.00001);
}

const CHANNEL_DECLARATION: &str =
    "{\"type\":\"channel\",\"track\":\"bass\",\"ha_gain_db\":6,\"pan\":-1,\"fader_db\":-6}\n";
const CHANNEL_SOURCE: &str = concat!(
    "{\"type\":\"tempo\",\"bpm\":120}\n",
    "{\"type\":\"channel\",\"track\":\"bass\",\"ha_gain_db\":6,\"pan\":-1,\"fader_db\":-6}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"bass\",\"note\":48,\"beat\":0,\"duration_beats\":0.5,\"velocity\":0.5}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"plain\",\"note\":52,\"beat\":0,\"duration_beats\":0.5,\"velocity\":0.5}\n"
);
const INVALID_CHANNEL_EDITS: [(&str, &str); 7] = [
    ("\"pan\":-1", "\"pan\":-1.1"),
    ("\"fader_db\":-6", "\"fader_db\":25"),
    ("\"ha_gain_db\":6", "\"ha_gain_db\":-97"),
    ("\"track\":\"bass\",\"ha_gain_db\"", "\"track\":\"bad/name\",\"ha_gain_db\""),
    ("\"pan\":-1", "\"pan\":\"left\""),
    ("\"fader_db\":-6", "\"fader_db\":-6,\"node_id\":1"),
    (
        "{\"type\":\"note\"",
        "{\"type\":\"channel\",\"track\":\"bass\",\"ha_gain_db\":0,\"pan\":0,\"fader_db\":0}\n{\"type\":\"note\"",
    ),
];

#[test]
fn muting_a_channel_track_drops_its_channel_and_notes() {
    let muted = parse(CHANNEL_SOURCE).without_track("bass").unwrap();
    assert!(muted.channels.is_empty());
    assert_eq!(muted.events.len(), 1);
}

#[test]
fn channel_trim_pan_and_fader_schedule_synth_levels() {
    let notes = plan(CHANNEL_SOURCE);
    assert_eq!((notes[0].pan, notes[1].pan), (Some(-1.0), None));
    assert_close(notes[0].amplitude, 0.1, 0.00001);
    assert_close(notes[1].amplitude, 0.1, 0.00001);
}

#[test]
fn channel_pan_is_encoded_only_when_set() {
    let notes = plan(CHANNEL_SOURCE);
    let panned = encode_s_new(&notes[0]);
    assert!(contains(&panned, b"pan\0"));
    assert!(contains(&panned, &(-1.0_f32).to_be_bytes()));
    assert!(!contains(&encode_s_new(&notes[1]), b"pan\0"));
}

#[test]
fn rejects_invalid_or_duplicate_channels() {
    assert_rejects_edits(CHANNEL_SOURCE, &INVALID_CHANNEL_EDITS);
}

#[test]
fn rejects_a_channel_declared_after_its_notes() {
    let late = CHANNEL_SOURCE.replacen(CHANNEL_DECLARATION, "", 1) + CHANNEL_DECLARATION;
    assert!(Score::parse_ndjson(&late).is_err());
}

const CONSOLE_FULL: &str = include_str!("../../composer/examples/console_full.ndjson");
const INVALID_CONSOLE_EDITS: [(&str, &str); 5] = [
    ("\"ratio\":3.000", "\"ratio\":0.500"),
    ("\"frequency_hz\":80.000", "\"frequency_hz\":1.000"),
    ("\"return\":\"room\"", "\"return\":\"missing\""),
    ("\"wet\":0.250", "\"wet\":1.500"),
    ("\"q\":1.000", "\"q\":1.000,\"node_id\":123"),
];

#[test]
fn full_console_declarations_parse_every_module() {
    let score = parse(CONSOLE_FULL);
    assert_eq!(score.aux_returns.len(), 1);
    let channel = &score.channels["kick"];
    assert_eq!(channel.compressor.as_ref().unwrap().ratio, 3.0);
    assert_eq!(channel.eq_bell.as_ref().unwrap().frequency_hz, 80.0);
    assert_eq!(channel.aux_send.as_ref().unwrap().return_name, "room");
}

#[test]
fn full_console_events_route_through_the_strip() {
    let score = spinel_sonic_lab::source::prepare_score(CONSOLE_FULL, &examples_dir(), None);
    let plan = Scheduler::plan_playback(&score.unwrap()).unwrap();
    assert!(plan.iter().all(|event| event.route.as_deref() == Some("kick")));
    assert!(plan.iter().all(|event| event.pan.is_none()));
}

#[test]
fn rejects_invalid_console_modules() {
    assert_rejects_edits(CONSOLE_FULL, &INVALID_CONSOLE_EDITS);
}

const BRIGHT_BASS: &str = include_str!("../../composer/examples/bright_bass.ndjson");
const BRIGHT_TONE: BassTone = BassTone {
    oscillator: Oscillator::Saw,
    cutoff_hz: 1800.0,
    attack_seconds: 0.005,
    decay_seconds: 0.0,
    sustain_level: 1.0,
    release_seconds: 0.06,
    lfo_rate_hz: 0.0,
    lfo_depth: 0.0,
    gain: 2.6,
};
const INVALID_TONE_EDITS: [(&str, &str); 11] = [
    ("1800.0", "20000.0"),
    ("0.005", "-0.1"),
    ("0.060", "6.0"),
    ("analog_bass", "unknown"),
    ("\"cutoff_hz\":1800.0,", ""),
    ("\"oscillator\":\"saw\"", "\"oscillator\":\"triangle\""),
    ("\"oscillator\":\"saw\"", "\"oscillator\":5"),
    ("\"gain\":2.600", "\"gain\":-1"),
    ("\"gain\":2.600", "\"gain\":4.1"),
    ("\"gain\":2.600", "\"gain\":\"loud\""),
    (
        "{\"type\":\"tempo\",\"bpm\":120}\n",
        "{\"type\":\"tempo\",\"bpm\":120}\n{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"cutoff_hz\":800,\"attack_seconds\":0.01,\"release_seconds\":0.09}\n",
    ),
];

#[test]
fn parses_tone_with_defaults_for_unset_fields() {
    assert_eq!(parse(BRIGHT_BASS).bass_tone, BRIGHT_TONE);
}

#[test]
fn older_tones_default_to_saw_at_unity_gain() {
    let without_oscillator = BRIGHT_BASS.replace("\"oscillator\":\"saw\",", "");
    assert_eq!(parse(&without_oscillator).bass_tone.oscillator, Oscillator::Saw);
    let without_gain = BRIGHT_BASS.replace(",\"gain\":2.600", "");
    assert_eq!(parse(&without_gain).bass_tone.gain, 1.0);
}

#[test]
fn rejects_invalid_or_duplicate_tones() {
    assert_rejects_edits(BRIGHT_BASS, &INVALID_TONE_EDITS);
}

#[test]
fn rejects_a_tone_after_notes() {
    let lines: Vec<_> = BRIGHT_BASS.lines().collect();
    let after_notes = format!("{}\n{}\n{}\n", lines[0], lines[2], lines[1]);
    assert!(Score::parse_ndjson(&after_notes).is_err());
}

const LFO_BASS: &str = include_str!("../../composer/examples/lfo_bass.ndjson");
const ADSR_BASS: &str = include_str!("../../composer/examples/adsr_bass.ndjson");
const INVALID_LFO_EDITS: [(&str, &str); 7] = [
    ("\"lfo_rate_hz\":4.000", "\"lfo_rate_hz\":0"),
    ("\"lfo_rate_hz\":4.000", "\"lfo_rate_hz\":21"),
    ("\"lfo_rate_hz\":4.000", "\"lfo_rate_hz\":\"fast\""),
    ("\"lfo_depth\":0.900", "\"lfo_depth\":-0.1"),
    ("\"lfo_depth\":0.900", "\"lfo_depth\":1.1"),
    (",\"lfo_rate_hz\":4.000", ""),
    (",\"lfo_depth\":0.900", ""),
];
const INVALID_ADSR_EDITS: [(&str, &str); 5] = [
    ("\"decay_seconds\":0.120", "\"decay_seconds\":-1"),
    ("\"decay_seconds\":0.120", "\"decay_seconds\":6"),
    ("\"decay_seconds\":0.120", "\"decay_seconds\":\"slow\""),
    ("\"sustain_level\":0.250", "\"sustain_level\":1.5"),
    ("\"sustain_level\":0.250", "\"sustain_level\":-0.1"),
];

fn note_timing(score: &Score) -> Vec<(u8, f64, f64)> {
    score.events.iter().map(|event| (event.note, event.beat, event.duration_beats)).collect()
}

#[test]
fn parses_lfo_settings() {
    let tone = parse(LFO_BASS).bass_tone;
    assert_eq!((tone.lfo_rate_hz, tone.lfo_depth), (4.0, 0.9));
}

#[test]
fn lfo_example_keeps_the_adsr_example_rhythm() {
    let (lfo, adsr) = (parse(LFO_BASS), parse(ADSR_BASS));
    assert_eq!(lfo.bpm, adsr.bpm);
    assert_eq!(note_timing(&lfo), note_timing(&adsr));
}

#[test]
fn rejects_invalid_modulation() {
    assert_rejects_edits(LFO_BASS, &INVALID_LFO_EDITS);
}

#[test]
fn parses_adsr_and_rejects_invalid_envelopes() {
    let tone = parse(ADSR_BASS).bass_tone;
    assert_eq!((tone.decay_seconds, tone.sustain_level), (0.12, 0.25));
    assert_rejects_edits(ADSR_BASS, &INVALID_ADSR_EDITS);
}

const NOISE_SCORE: &str = concat!(
    "{\"type\":\"tempo\",\"bpm\":120}\n",
    "{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"oscillator\":\"noise\",\"cutoff_hz\":4500.0,\"attack_seconds\":0.001,\"decay_seconds\":0.090,\"sustain_level\":0.000,\"release_seconds\":0.050,\"gain\":2.200}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":50,\"beat\":0.0,\"duration_beats\":0.25,\"velocity\":0.8}\n"
);

#[test]
fn parses_sine_and_pulse_oscillators() {
    let sine = include_str!("../../composer/examples/sine_bass.ndjson");
    let pulse = include_str!("../../composer/examples/pulse_bass.ndjson");
    assert_eq!(parse(sine).bass_tone.oscillator, Oscillator::Sine);
    assert_eq!(parse(pulse).bass_tone.oscillator, Oscillator::Pulse);
}

#[test]
fn parses_noise_oscillator_and_encodes_its_id() {
    assert_eq!(parse(NOISE_SCORE).bass_tone.oscillator, Oscillator::Noise);
    let packet = encode_s_new(&plan(NOISE_SCORE)[0]);
    assert!(packet.ends_with(b"osc\0\0\0\0\x03"));
}

#[test]
fn parses_soft_preset_as_regular_tone_event() {
    let score = parse(include_str!("../../composer/examples/soft_preset.ndjson"));
    assert_eq!((score.bpm, score.events.len()), (120.0, 4));
    let tone = score.bass_tone;
    let envelope = (tone.attack_seconds, tone.decay_seconds, tone.sustain_level);
    assert_eq!((tone.oscillator, envelope, tone.gain), (Oscillator::Sine, (0.02, 0.08, 0.65), 1.6));
}

#[test]
fn beat_conversion_and_pitch() {
    let transport = Transport::new(120.0).unwrap();
    assert_eq!(transport.beat_time(2.0).unwrap(), Duration::from_secs(1));
    assert_close(midi_to_frequency(69), 440.0, 0.00001);
    assert_close(midi_to_frequency(36), 65.40639, 0.001);
}

const UNORDERED_NOTES: &str = concat!(
    "{\"type\":\"tempo\",\"bpm\":120}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":43,\"beat\":1,\"duration_beats\":0.5,\"velocity\":0.8}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":40,\"beat\":0,\"duration_beats\":0.5,\"velocity\":0.8}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":36,\"beat\":0,\"duration_beats\":0.5,\"velocity\":0.8}\n"
);

#[test]
fn equal_beat_order_is_stable() {
    assert_eq!(notes_of(&parse(UNORDERED_NOTES)), [40, 36, 43]);
}

const MELODY_CHORDS: &str = include_str!("../../composer/examples/melody_chords.ndjson");

#[test]
fn melody_and_chords_expand_to_simultaneous_notes() {
    let score = parse(MELODY_CHORDS);
    assert_eq!(score.events.len(), 10);
    assert_eq!(notes_of(&score)[..4], [48, 52, 55, 64]);
}

#[test]
fn melody_and_chords_schedule_in_parallel() {
    let plan = plan(MELODY_CHORDS);
    assert_eq!(millis(&plan), [0, 0, 0, 0, 250, 500, 500, 500, 500, 750]);
    assert_eq!((plan[0].duration_seconds, plan[3].duration_seconds), (0.5, 0.25));
    assert_close(plan[3].frequency, midi_to_frequency(64), 0.001);
}

const STEP_BASS: &str = include_str!("../../composer/examples/step_bass.ndjson");
const STEP_NOTES: [(u8, f64); 8] =
    [(36, 0.0), (40, 0.5), (36, 1.0), (43, 1.75), (36, 2.0), (40, 2.5), (36, 3.0), (43, 3.75)];

#[test]
fn step_patterns_expand_to_ordered_note_beats() {
    let score = parse(STEP_BASS);
    let notes: Vec<_> = score.events.iter().map(|event| (event.note, event.beat)).collect();
    assert_eq!(notes, STEP_NOTES);
}

#[test]
fn step_patterns_schedule_sixteenth_steps() {
    let plan = plan(STEP_BASS);
    assert_eq!(plan[3].at, Duration::from_millis(875));
    assert_eq!(plan[7].at, Duration::from_millis(1875));
    assert_eq!(plan[0].duration_seconds, 0.1);
}

const TWO_TRACKS: &str = concat!(
    "{\"type\":\"tempo\",\"bpm\":120}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"bass\",\"note\":36,\"beat\":0,\"duration_beats\":0.5,\"velocity\":0.5}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"lead\",\"note\":64,\"beat\":0,\"duration_beats\":0.5,\"velocity\":0.5}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"bass\",\"note\":40,\"beat\":1,\"duration_beats\":0.5,\"velocity\":0.5}\n"
);

#[test]
fn named_tracks_can_be_muted_without_changing_other_notes() {
    let score = parse(TWO_TRACKS);
    assert_eq!((score.events[0].track.as_str(), score.events[1].track.as_str()), ("bass", "lead"));
    let muted = score.without_track("lead").unwrap();
    assert_eq!(notes_of(&muted), [36, 40]);
    assert_eq!(Scheduler::plan(&muted).unwrap()[1].at, Duration::from_millis(500));
    assert_eq!(score.events.len(), 3);
    assert!(score.without_track("missing").is_err());
}

#[test]
fn legacy_notes_play_on_main_which_cannot_be_muted_alone() {
    let legacy = parse(SCORE);
    assert_eq!(legacy.events[0].track, "main");
    assert!(legacy.without_track("main").is_err());
}

#[test]
fn track_names_allow_only_safe_characters() {
    let named = parse(&TWO_TRACKS.replace("\"bass\"", "\"lead_2-v1\""));
    assert_eq!(named.events[0].track, "lead_2-v1");
    for invalid in ["", "a/b", "white space"] {
        let source = TWO_TRACKS.replace("\"bass\"", &format!("\"{invalid}\""));
        assert!(Score::parse_ndjson(&source).is_err(), "accepted track {invalid:?}");
    }
}

const BASS_AND_LEAD: &str = concat!(
    "{\"type\":\"tempo\",\"bpm\":120}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"bass\",\"note\":36,\"beat\":0,\"duration_beats\":0.5,\"velocity\":0.5}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"lead\",\"note\":64,\"beat\":0,\"duration_beats\":0.5,\"velocity\":0.5}\n"
);

#[test]
fn dry_run_can_mute_one_track() {
    let output = run_cli_with_stdin(&["--dry-run", "--mute-track", "lead", "-"], BASS_AND_LEAD);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let lines = String::from_utf8(output.stdout).unwrap();
    assert_eq!(lines.lines().count(), 1);
    assert!(lines.contains("65.406Hz"));
}

#[test]
fn compiled_multi_track_example_keeps_its_bass_when_counter_is_muted() {
    let score = parse(include_str!("../../composer/examples/tracks.ndjson"));
    assert_eq!(score.events.len(), 8);
    let bass = score.without_track("counter").unwrap();
    assert_eq!(bass.events.len(), 4);
    assert!(bass.events.iter().all(|event| event.track == "bass" && event.note == 36));
    assert_eq!(millis(&Scheduler::plan(&bass).unwrap()), [0, 500, 1000, 1500]);
}

const INVALID_NOTE_EDITS: [(&str, &str); 8] = [
    ("120", "0"),
    ("\"note\":36", "\"note\":128"),
    ("\"beat\":0.0", "\"beat\":-1"),
    ("\"duration_beats\":0.5", "\"duration_beats\":0"),
    ("\"velocity\":0.8", "\"velocity\":1.1"),
    ("analog_bass", "unknown"),
    ("\"type\":\"note\"", "\"type\":\"tempo\""),
    ("\"velocity\":0.8", "\"velocity\":\"loud\""),
];

#[test]
fn rejects_bad_events_before_playback() {
    assert_rejects_edits(SCORE, &INVALID_NOTE_EDITS);
    assert!(Score::parse_ndjson("{\"type\":\"tempo\",\"bpm\":120}\n").is_err());
    assert!(Score::parse_ndjson("garbage\n").is_err());
}

const BAD_TONE_ON_LINE_3: &str = concat!(
    "{\"type\":\"tempo\",\"bpm\":120}\n",
    "\n",
    "{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"cutoff_hz\":1400,\"attack_seconds\":0.01,\"release_seconds\":0.05,\"gain\":5}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":36,\"beat\":0,\"duration_beats\":0.5,\"velocity\":0.8}\n"
);
const BAD_NOTE_ON_LINE_3: &str = concat!(
    "{\"type\":\"tempo\",\"bpm\":120}\n",
    "\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":128,\"beat\":0,\"duration_beats\":0.5,\"velocity\":0.8}\n"
);

#[test]
fn score_errors_identify_the_offending_line() {
    assert!(Score::parse_ndjson(BAD_TONE_ON_LINE_3).unwrap_err().contains("line 3: gain"));
    assert!(Score::parse_ndjson(BAD_NOTE_ON_LINE_3).unwrap_err().contains("line 3: note"));
}

const PULSE_TONE: BassTone = BassTone {
    oscillator: Oscillator::Pulse,
    cutoff_hz: 1800.0,
    attack_seconds: 0.005,
    decay_seconds: 0.12,
    sustain_level: 0.25,
    release_seconds: 0.06,
    lfo_rate_hz: 2.0,
    lfo_depth: 0.9,
    gain: 2.6,
};

const PULSE_NOTE: ScheduledNote = ScheduledNote {
    at: Duration::ZERO,
    frequency: 440.0,
    duration_seconds: 0.5,
    amplitude: 0.16,
    tone: PULSE_TONE,
    pan: None,
};

fn pulse_note_packet() -> Vec<u8> {
    encode_s_new(&PULSE_NOTE)
}

/// Encodes an OSC string: NUL-terminated and padded to a 4-byte boundary.
fn osc_string(value: &str) -> Vec<u8> {
    let mut out = value.as_bytes().to_vec();
    out.resize(value.len() / 4 * 4 + 4, 0);
    out
}

#[test]
fn s_new_has_address_tags_definition_and_head_placement() {
    let bytes = pulse_note_packet();
    assert_eq!(&bytes[..8], b"/s_new\0\0");
    assert!(contains(&bytes, &osc_string(",siiisfsfsfsfsfsfsfsfsfsfsfsi")));
    assert!(contains(&bytes, &osc_string("analog_bass")));
    assert!(contains(&bytes, b"\xff\xff\xff\xff\0\0\0\0\0\0\0\0"));
    assert_eq!(bytes.len() % 4, 0);
}

#[test]
fn s_new_carries_tone_controls_and_the_oscillator_id() {
    let bytes = pulse_note_packet();
    for name in ["cutoff", "gain", "sustain", "lfoRate", "lfoDepth"] {
        assert!(contains(&bytes, &osc_string(name)), "missing {name}");
    }
    for value in [440.0_f32, 1800.0, 2.6, 0.25, 2.0] {
        assert!(contains(&bytes, &value.to_be_bytes()), "missing {value}");
    }
    assert!(bytes.ends_with(b"osc\0\0\0\0\x02"));
}

#[test]
fn load_and_sync_messages_are_padded_osc() {
    let load = encode_load("/tmp/analog_bass.scsyndef");
    assert_eq!(&load[..8], b"/d_load\0");
    assert_eq!(load.len() % 4, 0);
    assert_eq!(encode_sync(1), b"/sync\0\0\0,i\0\0\0\0\0\x01");
}

const TRACK_TONES: &str = concat!(
    "{\"type\":\"tempo\",\"bpm\":120}\n",
    "{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"oscillator\":\"pulse\",\"cutoff_hz\":900.0,\"attack_seconds\":0.010,\"release_seconds\":0.080,\"gain\":1.500}\n",
    "{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"track\":\"bass\",\"oscillator\":\"saw\",\"cutoff_hz\":1200.0,\"attack_seconds\":0.005,\"decay_seconds\":0.040,\"sustain_level\":0.800,\"release_seconds\":0.060,\"lfo_rate_hz\":4.000,\"lfo_depth\":0.900,\"gain\":3.500}\n",
    "{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"track\":\"chords\",\"oscillator\":\"sine\",\"cutoff_hz\":1600.0,\"attack_seconds\":0.020,\"decay_seconds\":0.080,\"sustain_level\":0.650,\"release_seconds\":0.100,\"gain\":1.600}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"bass\",\"note\":36,\"beat\":0.0,\"duration_beats\":0.5,\"velocity\":0.7}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"chords\",\"note\":60,\"beat\":0.0,\"duration_beats\":0.5,\"velocity\":0.35}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":67,\"beat\":1.0,\"duration_beats\":0.5,\"velocity\":0.5}\n"
);
const DUPLICATE_TRACK_TONE: &str = concat!(
    "{\"type\":\"tempo\",\"bpm\":120}\n",
    "{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"track\":\"bass\",\"cutoff_hz\":1200.0,\"attack_seconds\":0.01,\"release_seconds\":0.05}\n",
    "{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"track\":\"bass\",\"cutoff_hz\":1400.0,\"attack_seconds\":0.01,\"release_seconds\":0.05}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"bass\",\"note\":36,\"beat\":0.0,\"duration_beats\":0.5,\"velocity\":0.8}\n"
);
const BAD_TRACK_NAME_TONE: &str = concat!(
    "{\"type\":\"tempo\",\"bpm\":120}\n",
    "{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"track\":\"bad/name\",\"cutoff_hz\":1200.0,\"attack_seconds\":0.01,\"release_seconds\":0.05}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":36,\"beat\":0.0,\"duration_beats\":0.5,\"velocity\":0.8}\n"
);

fn tone_summary(notes: &[ScheduledNote]) -> Vec<(Oscillator, f32, f32)> {
    notes.iter().map(|note| (note.tone.oscillator, note.tone.cutoff_hz, note.tone.gain)).collect()
}

#[test]
fn per_track_tones_apply_to_matching_tracks_and_fall_back_to_default_tone() {
    let notes = plan(TRACK_TONES);
    let expected = [
        (Oscillator::Saw, 1200.0, 3.5),
        (Oscillator::Sine, 1600.0, 1.6),
        (Oscillator::Pulse, 900.0, 1.5),
    ];
    assert_eq!(tone_summary(&notes), expected);
    assert_eq!(notes[0].tone.lfo_depth, 0.9);
}

#[test]
fn live_bars_keep_per_track_tones() {
    let bar = LiveScheduler::new(parse(TRACK_TONES)).next_bar().unwrap();
    let oscillators: Vec<_> = bar.notes.iter().map(|note| note.tone.oscillator).collect();
    assert_eq!(oscillators, [Oscillator::Saw, Oscillator::Sine, Oscillator::Pulse]);
}

#[test]
fn rejects_duplicate_track_tones_and_bad_tone_track_names() {
    assert!(Score::parse_ndjson(DUPLICATE_TRACK_TONE).is_err());
    assert!(Score::parse_ndjson(BAD_TRACK_NAME_TONE).is_err());
}

#[test]
fn simultaneous_notes_group_in_time_order() {
    assert!(group_notes_by_time(&[]).next().is_none());
    let notes = plan(MELODY_CHORDS);
    let groups: Vec<_> = group_notes_by_time(&notes).collect();
    let shape: Vec<_> = groups.iter().map(|(at, group)| (at.as_millis(), group.len())).collect();
    assert_eq!(shape, [(0, 4), (250, 1), (500, 4), (750, 1)]);
    let flattened: Vec<_> = groups.iter().flat_map(|(_, group)| group.iter()).collect();
    assert_eq!(flattened, notes.iter().collect::<Vec<_>>());
}

#[test]
fn bundles_carry_the_timetag_and_length_prefixed_messages() {
    let chord = &plan(MELODY_CHORDS)[..4];
    let bundle = encode_bundle(OSC_IMMEDIATE_TIMETAG, chord);
    assert_eq!(&bundle[..8], b"#bundle\0");
    assert_eq!(u64::from_be_bytes(bundle[8..16].try_into().unwrap()), OSC_IMMEDIATE_TIMETAG);
    let first = encode_s_new(&chord[0]);
    assert_eq!(i32::from_be_bytes(bundle[16..20].try_into().unwrap()) as usize, first.len());
    assert_eq!(&bundle[20..20 + first.len()], first.as_slice());
    assert_eq!(bundle.len(), 16 + 4 * (4 + first.len()));
}

#[test]
fn ntp_timetags_count_from_1900_with_32_bit_fractions() {
    let ntp = system_time_to_ntp(std::time::UNIX_EPOCH + Duration::from_millis(1500));
    assert_eq!(ntp >> 32, 2_208_988_801);
    assert!(((ntp & 0xffff_ffff) as i64 - 0x8000_0000_i64).abs() <= 2);
}
