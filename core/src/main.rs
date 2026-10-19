use std::env;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use spinel_sonic_lab::osc::{OscClient, system_time_to_ntp};
use spinel_sonic_lab::schedule::{PlaybackEvent, PlaybackSound, Scheduler};

use spinel_sonic_lab::model::Score;
use spinel_sonic_lab::sampler::SampleBuffers;
use spinel_sonic_lab::source::{load_score_file, prepare_score};
use spinel_sonic_lab::watch::ScoreWatcher;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut dry_run = false;
    let mut watch = false;
    let mut bars: Option<usize> = None;
    let mut host = String::from("127.0.0.1");
    let mut port = 57110_u16;
    let mut synthdef = PathBuf::from("synthdefs/analog_bass.scsyndef");
    let mut mute_track = None;
    let mut input = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            "--watch" => watch = true,
            "--bars" => {
                let count: usize = args
                    .next()
                    .ok_or("--bars needs a value")?
                    .parse()
                    .map_err(|_| "--bars must be a positive integer")?;
                if count == 0 {
                    return Err("--bars must be positive".to_owned());
                }
                bars = Some(count);
            }
            "--host" => host = args.next().ok_or("--host needs a value")?,
            "--port" => {
                port = args
                    .next()
                    .ok_or("--port needs a value")?
                    .parse()
                    .map_err(|_| "--port must be a number from 1 to 65535")?;
                if port == 0 {
                    return Err("--port must be nonzero".to_owned());
                }
            }
            "--synthdef" => synthdef = PathBuf::from(args.next().ok_or("--synthdef needs a path")?),
            "--mute-track" => {
                if mute_track.is_some() {
                    return Err("--mute-track may be specified once".to_owned());
                }
                mute_track = Some(args.next().ok_or("--mute-track needs a name")?);
            }
            "--help" | "-h" => {
                println!(
                    "Usage: spinel-sonic-lab [--dry-run] [--watch] [--bars COUNT] [--mute-track NAME] [--host 127.0.0.1] [--port 57110] [--synthdef PATH] INPUT\nINPUT is an NDJSON or Spinel .rb file, or - for stdin (--watch requires a file path)."
                );
                return Ok(());
            }
            _ if arg.starts_with('-') && arg != "-" => {
                return Err(format!("unknown option: {arg}"));
            }
            _ if input.is_none() => input = Some(arg),
            _ => return Err("provide exactly one input file or -".to_owned()),
        }
    }
    if bars.is_some() && !watch {
        return Err("--bars requires --watch".to_owned());
    }
    let input = input.ok_or("provide an NDJSON or .rb input file or -")?;
    if watch {
        if input == "-" {
            return Err("--watch requires a file path, not stdin".to_owned());
        }
        return run_watch(
            PathBuf::from(input),
            mute_track,
            dry_run,
            bars,
            &host,
            port,
            &synthdef,
        );
    }

    let source = if input == "-" {
        let mut text = String::new();
        io::stdin()
            .read_to_string(&mut text)
            .map_err(|error| format!("stdin read failed: {error}"))?;
        text
    } else {
        load_score_file(Path::new(&input))?
    };

    let base = if input == "-" {
        Path::new(".")
    } else {
        Path::new(&input).parent().unwrap_or(Path::new("."))
    };
    let score = prepare_score(&source, base, mute_track.as_deref())?;
    let notes = Scheduler::plan_playback(&score)?;
    if dry_run {
        print_dry_run(&notes);
        return Ok(());
    }

    let osc = OscClient::connect(&host, port)?;
    osc.load_synthdef(&synthdef)?;
    let mut buffers = SampleBuffers::new(&osc);
    prepare_sampler(&osc, &synthdef, &mut false, &mut buffers, &score)?;
    let clock = PlaybackClock::new(Instant::now(), SystemTime::now())?;
    let mut end = Duration::ZERO;
    let result = play_scheduled_events(&osc, &clock, &notes, &mut buffers, &mut end);
    sleep_until(clock.audible_start, end)?;
    result?;
    osc.verify_commands()?;
    eprintln!("scsynth accepted {} note commands", notes.len());
    Ok(())
}

