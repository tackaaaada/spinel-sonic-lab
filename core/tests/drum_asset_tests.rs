//! Contract of the rendered drum WAVs in `samples/`; no audio server needed.

mod common;

use std::path::Path;

use common::audio::{Stereo, db, read_wav};

/// 10 ms at 48kHz.
const EDGE: f64 = 0.01;

#[test]
fn drum_wavs_are_short_audible_unclipped_and_finish_in_silence() {
    for (name, seconds) in [("kick", 0.35), ("snare", 0.30), ("hat", 0.18)] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../samples/{name}.wav"));
        assert_drum(name, &Stereo::from_asset(&read_wav(&path)), seconds);
    }
}

/// NRT block boundaries may lengthen a file by a few milliseconds.
fn assert_drum(name: &str, audio: &Stereo, seconds: f64) {
    let length = audio.left.len() as f64 / 48_000.0;
    assert!((length - seconds).abs() <= 0.003, "{name}: {length} s");
    let (peak, rms) = (audio.peak(), audio.rms());
    assert!(0.05 < peak && peak < 0.95 && rms > 0.005, "{name}: {peak} / {rms}");
    assert!(audio.window(0.0, EDGE).peak() > 100.0 / 32768.0, "{name}: no onset");
    assert!(audio.window(length - EDGE, length).peak() <= 2.0 / 32768.0, "{name}: tail");
    println!("{name}: {length:.4} s, peak {:.2} dBFS, RMS {:.2} dBFS", db(peak), db(rms));
}
