use std::env;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::Score;
use crate::schedule::parse_and_validate_score;

pub(crate) fn compile_and_validate_score(
    path: &Path,
    source: &str,
    mute_track: Option<&str>,
) -> Result<Score, String> {
    let ndjson = source_to_ndjson(path, source)?;
    prepare_score(&ndjson, path.parent().unwrap_or(Path::new(".")), mute_track)
}

pub fn load_score_file(path: &Path) -> Result<String, String> {
    let source = fs::read_to_string(path).map_err(|error| format!("score read failed: {error}"))?;
    source_to_ndjson(path, &source)
}

fn source_to_ndjson(path: &Path, source: &str) -> Result<String, String> {
    if path.extension().and_then(|ext| ext.to_str()) != Some("rb") {
        return Ok(source.to_owned());
    }
    let spinel_bin = resolve_spinel_bin();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let bin_path =
        env::temp_dir().join(format!("spinel_sonic_lab_{}_{}", std::process::id(), nonce));
    let compile = Command::new(&spinel_bin)
        .arg(path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .map_err(|error| format!("failed to invoke Spinel ({spinel_bin}): {error}"))?;
    if !compile.status.success() {
        let _ = fs::remove_file(&bin_path);
        return Err(format!(
            "Spinel compile failed: {}",
            command_error_detail(&compile)
        ));
    }
    let run = Command::new(&bin_path).output();
    let _ = fs::remove_file(&bin_path);
    let run = run.map_err(|error| format!("compiled Spinel execution failed: {error}"))?;
    if !run.status.success() {
        return Err(format!(
            "Spinel composition failed: {}",
            command_error_detail(&run)
        ));
    }
    String::from_utf8(run.stdout).map_err(|error| format!("Spinel output was not UTF-8: {error}"))
}

fn command_error_detail(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.trim().is_empty() {
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    } else {
        stderr.trim().to_owned()
    }
}

fn resolve_spinel_bin() -> String {
    if let Ok(bin) = env::var("SPINEL_BIN")
        && !bin.is_empty()
    {
        return bin;
    }
    for candidate in [
        "../.tools/spinel/bin/spinel",
        "../../.tools/spinel/bin/spinel",
    ] {
        if Path::new(candidate).is_file() {
            return candidate.to_owned();
        }
    }
    String::from("spinel")
}

/// Validate every declared asset before muting and before any engine operation.
pub fn prepare_score(input: &str, base: &Path, mute_track: Option<&str>) -> Result<Score, String> {
    let mut score = parse_and_validate_score(input, None)?;
    let mut total = 0;
    for (name, declared) in &score.samples {
        let path = base
            .join(declared)
            .canonicalize()
            .map_err(|e| format!("sample {name}: {e}"))?;
        let file = fs::File::open(&path).map_err(|e| format!("sample {name}: {e}"))?;
        let mut bytes = Vec::new();
        file.take(crate::sample::MAX_SAMPLE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("sample {name}: {e}"))?;
        total += bytes.len();
        if total > crate::sample::MAX_SCORE_SAMPLE_BYTES {
            return Err("score sample data exceeds 64MiB".to_owned());
        }
        score.sample_assets.insert(
            name.clone(),
            crate::sample::SampleAsset::from_wav(path, bytes)?,
        );
    }
    crate::schedule::Scheduler::plan_playback(&score)?;
    if let Some(track) = mute_track {
        score = score.without_track(track)?;
    }
    Ok(score)
}

/// scsynth reads exactly the validated bytes, even if the original file changed.
pub fn with_sample_snapshot<T>(
    asset: &crate::sample::SampleAsset,
    load: impl FnOnce(&Path) -> Result<T, String>,
) -> Result<T, String> {
    struct Snapshot(std::path::PathBuf);
    impl Drop for Snapshot {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let snapshot_path = env::temp_dir().join(format!(
        "spinel_sample_{}_{}.wav",
        std::process::id(),
        nonce
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&snapshot_path)
        .map_err(|e| e.to_string())?;
    let snapshot = Snapshot(snapshot_path);
    use std::io::Write;
    file.write_all(&asset.bytes).map_err(|e| e.to_string())?;
    drop(file);
    load(&snapshot.0)
}
