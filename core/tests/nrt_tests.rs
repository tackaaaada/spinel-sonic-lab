mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use common::audio::read_wav;
use common::contains;
use spinel_sonic_lab::mixer::REVERB_TAIL;
use spinel_sonic_lab::nrt::{NrtScore, elapsed_timetag, score_commands};
use spinel_sonic_lab::osc::{encode_graph_groups, encode_load, encode_playback_bundle};
use spinel_sonic_lab::schedule::Scheduler;
use spinel_sonic_lab::source::prepare_score;

#[test]
fn nrt_timetags_count_seconds_from_the_score_start_in_32_32_fixed_point() {
    assert_eq!(elapsed_timetag(Duration::ZERO), 0);
    assert_eq!(elapsed_timetag(Duration::from_millis(500)), 0x8000_0000);
    assert_eq!(elapsed_timetag(Duration::from_millis(1250)), 0x1_4000_0000);
}

#[test]
fn nrt_scores_prefix_each_bundle_with_its_big_endian_length() {
    let mut score = NrtScore::default();
    score.push_bundle(b"#bundle\0abcdefgh");
    score.push_bundle(b"#bundle\0ijklmnop1234");
    let mut expected = 16u32.to_be_bytes().to_vec();
    expected.extend_from_slice(b"#bundle\0abcdefgh");
    expected.extend_from_slice(&20u32.to_be_bytes());
    expected.extend_from_slice(b"#bundle\0ijklmnop1234");
    assert_eq!(score.bytes(), expected);
}

const ONE_NOTE: &str = concat!(
    "{\"type\":\"tempo\",\"bpm\":60}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":48,\"beat\":1.0,\"duration_beats\":0.5,\"velocity\":0.5}\n",
);

const STRIP_AND_SAMPLE: &str = concat!(
    "{\"type\":\"tempo\",\"bpm\":60}\n",
    "{\"type\":\"sample\",\"name\":\"kick\",\"path\":\"samples/kick.wav\"}\n",
    "{\"type\":\"aux_return\",\"name\":\"room\",\"reverb\":{\"room\":0.6,\"damp\":0.4,\"wet\":0.5},\"fader_db\":-6}\n",
    "{\"type\":\"channel\",\"track\":\"drums\",\"ha_gain_db\":0,\"pan\":0,\"fader_db\":-6,\"aux_send\":{\"return\":\"room\",\"level_db\":-18}}\n",
    "{\"type\":\"sample_note\",\"sample\":\"kick\",\"track\":\"drums\",\"beat\":0.0,\"velocity\":0.7}\n",
);

fn project_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn synthdef_dir() -> PathBuf {
    project_dir().join("synthdefs")
}

fn record(ndjson: &str, length: Option<Duration>) -> Vec<(u64, Vec<u8>)> {
    let score = prepare_score(ndjson, &project_dir(), None).unwrap();
    bundles(&score_commands(&score, &synthdef_dir(), length).unwrap())
}

/// Splits an NRT command file into `(timetag, bundle)` pairs.
fn bundles(score: &NrtScore) -> Vec<(u64, Vec<u8>)> {
    let (mut bytes, mut out) = (score.bytes(), Vec::new());
    while !bytes.is_empty() {
        let length = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
        let bundle = bytes[4..4 + length].to_vec();
        out.push((u64::from_be_bytes(bundle[8..16].try_into().unwrap()), bundle));
        bytes = &bytes[4 + length..];
    }
    out
}

#[test]
fn notes_are_encoded_by_the_live_encoder_at_their_elapsed_time() {
    let score = prepare_score(ONE_NOTE, &project_dir(), None).unwrap();
    let plan = Scheduler::plan_playback(&score).unwrap();
    let at = elapsed_timetag(Duration::from_secs(1));
    let live = encode_playback_bundle(at, &plan, |_| unreachable!()).unwrap();
    let recorded = bundles(&score_commands(&score, &synthdef_dir(), None).unwrap());
    assert!(recorded.contains(&(at, live)));
}

#[test]
fn definitions_load_at_time_zero_and_the_file_ends_after_the_last_note() {
    let recorded = record(ONE_NOTE, None);
    let load = encode_load(synthdef_dir().join("analog_bass.scsyndef").to_str().unwrap());
    assert!(recorded[0].0 == 0 && contains(&recorded[0].1, &load));
    assert_eq!(recorded.last().unwrap().0, elapsed_timetag(Duration::from_millis(1500)));
}

#[test]
fn a_fixed_length_overrides_the_natural_end() {
    let recorded = record(ONE_NOTE, Some(Duration::from_secs(4)));
    assert_eq!(recorded.last().unwrap().0, elapsed_timetag(Duration::from_secs(4)));
}

#[test]
fn strips_and_buffers_are_prepared_before_the_first_event() {
    let recorded = record(STRIP_AND_SAMPLE, None);
    let first_event = recorded.iter().position(|(_, b)| contains(b, b"sample_stereo\0")).unwrap();
    let setup = &recorded[..first_event];
    let parts =
        [&encode_graph_groups()[..], b"channel_strip\0", b"aux_return\0", b"/b_allocRead\0"];
    parts.iter().for_each(|part| assert_set_up(setup, part));
    assert!(contains(&recorded[first_event].1, b"out\0"), "the sample must play into its strip");
}

fn assert_set_up(setup: &[(u64, Vec<u8>)], part: &[u8]) {
    let found = setup.iter().any(|(_, bundle)| contains(bundle, part));
    assert!(found, "missing before the first event: {}", String::from_utf8_lossy(part));
}

#[test]
fn the_reverb_tail_extends_the_natural_end() {
    let recorded = record(STRIP_AND_SAMPLE, None);
    let kick = read_wav(&project_dir().join("samples/kick.wav")).duration();
    assert_eq!(recorded.last().unwrap().0, elapsed_timetag(kick + REVERB_TAIL));
}