fn run_watch(
    path: PathBuf,
    mute_track: Option<String>,
    dry_run: bool,
    bars: Option<usize>,
    host: &str,
    port: u16,
    synthdef: &Path,
) -> Result<(), String> {
    let mut watcher = ScoreWatcher::from_path(path, mute_track)?;
    if dry_run {
        let limit = bars.unwrap_or(1);
        for _ in 0..limit {
            let (bar, warning) = watcher.next_bar()?;
            if let Some(warning) = warning {
                eprintln!("reload ignored: {warning}");
            }
            print_dry_run(&bar.playback);
        }
        return Ok(());
    }

    let osc = OscClient::connect(host, port)?;
    osc.load_synthdef(synthdef)?;
    watcher.start_background_poller(Duration::from_millis(50));
    let mut buffers = SampleBuffers::new(&osc);
    let mut definitions_loaded = false;
    let (first_bar, warning) = watcher.next_bar_with(&mut |score| {
        prepare_sampler(&osc, synthdef, &mut definitions_loaded, &mut buffers, score)
    })?;
    if let Some(warning) = warning {
        eprintln!("reload ignored: {warning}");
    }
    let clock = PlaybackClock::new(Instant::now(), SystemTime::now())?;
    let mut first_bar = Some(first_bar);
    let mut bar_index = 0;
    let mut total_notes = 0;
    let mut end = Duration::ZERO;
    let result = (|| -> Result<(), String> {
        loop {
            if bars.is_some_and(|limit| bar_index >= limit) {
                break;
            }
            buffers.prune(
                Instant::now().saturating_duration_since(clock.audible_start),
                &watcher.sample_assets(),
            )?;
            let bar = if let Some(bar) = first_bar.take() {
                bar
            } else {
                let (bar, warning) = watcher.next_bar_with(&mut |score| {
                    prepare_sampler(&osc, synthdef, &mut definitions_loaded, &mut buffers, score)
                })?;
                if let Some(warning) = warning {
                    eprintln!("reload ignored: {warning}");
                }
                bar
            };
            play_scheduled_events(&osc, &clock, &bar.playback, &mut buffers, &mut end)?;
            osc.verify_commands()?;
            total_notes += bar.playback.len();
            bar_index += 1;
            if bars.is_none_or(|limit| bar_index < limit) {
                let boundary = bar
                    .start
                    .checked_add(bar.duration)
                    .ok_or("scheduled time is out of range")?;
                sleep_until(clock.dispatch_start, boundary)?;
            }
        }
        Ok(())
    })();
    // Also protects buffers referenced by long notes from earlier bars or by a
    // bundle already sent before an error. Dropping them here is then safe.
    sleep_until(clock.audible_start, end)?;
    result?;
    osc.verify_commands()?;
    eprintln!("scsynth accepted {total_notes} note commands");
    Ok(())
}

fn prepare_sampler(
    osc: &OscClient,
    synthdef: &Path,
    loaded: &mut bool,
    buffers: &mut SampleBuffers<&OscClient>,
    score: &Score,
) -> Result<(), String> {
    let assets = score.used_sample_assets();
    if !assets.is_empty() && !*loaded {
        let directory = synthdef.parent().unwrap_or(Path::new("."));
        osc.load_synthdef(&directory.join("sample_mono.scsyndef"))?;
        osc.load_synthdef(&directory.join("sample_stereo.scsyndef"))?;
        *loaded = true;
    }
    buffers.prepare(assets.iter())
}

fn print_dry_run(events: &[PlaybackEvent]) {
    for event in events {
        match &event.sound {
            PlaybackSound::Synth(note) => println!(
                "{:.3}s freq={:.3}Hz dur={:.3}s amp={:.3} gain={:.3} output_amp={:.3}",
                note.at.as_secs_f64(),
                note.frequency,
                note.duration_seconds,
                note.amplitude,
                note.tone.gain,
                note.amplitude * note.tone.gain
            ),
            PlaybackSound::Sample { name, .. } => println!(
                "{:.3}s sample={} dur={:.3}s amp={:.3}",
                event.at.as_secs_f64(),
                name,
                event.duration.as_secs_f64(),
                event.amplitude
            ),
        }
    }
}

