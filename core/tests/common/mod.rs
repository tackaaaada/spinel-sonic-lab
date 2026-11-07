//! Helpers shared by the integration test crates; each crate uses a subset.
#![allow(dead_code)]

pub mod audio;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

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

/// A uniquely named directory under the system temp dir, removed on drop.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn create(prefix: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let count = NEXT.fetch_add(1, Ordering::Relaxed);
        let name = format!("{prefix}-{}-{count}", std::process::id());
        let path = std::env::temp_dir().join(name);
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
