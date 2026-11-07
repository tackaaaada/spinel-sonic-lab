use std::env;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::Score;
use crate::sample::SampleAsset;
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
    let bin_path = compiled_path();
    compile_spinel(path, &spinel_bin, &bin_path)?;
    run_spinel(&bin_path)
}

fn compiled_path() -> std::path::PathBuf {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    env::temp_dir().join(format!("spinel_sonic_lab_{}_{}", std::process::id(), nonce))
}

fn compile_spinel(path: &Path, spinel_bin: &str, bin_path: &Path) -> Result<(), String> {
    let compile = spinel_compile_output(path, spinel_bin, bin_path)?;
    if compile.status.success() {
        return Ok(());
    }
    let _ = fs::remove_file(bin_path);
    let detail = command_error_detail(&compile);
    Err(format!("Spinel compile failed: {detail}"))
}

fn spinel_compile_output(path: &Path, spinel_bin: &str, bin_path: &Path) -> Result<Output, String> {
    Command::new(spinel_bin)
        .arg(path)
        .arg("-o")
        .arg(bin_path)
        .output()
        .map_err(|error| format!("failed to invoke Spinel ({spinel_bin}): {error}"))
}

fn run_spinel(bin_path: &Path) -> Result<String, String> {
    let run = Command::new(bin_path).output();
    let _ = fs::remove_file(bin_path);
    let run = run.map_err(|error| format!("compiled Spinel execution failed: {error}"))?;
    ensure_composition_success(&run)?;
    String::from_utf8(run.stdout).map_err(|error| format!("Spinel output was not UTF-8: {error}"))
}

fn ensure_composition_success(run: &Output) -> Result<(), String> {
    if run.status.success() {
        return Ok(());
    }
    Err(format!("Spinel composition failed: {}", command_error_detail(run)))
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
    if let Some(bin) = env_spinel_bin() {
        return bin;
    }
    if let Some(candidate) = local_spinel_bin() {
        return candidate.to_owned();
    }
    String::from("spinel")
}

fn env_spinel_bin() -> Option<String> {
    env::var("SPINEL_BIN").ok().filter(|bin| !bin.is_empty())
}

fn local_spinel_bin() -> Option<&'static str> {
    ["../.tools/spinel/bin/spinel", "../../.tools/spinel/bin/spinel"]
        .into_iter()
        .find(|candidate| Path::new(candidate).is_file())
}

/// Validate every declared asset before muting and before any engine operation.
pub fn prepare_score(input: &str, base: &Path, mute_track: Option<&str>) -> Result<Score, String> {
    let mut score = parse_and_validate_score(input, None)?;
    load_assets(&mut score, base)?;
    crate::schedule::Scheduler::plan_playback(&score)?;
    if let Some(track) = mute_track {
        score = score.without_track(track)?;
    }
    Ok(score)
}

fn load_assets(score: &mut Score, base: &Path) -> Result<(), String> {
    let mut total = 0;
    for (name, declared) in &score.samples {
        let asset = load_asset(name, declared, base)?;
        total += asset.bytes.len();
        check_sample_total(total)?;
        score.sample_assets.insert(name.clone(), asset);
    }
    Ok(())
}

fn check_sample_total(total: usize) -> Result<(), String> {
    if total > crate::sample::MAX_SCORE_SAMPLE_BYTES {
        return Err("score sample data exceeds 64MiB".to_owned());
    }
    Ok(())
}

fn load_asset(name: &str, declared: &str, base: &Path) -> Result<SampleAsset, String> {
    let path = base.join(declared).canonicalize().map_err(|e| format!("sample {name}: {e}"))?;
    let bytes = read_sample_bytes(name, &path)?;
    SampleAsset::from_wav(path, bytes)
}

fn read_sample_bytes(name: &str, path: &Path) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|e| format!("sample {name}: {e}"))?;
    let mut bytes = Vec::new();
    file.take(crate::sample::MAX_SAMPLE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("sample {name}: {e}"))?;
    Ok(bytes)
}

struct Snapshot(std::path::PathBuf);

impl Drop for Snapshot {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn snapshot_path() -> Result<std::path::PathBuf, String> {
    let nonce = current_nonce()?;
    Ok(env::temp_dir().join(format!("spinel_sample_{}_{}.wav", std::process::id(), nonce)))
}

fn current_nonce() -> Result<u128, String> {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
    Ok(nonce)
}

fn write_snapshot(asset: &SampleAsset) -> Result<Snapshot, String> {
    let snapshot_path = snapshot_path()?;
    let mut file = open_snapshot(&snapshot_path)?;
    let snapshot = Snapshot(snapshot_path);
    use std::io::Write;
    file.write_all(&asset.bytes).map_err(|e| e.to_string())?;
    drop(file);
    Ok(snapshot)
}

fn open_snapshot(path: &Path) -> Result<fs::File, String> {
    fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(|e| e.to_string())
}

/// scsynth reads exactly the validated bytes, even if the original file changed.
pub fn with_sample_snapshot<T>(
    asset: &SampleAsset,
    load: impl FnOnce(&Path) -> Result<T, String>,
) -> Result<T, String> {
    let snapshot = write_snapshot(asset)?;
    load(&snapshot.0)
}
