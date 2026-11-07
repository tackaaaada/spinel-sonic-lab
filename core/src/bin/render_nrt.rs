//! Renders a score offline with `scsynth -N` through the live encoders.
//! Run from the project root so `synthdefs/` resolves.

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use spinel_sonic_lab::nrt::score_commands;
use spinel_sonic_lab::source::{load_score_file, prepare_score, render_nrt};

const USAGE: &str = "Usage: render_nrt INPUT OUTPUT.wav [SECONDS]\nINPUT is an NDJSON or Spinel .rb score. SECONDS fixes the file length; by default it ends after the last note and any reverb tail.";

struct Request {
    input: PathBuf,
    output: PathBuf,
    length: Option<Duration>,
}

fn main() -> ExitCode {
    match parse(env::args().skip(1).collect()).and_then(|request| render(&request)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn parse(args: Vec<String>) -> Result<Request, String> {
    let [input, output, rest @ ..] = args.as_slice() else { return Err(USAGE.into()) };
    let length = match rest {
        [] => None,
        [seconds] => Some(seconds_arg(seconds)?),
        _ => return Err(USAGE.into()),
    };
    Ok(Request { input: input.into(), output: output.into(), length })
}

fn seconds_arg(value: &str) -> Result<Duration, String> {
    let seconds: f64 = value.parse().map_err(|_| format!("SECONDS must be a number: {value}"))?;
    Duration::try_from_secs_f64(seconds).map_err(|_| format!("SECONDS must be positive: {value}"))
}

fn render(request: &Request) -> Result<(), String> {
    let ndjson = load_score_file(&request.input)?;
    let base = request.input.parent().unwrap_or(Path::new("."));
    let score = prepare_score(&ndjson, base, None)?;
    let commands = score_commands(&score, Path::new("synthdefs"), request.length)?;
    let asset = render_nrt(&commands, &request.output)?;
    println!("Rendered {} ({:.3} s)", request.output.display(), asset.duration().as_secs_f64());
    Ok(())
}
