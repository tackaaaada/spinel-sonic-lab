//! Helpers shared by the integration test crates; each crate uses a subset.
#![allow(dead_code)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};

use spinel_sonic_lab::model::Score;

pub fn parse(source: &str) -> Score {
    Score::parse_ndjson(source).unwrap()
}

pub fn assert_close(actual: f32, expected: f32, tolerance: f32) {
    assert!((actual - expected).abs() < tolerance, "{actual} != {expected}");
}

pub fn contains(packet: &[u8], part: &[u8]) -> bool {
    packet.windows(part.len()).any(|window| window == part)
}

/// Applies each `(from, to)` edit to `source` on its own and expects a rejection.
pub fn assert_rejects_edits(source: &str, edits: &[(&str, &str)]) {
    for (from, to) in edits {
        let invalid = source.replace(from, to);
        assert!(Score::parse_ndjson(&invalid).is_err(), "accepted: {invalid}");
    }
}

pub fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../composer/examples")
}

/// Starts the CLI with every standard stream piped.
pub fn spawn_cli(args: &[&str]) -> Child {
    Command::new(env!("CARGO_BIN_EXE_spinel-sonic-lab"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

pub fn run_cli_with_stdin(args: &[&str], stdin: &str) -> Output {
    let mut child = spawn_cli(args);
    child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}