// Runtime clock bridge: musical offsets stay independent of OSC and wall time.
const DISPATCH_LEAD: Duration = Duration::from_millis(200);

struct PlaybackClock {
    dispatch_start: Instant,
    audible_start: Instant,
    wall_start: SystemTime,
}

impl PlaybackClock {
    fn new(monotonic: Instant, wall: SystemTime) -> Result<Self, String> {
        Ok(Self {
            dispatch_start: monotonic,
            audible_start: monotonic
                .checked_add(DISPATCH_LEAD)
                .ok_or("scheduled time is out of range")?,
            wall_start: wall
                .checked_add(DISPATCH_LEAD)
                .ok_or("scheduled time is out of range")?,
        })
    }

    fn timetag(&self, offset: Duration) -> Result<u64, String> {
        let time = self
            .wall_start
            .checked_add(offset)
            .ok_or("scheduled time is out of range")?;
        Ok(system_time_to_ntp(time))
    }
}

fn play_scheduled_events(
    osc: &OscClient,
    clock: &PlaybackClock,
    events: &[PlaybackEvent],
    buffers: &mut SampleBuffers<&OscClient>,
    end: &mut Duration,
) -> Result<(), String> {
    for group in events.chunk_by(|a, b| a.at == b.at) {
        let at = group[0].at;
        sleep_until(clock.dispatch_start, at)?;
        let due = clock
            .audible_start
            .checked_add(at)
            .ok_or("scheduled time is out of range")?;
        if Instant::now() >= due {
            eprintln!(
                "warning: late OSC dispatch at {:.3}s; keeping planned timetag",
                at.as_secs_f64()
            );
        }
        for event in group {
            *end = (*end).max(event.end()?);
            if let PlaybackSound::Sample { asset, .. } = &event.sound {
                buffers.mark_used(asset, event.end()?)?;
            }
        }
        osc.play_events_bundle(clock.timetag(at)?, group, |asset| buffers.buffer_id(asset))?;
    }
    Ok(())
}

fn sleep_until(start: Instant, offset: Duration) -> Result<(), String> {
    let due = start
        .checked_add(offset)
        .ok_or("scheduled time is out of range")?;
    let now = Instant::now();
    if due > now {
        thread::sleep(due - now);
    }
    Ok(())
}

#[cfg(test)]
mod timing_tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    #[test]
    fn playback_clock_keeps_first_note_ahead_and_all_intervals_even() {
        let monotonic = Instant::now();
        let wall = UNIX_EPOCH + Duration::from_secs(1_000);
        let clock = PlaybackClock::new(monotonic, wall).unwrap();
        assert_eq!(
            clock.audible_start.duration_since(monotonic),
            Duration::from_millis(200)
        );
        let first = clock.timetag(Duration::ZERO).unwrap();
        assert_eq!(
            first,
            spinel_sonic_lab::osc::system_time_to_ntp(wall + Duration::from_millis(200))
        );
        for (ms, fraction) in [
            (250, 1_u64 << 30),
            (500, 1_u64 << 31),
            (750, 3_u64 << 30),
            (2000, 2_u64 << 32),
        ] {
            assert_eq!(
                clock.timetag(Duration::from_millis(ms)).unwrap() - first,
                fraction
            );
            assert_eq!(
                clock.dispatch_start.checked_add(Duration::from_millis(ms)),
                monotonic.checked_add(Duration::from_millis(ms))
            );
        }
        // Even after dispatch falls behind, planned timestamps must not be rebased.
        assert_eq!(clock.timetag(Duration::ZERO).unwrap(), first);
        assert!(clock.timetag(Duration::MAX).is_err());
    }
}
