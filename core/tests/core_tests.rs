use std::time::Duration;
use std::{
    io::Write,
    process::{Command, Stdio},
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

#[test]
fn parses_score_and_schedules_note() {
    let score = Score::parse_ndjson(SCORE).unwrap();
    assert_eq!(score.bpm, 120.0);
    assert_eq!(score.bass_tone, BassTone::default());
    assert_eq!(score.bass_tone.gain, 2.8);
    assert_eq!(score.events[0].note, 36);
    let notes = Scheduler::plan(&score).unwrap();
    assert_eq!(notes[0].at, Duration::ZERO);
    assert_eq!(notes[0].duration_seconds, 0.25);
    assert!((notes[0].amplitude - 0.16).abs() < 0.00001);
}

#[test]
fn parses_tone_and_rejects_invalid_settings() {
    let source = include_str!("../../composer/examples/bright_bass.ndjson");
    let score = Score::parse_ndjson(source).unwrap();
    assert_eq!(score.bass_tone.cutoff_hz, 1800.0);
    assert_eq!(score.bass_tone.oscillator, Oscillator::Saw);
    assert_eq!(score.bass_tone.attack_seconds, 0.005);
    assert_eq!(score.bass_tone.release_seconds, 0.06);
    assert_eq!(score.bass_tone.decay_seconds, 0.0);
    assert_eq!(score.bass_tone.sustain_level, 1.0);
    assert_eq!(score.bass_tone.lfo_rate_hz, 0.0);
    assert_eq!(score.bass_tone.lfo_depth, 0.0);
    assert_eq!(score.bass_tone.gain, 2.6);
    let previous_tone = source.replace("\"oscillator\":\"saw\",", "");
    assert_eq!(
        Score::parse_ndjson(&previous_tone)
            .unwrap()
            .bass_tone
            .oscillator,
        Oscillator::Saw
    );
    let previous_gain = source.replace(",\"gain\":2.600", "");
    assert_eq!(
        Score::parse_ndjson(&previous_gain).unwrap().bass_tone.gain,
        1.0
    );
    for invalid in [
        source.replace("1800.0", "20000.0"),
        source.replace("0.005", "-0.1"),
        source.replace("0.060", "6.0"),
        source.replace("analog_bass", "unknown"),
        source.replace("\"cutoff_hz\":1800.0,", ""),
        source.replace("\"oscillator\":\"saw\"", "\"oscillator\":\"triangle\""),
        source.replace("\"oscillator\":\"saw\"", "\"oscillator\":5"),
        source.replace("\"gain\":2.600", "\"gain\":-1"),
        source.replace("\"gain\":2.600", "\"gain\":4.1"),
        source.replace("\"gain\":2.600", "\"gain\":\"loud\""),
        source.replace(
            "{\"type\":\"tempo\",\"bpm\":120}\n",
            "{\"type\":\"tempo\",\"bpm\":120}\n{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"cutoff_hz\":800,\"attack_seconds\":0.01,\"release_seconds\":0.09}\n",
        ),
    ] {
        assert!(Score::parse_ndjson(&invalid).is_err(), "accepted: {invalid}");
    }
    let lines: Vec<_> = source.lines().collect();
    let after_notes = format!("{}\n{}\n{}\n", lines[0], lines[2], lines[1]);
    assert!(Score::parse_ndjson(&after_notes).is_err());
}

#[test]
fn parses_lfo_and_rejects_invalid_modulation() {
    let source = include_str!("../../composer/examples/lfo_bass.ndjson");
    let score = Score::parse_ndjson(source).unwrap();
    assert_eq!(score.bass_tone.lfo_rate_hz, 4.0);
    assert_eq!(score.bass_tone.lfo_depth, 0.9);
    let reference =
        Score::parse_ndjson(include_str!("../../composer/examples/adsr_bass.ndjson")).unwrap();
    assert_eq!(score.bpm, reference.bpm);
    assert_eq!(
        score
            .events
            .iter()
            .map(|event| (event.note, event.beat, event.duration_beats))
            .collect::<Vec<_>>(),
        reference
            .events
            .iter()
            .map(|event| (event.note, event.beat, event.duration_beats))
            .collect::<Vec<_>>()
    );
    for invalid in [
        source.replace("\"lfo_rate_hz\":4.000", "\"lfo_rate_hz\":0"),
        source.replace("\"lfo_rate_hz\":4.000", "\"lfo_rate_hz\":21"),
        source.replace("\"lfo_rate_hz\":4.000", "\"lfo_rate_hz\":\"fast\""),
        source.replace("\"lfo_depth\":0.900", "\"lfo_depth\":-0.1"),
        source.replace("\"lfo_depth\":0.900", "\"lfo_depth\":1.1"),
        source.replace(",\"lfo_rate_hz\":4.000", ""),
        source.replace(",\"lfo_depth\":0.900", ""),
    ] {
        assert!(
            Score::parse_ndjson(&invalid).is_err(),
            "accepted: {invalid}"
        );
    }
}

#[test]
fn parses_adsr_and_rejects_invalid_envelopes() {
    let source = include_str!("../../composer/examples/adsr_bass.ndjson");
    let score = Score::parse_ndjson(source).unwrap();
    assert_eq!(score.bass_tone.decay_seconds, 0.12);
    assert_eq!(score.bass_tone.sustain_level, 0.25);
    for invalid in [
        source.replace("\"decay_seconds\":0.120", "\"decay_seconds\":-1"),
        source.replace("\"decay_seconds\":0.120", "\"decay_seconds\":6"),
        source.replace("\"decay_seconds\":0.120", "\"decay_seconds\":\"slow\""),
        source.replace("\"sustain_level\":0.250", "\"sustain_level\":1.5"),
        source.replace("\"sustain_level\":0.250", "\"sustain_level\":-0.1"),
    ] {
        assert!(
            Score::parse_ndjson(&invalid).is_err(),
            "accepted: {invalid}"
        );
    }
}

#[test]
fn parses_all_oscillator_choices() {
    for (filename, expected) in [
        (
            include_str!("../../composer/examples/sine_bass.ndjson"),
            Oscillator::Sine,
        ),
        (
            include_str!("../../composer/examples/pulse_bass.ndjson"),
            Oscillator::Pulse,
        ),
    ] {
        assert_eq!(
            Score::parse_ndjson(filename).unwrap().bass_tone.oscillator,
            expected
        );
    }
    let noise_score = Score::parse_ndjson(concat!(
        "{\"type\":\"tempo\",\"bpm\":120}\n",
        "{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"oscillator\":\"noise\",\"cutoff_hz\":4500.0,\"attack_seconds\":0.001,\"decay_seconds\":0.090,\"sustain_level\":0.000,\"release_seconds\":0.050,\"gain\":2.200}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":50,\"beat\":0.0,\"duration_beats\":0.25,\"velocity\":0.8}\n"
    ))
    .unwrap();
    assert_eq!(noise_score.bass_tone.oscillator, Oscillator::Noise);
    let scheduled = Scheduler::plan(&noise_score).unwrap();
    let packet = encode_s_new(&scheduled[0]);
    assert!(packet.ends_with(b"osc\0\0\0\0\x03"));
}

#[test]
fn parses_soft_preset_as_regular_tone_event() {
    let source = include_str!("../../composer/examples/soft_preset.ndjson");
    let score = Score::parse_ndjson(source).unwrap();
    assert_eq!(score.bpm, 120.0);
    assert_eq!(score.bass_tone.oscillator, Oscillator::Sine);
    assert_eq!(score.bass_tone.attack_seconds, 0.02);
    assert_eq!(score.bass_tone.decay_seconds, 0.08);
    assert_eq!(score.bass_tone.sustain_level, 0.65);
    assert_eq!(score.bass_tone.gain, 1.6);
    assert_eq!(score.events.len(), 4);
}

#[test]
fn beat_conversion_and_pitch() {
    let transport = Transport::new(120.0).unwrap();
    assert_eq!(transport.beat_time(2.0).unwrap(), Duration::from_secs(1));
    assert!((midi_to_frequency(69) - 440.0).abs() < 0.00001);
    assert!((midi_to_frequency(36) - 65.40639).abs() < 0.001);
}

#[test]
fn equal_beat_order_is_stable() {
    let score = Score::parse_ndjson(concat!(
        "{\"type\":\"tempo\",\"bpm\":120}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":43,\"beat\":1,\"duration_beats\":0.5,\"velocity\":0.8}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":40,\"beat\":0,\"duration_beats\":0.5,\"velocity\":0.8}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":36,\"beat\":0,\"duration_beats\":0.5,\"velocity\":0.8}\n"
    )).unwrap();
    assert_eq!(
        score.events.iter().map(|e| e.note).collect::<Vec<_>>(),
        vec![40, 36, 43]
    );
}

#[test]
fn melody_and_chords_schedule_in_parallel() {
    let source = include_str!("../../composer/examples/melody_chords.ndjson");
    let score = Score::parse_ndjson(source).unwrap();
    assert_eq!(score.events.len(), 10);
    assert_eq!(
        score
            .events
            .iter()
            .take(4)
            .map(|event| event.note)
            .collect::<Vec<_>>(),
        vec![48, 52, 55, 64]
    );
    let plan = Scheduler::plan(&score).unwrap();
    assert!(plan.iter().take(4).all(|note| note.at == Duration::ZERO));
    assert_eq!(plan[4].at, Duration::from_millis(250));
    assert!(
        plan[5..9]
            .iter()
            .all(|note| note.at == Duration::from_millis(500))
    );
    assert_eq!(plan[9].at, Duration::from_millis(750));
    assert_eq!(plan[0].duration_seconds, 0.5);
    assert_eq!(plan[3].duration_seconds, 0.25);
    assert!((plan[3].frequency - midi_to_frequency(64)).abs() < 0.001);
}

#[test]
fn step_patterns_expand_to_ordered_note_times() {
    let source = include_str!("../../composer/examples/step_bass.ndjson");
    let score = Score::parse_ndjson(source).unwrap();
    assert_eq!(score.events.len(), 8);
    assert_eq!(
        score
            .events
            .iter()
            .map(|event| (event.note, event.beat))
            .collect::<Vec<_>>(),
        vec![
            (36, 0.0),
            (40, 0.5),
            (36, 1.0),
            (43, 1.75),
            (36, 2.0),
            (40, 2.5),
            (36, 3.0),
            (43, 3.75),
        ]
    );
    let plan = Scheduler::plan(&score).unwrap();
    assert_eq!(plan[3].at, Duration::from_millis(875));
    assert_eq!(plan[7].at, Duration::from_millis(1875));
    assert_eq!(plan[0].duration_seconds, 0.1);
}

#[test]
fn named_tracks_can_be_muted_without_changing_other_notes() {
    let source = concat!(
        "{\"type\":\"tempo\",\"bpm\":120}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"bass\",\"note\":36,\"beat\":0,\"duration_beats\":0.5,\"velocity\":0.5}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"lead\",\"note\":64,\"beat\":0,\"duration_beats\":0.5,\"velocity\":0.5}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"bass\",\"note\":40,\"beat\":1,\"duration_beats\":0.5,\"velocity\":0.5}\n"
    );
    let score = Score::parse_ndjson(source).unwrap();
    assert_eq!(score.events[0].track, "bass");
    assert_eq!(score.events[1].track, "lead");
    let muted = score.without_track("lead").unwrap();
    assert_eq!(
        muted
            .events
            .iter()
            .map(|event| event.note)
            .collect::<Vec<_>>(),
        vec![36, 40]
    );
    assert_eq!(
        Scheduler::plan(&muted).unwrap()[1].at,
        Duration::from_millis(500)
    );
    assert_eq!(score.events.len(), 3);
    assert!(score.without_track("missing").is_err());

    let legacy = Score::parse_ndjson(SCORE).unwrap();
    assert_eq!(legacy.events[0].track, "main");
    assert!(legacy.without_track("main").is_err());
    let named = Score::parse_ndjson(&source.replace("\"bass\"", "\"lead_2-v1\"")).unwrap();
    assert_eq!(named.events[0].track, "lead_2-v1");
    for invalid in ["", "a/b", "white space"] {
        assert!(
            Score::parse_ndjson(&source.replace("\"bass\"", &format!("\"{invalid}\""))).is_err()
        );
    }
}

#[test]
fn dry_run_can_mute_one_track() {
    let source = concat!(
        "{\"type\":\"tempo\",\"bpm\":120}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"bass\",\"note\":36,\"beat\":0,\"duration_beats\":0.5,\"velocity\":0.5}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"lead\",\"note\":64,\"beat\":0,\"duration_beats\":0.5,\"velocity\":0.5}\n"
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_spinel-sonic-lab"))
        .args(["--dry-run", "--mute-track", "lead", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lines = String::from_utf8(output.stdout).unwrap();
    assert_eq!(lines.lines().count(), 1);
    assert!(lines.contains("65.406Hz"));
}

#[test]
fn compiled_multi_track_example_keeps_its_bass_when_counter_is_muted() {
    let score = Score::parse_ndjson(include_str!("../../composer/examples/tracks.ndjson")).unwrap();
    assert_eq!(score.events.len(), 8);
    let bass = score.without_track("counter").unwrap();
    assert_eq!(bass.events.len(), 4);
    assert!(
        bass.events
            .iter()
            .all(|event| event.track == "bass" && event.note == 36)
    );
    assert_eq!(
        Scheduler::plan(&bass)
            .unwrap()
            .iter()
            .map(|note| note.at)
            .collect::<Vec<_>>(),
        vec![
            Duration::ZERO,
            Duration::from_millis(500),
            Duration::from_secs(1),
            Duration::from_millis(1500)
        ]
    );
}

#[test]
fn rejects_bad_events_before_playback() {
    for invalid in [
        SCORE.replace("120", "0"),
        SCORE.replace("\"note\":36", "\"note\":128"),
        SCORE.replace("\"beat\":0.0", "\"beat\":-1"),
        SCORE.replace("\"duration_beats\":0.5", "\"duration_beats\":0"),
        SCORE.replace("\"velocity\":0.8", "\"velocity\":1.1"),
        SCORE.replace("analog_bass", "unknown"),
        SCORE.replace("\"type\":\"note\"", "\"type\":\"tempo\""),
        SCORE.replace("\"velocity\":0.8", "\"velocity\":\"loud\""),
    ] {
        assert!(
            Score::parse_ndjson(&invalid).is_err(),
            "accepted: {invalid}"
        );
    }
    assert!(Score::parse_ndjson("{\"type\":\"tempo\",\"bpm\":120}\n").is_err());
    assert!(Score::parse_ndjson("garbage\n").is_err());
}

#[test]
fn score_errors_identify_the_offending_line() {
    let bad_tone = concat!(
        "{\"type\":\"tempo\",\"bpm\":120}\n",
        "\n",
        "{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"cutoff_hz\":1400,\"attack_seconds\":0.01,\"release_seconds\":0.05,\"gain\":5}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":36,\"beat\":0,\"duration_beats\":0.5,\"velocity\":0.8}\n"
    );
    assert!(
        Score::parse_ndjson(bad_tone)
            .unwrap_err()
            .contains("line 3: gain")
    );

    let bad_note = concat!(
        "{\"type\":\"tempo\",\"bpm\":120}\n",
        "\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":128,\"beat\":0,\"duration_beats\":0.5,\"velocity\":0.8}\n"
    );
    assert!(
        Score::parse_ndjson(bad_note)
            .unwrap_err()
            .contains("line 3: note")
    );
}

#[test]
fn osc_messages_have_expected_structure() {
    let tone = BassTone {
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
    let note = ScheduledNote {
        at: Duration::ZERO,
        frequency: 440.0,
        duration_seconds: 0.5,
        amplitude: 0.16,
        tone,
    };
    let bytes = encode_s_new(&note);
    assert_eq!(&bytes[..8], b"/s_new\0\0");
    assert!(
        bytes
            .windows(32)
            .any(|part| part == b",siiisfsfsfsfsfsfsfsfsfsfsfsi\0\0\0")
    );
    assert!(bytes.windows(12).any(|part| part == b"analog_bass\0"));
    assert!(bytes.windows(4).any(|part| part == 440.0_f32.to_be_bytes()));
    assert!(
        bytes
            .windows(4)
            .any(|part| part == 1800.0_f32.to_be_bytes())
    );
    assert!(bytes.windows(8).any(|part| part == b"cutoff\0\0"));
    assert!(bytes.windows(8).any(|part| part == b"gain\0\0\0\0"));
    assert!(bytes.windows(4).any(|part| part == 2.6_f32.to_be_bytes()));
    assert!(bytes.windows(8).any(|part| part == b"sustain\0"));
    assert!(bytes.windows(4).any(|part| part == 0.25_f32.to_be_bytes()));
    assert!(bytes.windows(8).any(|part| part == b"lfoRate\0"));
    assert!(bytes.windows(12).any(|part| part == b"lfoDepth\0\0\0\0"));
    assert!(bytes.windows(4).any(|part| part == 2.0_f32.to_be_bytes()));
    assert!(bytes.ends_with(b"osc\0\0\0\0\x02"));
    assert!(
        bytes
            .windows(12)
            .any(|part| part == b"\xff\xff\xff\xff\0\0\0\0\0\0\0\0")
    );
    assert_eq!(bytes.len() % 4, 0);
    let load = encode_load("/tmp/analog_bass.scsyndef");
    assert_eq!(&load[..8], b"/d_load\0");
    assert_eq!(load.len() % 4, 0);
    assert_eq!(encode_sync(1), b"/sync\0\0\0,i\0\0\0\0\0\x01");
}

#[test]
fn per_track_tones_apply_to_matching_tracks_and_fall_back_to_default_tone() {
    let source = concat!(
        "{\"type\":\"tempo\",\"bpm\":120}\n",
        "{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"oscillator\":\"pulse\",\"cutoff_hz\":900.0,\"attack_seconds\":0.010,\"release_seconds\":0.080,\"gain\":1.500}\n",
        "{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"track\":\"bass\",\"oscillator\":\"saw\",\"cutoff_hz\":1200.0,\"attack_seconds\":0.005,\"decay_seconds\":0.040,\"sustain_level\":0.800,\"release_seconds\":0.060,\"lfo_rate_hz\":4.000,\"lfo_depth\":0.900,\"gain\":3.500}\n",
        "{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"track\":\"chords\",\"oscillator\":\"sine\",\"cutoff_hz\":1600.0,\"attack_seconds\":0.020,\"decay_seconds\":0.080,\"sustain_level\":0.650,\"release_seconds\":0.100,\"gain\":1.600}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"bass\",\"note\":36,\"beat\":0.0,\"duration_beats\":0.5,\"velocity\":0.7}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"chords\",\"note\":60,\"beat\":0.0,\"duration_beats\":0.5,\"velocity\":0.35}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":67,\"beat\":1.0,\"duration_beats\":0.5,\"velocity\":0.5}\n"
    );
    let score = Score::parse_ndjson(source).unwrap();
    let notes = Scheduler::plan(&score).unwrap();
    assert_eq!(notes.len(), 3);
    assert_eq!(notes[0].tone.oscillator, Oscillator::Saw);
    assert_eq!(notes[0].tone.lfo_depth, 0.9);
    assert_eq!(notes[0].tone.gain, 3.5);
    assert_eq!(notes[1].tone.oscillator, Oscillator::Sine);
    assert_eq!(notes[1].tone.cutoff_hz, 1600.0);
    assert_eq!(notes[1].tone.gain, 1.6);
    assert_eq!(notes[2].tone.oscillator, Oscillator::Pulse);
    assert_eq!(notes[2].tone.cutoff_hz, 900.0);
    assert_eq!(notes[2].tone.gain, 1.5);

    let mut live = LiveScheduler::new(score);
    let bar = live.next_bar().unwrap();
    assert_eq!(bar.notes[0].tone.oscillator, Oscillator::Saw);
    assert_eq!(bar.notes[1].tone.oscillator, Oscillator::Sine);
    assert_eq!(bar.notes[2].tone.oscillator, Oscillator::Pulse);

    // Duplicate tone for the same track or invalid track name on tone is rejected.
    let duplicate_track_tone = concat!(
        "{\"type\":\"tempo\",\"bpm\":120}\n",
        "{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"track\":\"bass\",\"cutoff_hz\":1200.0,\"attack_seconds\":0.01,\"release_seconds\":0.05}\n",
        "{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"track\":\"bass\",\"cutoff_hz\":1400.0,\"attack_seconds\":0.01,\"release_seconds\":0.05}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"bass\",\"note\":36,\"beat\":0.0,\"duration_beats\":0.5,\"velocity\":0.8}\n"
    );
    assert!(Score::parse_ndjson(duplicate_track_tone).is_err());

    let bad_track_name_tone = concat!(
        "{\"type\":\"tempo\",\"bpm\":120}\n",
        "{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"track\":\"bad/name\",\"cutoff_hz\":1200.0,\"attack_seconds\":0.01,\"release_seconds\":0.05}\n",
        "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":36,\"beat\":0.0,\"duration_beats\":0.5,\"velocity\":0.8}\n"
    );
    assert!(Score::parse_ndjson(bad_track_name_tone).is_err());
}

#[test]
fn osc_bundles_group_simultaneous_notes_and_encode_ntp_timetags() {
    assert!(group_notes_by_time(&[]).next().is_none());
    let source = include_str!("../../composer/examples/melody_chords.ndjson");
    let score = Score::parse_ndjson(source).unwrap();
    let notes = Scheduler::plan(&score).unwrap();
    let groups: Vec<_> = group_notes_by_time(&notes).collect();

    // melody_chords has 4 notes at 0.0s, 1 at 0.25s, 4 at 0.5s, 1 at 0.75s.
    assert_eq!(groups.len(), 4);
    assert_eq!(groups[0].0, Duration::ZERO);
    assert_eq!(groups[0].1.len(), 4);
    assert_eq!(groups[1].0, Duration::from_millis(250));
    assert_eq!(groups[1].1.len(), 1);
    assert_eq!(groups[2].0, Duration::from_millis(500));
    assert_eq!(groups[2].1.len(), 4);
    assert_eq!(groups[3].0, Duration::from_millis(750));
    assert_eq!(groups[3].1.len(), 1);
    assert_eq!(
        groups
            .iter()
            .flat_map(|(_, group)| group.iter())
            .collect::<Vec<_>>(),
        notes.iter().collect::<Vec<_>>()
    );

    let bundle = encode_bundle(OSC_IMMEDIATE_TIMETAG, groups[0].1);
    assert_eq!(&bundle[..8], b"#bundle\0");
    assert_eq!(
        u64::from_be_bytes(bundle[8..16].try_into().unwrap()),
        OSC_IMMEDIATE_TIMETAG
    );
    let first_msg = encode_s_new(&groups[0].1[0]);
    let first_len = i32::from_be_bytes(bundle[16..20].try_into().unwrap()) as usize;
    assert_eq!(first_len, first_msg.len());
    assert_eq!(&bundle[20..20 + first_len], first_msg.as_slice());
    assert_eq!(bundle.len(), 16 + 4 * (4 + first_msg.len()));
    assert_eq!(bundle.len() % 4, 0);

    let ntp = system_time_to_ntp(std::time::UNIX_EPOCH + Duration::from_millis(1500));
    let ntp_seconds = ntp >> 32;
    let ntp_fraction = ntp & 0xffff_ffff;
    assert_eq!(ntp_seconds, 2_208_988_801);
    assert!((ntp_fraction as i64 - 0x8000_0000_i64).abs() <= 2);
}
