use std::env;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use spinel_sonic_lab::mixer::ChannelGraph;
use spinel_sonic_lab::osc::{
    GraphClient, Lookups, OscClient, SAMPLE_SYNTHDEFS, system_time_to_ntp,
};
use spinel_sonic_lab::sample::SampleAsset;
use spinel_sonic_lab::schedule::{
    PlaybackEvent, PlaybackSound, ScheduledBar, ScheduledNote, Scheduler,
};

use spinel_sonic_lab::model::Score;
use spinel_sonic_lab::sampler::SampleBuffers;
use spinel_sonic_lab::source::{load_score_file, prepare_score};
use spinel_sonic_lab::watch::ScoreWatcher;

type CliArgs = std::iter::Skip<std::env::Args>;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

#[derive(Default)]
struct CliOptions {
    dry_run: bool,
    watch: bool,
    bars: Option<usize>,
    host: String,
    port: u16,
    synthdef: PathBuf,
    mute_track: Option<String>,
    input: Option<String>,
    help: bool,
}

impl CliOptions {
    fn parse() -> Result<Self, String> {
        let mut options = Self::new();
        options.consume_args(&mut env::args().skip(1))?;
        if !options.help {
            options.validate()?;
        }
        Ok(options)
    }

    fn consume_args(&mut self, args: &mut CliArgs) -> Result<(), String> {
        while let Some(arg) = args.next() {
            if arg == "--help" || arg == "-h" {
                self.help = true;
                return Ok(());
            }
            self.apply(&arg, args)?;
        }
        Ok(())
    }

    fn new() -> Self {
        Self {
            host: String::from("127.0.0.1"),
            port: 57110,
            synthdef: PathBuf::from("synthdefs/analog_bass.scsyndef"),
            ..Self::default()
        }
    }

    fn apply(&mut self, arg: &str, args: &mut CliArgs) -> Result<(), String> {
        match arg {
            "--dry-run" => self.dry_run = true,
            "--watch" => self.watch = true,
            _ => return self.apply_value(arg, args),
        }
        Ok(())
    }

    fn apply_value(&mut self, arg: &str, args: &mut CliArgs) -> Result<(), String> {
        match arg {
            "--bars" => self.bars = Some(parse_bars_arg(args.next())?),
            "--host" => self.host = args.next().ok_or("--host needs a value")?,
            "--port" => self.port = parse_port_arg(args.next())?,
            _ => return self.apply_remaining(arg, args),
        }
        Ok(())
    }

    fn apply_remaining(&mut self, arg: &str, args: &mut CliArgs) -> Result<(), String> {
        match arg {
            "--synthdef" => self.set_synthdef(args.next())?,
            "--mute-track" => self.set_mute(args.next())?,
            _ => self.set_input(arg)?,
        }
        Ok(())
    }

    fn set_synthdef(&mut self, value: Option<String>) -> Result<(), String> {
        self.synthdef = PathBuf::from(value.ok_or("--synthdef needs a path")?);
        Ok(())
    }

    fn set_mute(&mut self, value: Option<String>) -> Result<(), String> {
        if self.mute_track.is_some() {
            return Err("--mute-track may be specified once".to_owned());
        }
        self.mute_track = Some(value.ok_or("--mute-track needs a name")?);
        Ok(())
    }

    fn set_input(&mut self, arg: &str) -> Result<(), String> {
        if arg.starts_with('-') && arg != "-" {
            return Err(format!("unknown option: {arg}"));
        }
        if self.input.is_some() {
            return Err("provide exactly one input file or -".to_owned());
        }
        self.input = Some(arg.to_owned());
        Ok(())
    }

    fn validate(&self) -> Result<(), String> {
        if self.bars.is_some() && !self.watch {
            return Err("--bars requires --watch".to_owned());
        }
        let input = self.required_input()?;
        if self.watch && input == "-" {
            return Err("--watch requires a file path, not stdin".to_owned());
        }
        Ok(())
    }

    fn required_input(&self) -> Result<&str, String> {
        self.input.as_deref().ok_or("provide an NDJSON or .rb input file or -".to_owned())
    }
}

fn parse_bars_arg(value: Option<String>) -> Result<usize, String> {
    let count: usize = value
        .ok_or("--bars needs a value")?
        .parse()
        .map_err(|_| "--bars must be a positive integer")?;
    if count == 0 {
        return Err("--bars must be positive".to_owned());
    }
    Ok(count)
}

fn parse_port_arg(value: Option<String>) -> Result<u16, String> {
    let port: u16 = value
        .ok_or("--port needs a value")?
        .parse()
        .map_err(|_| "--port must be a number from 1 to 65535")?;
    if port == 0 {
        return Err("--port must be nonzero".to_owned());
    }
    Ok(port)
}

fn print_help() {
    println!(
        "Usage: spinel-sonic-lab [--dry-run] [--watch] [--bars COUNT] [--mute-track NAME] [--host 127.0.0.1] [--port 57110] [--synthdef PATH] INPUT\nINPUT is an NDJSON or Spinel .rb file, or - for stdin (--watch requires a file path)."
    );
}

fn run() -> Result<(), String> {
    let options = CliOptions::parse()?;
    if options.help {
        print_help();
        return Ok(());
    }
    run_with_options(options)
}

fn run_with_options(options: CliOptions) -> Result<(), String> {
    let input = options.required_input()?.to_owned();
    if options.watch {
        return run_watch(options, input);
    }
    run_once(&options, &input)
}

fn run_once(options: &CliOptions, input: &str) -> Result<(), String> {
    let source = load_input(input)?;
    let score = prepare_score(&source, input_base(input), options.mute_track.as_deref())?;
    let events = Scheduler::plan_playback(&score)?;
    if options.dry_run {
        print_dry_run(&events);
        return Ok(());
    }
    play_once(&score, &events, options)
}

fn input_base(input: &str) -> &Path {
    if input == "-" { Path::new(".") } else { Path::new(input).parent().unwrap_or(Path::new(".")) }
}

fn load_input(input: &str) -> Result<String, String> {
    if input != "-" {
        return load_score_file(Path::new(input));
    }
    let mut text = String::new();
    io::stdin().read_to_string(&mut text).map_err(|error| format!("stdin read failed: {error}"))?;
    Ok(text)
}

fn play_once(score: &Score, events: &[PlaybackEvent], options: &CliOptions) -> Result<(), String> {
    let osc = connect_server(options)?;
    let mut engine = Engine::new(&osc, &options.synthdef);
    engine.prepare(score)?;
    let clock = PlaybackClock::start()?;
    let result = engine.play(&clock, events);
    engine.finish(&clock, result)?;
    eprintln!("scsynth accepted {} note commands", events.len());
    Ok(())
}

fn connect_server(options: &CliOptions) -> Result<OscClient, String> {
    let osc = OscClient::connect(&options.host, options.port)?;
    osc.load_synthdef(&options.synthdef)?;
    Ok(osc)
}

/// Engine resources prepared before playback: sample buffers and the channel graph.
struct Engine<'a> {
    osc: &'a OscClient,
    samples: SamplePlayer<'a>,
    graph: ChannelGraph<GraphClient<'a>>,
    /// Latest end of any dispatched event; resources stay reserved until then.
    end: Duration,
}

impl<'a> Engine<'a> {
    fn new(osc: &'a OscClient, synthdef: &'a Path) -> Self {
        let synthdef_dir = synthdef.parent().unwrap_or(Path::new("."));
        let graph = ChannelGraph::new(GraphClient { osc, synthdef_dir });
        let samples = SamplePlayer::new(osc, synthdef_dir);
        Self { osc, samples, graph, end: Duration::ZERO }
    }

    /// Both must succeed before a watch reload commits.
    fn prepare(&mut self, score: &Score) -> Result<(), String> {
        self.samples.prepare(score)?;
        self.graph.prepare(score)
    }

    fn prune(&mut self, now: Duration, assets: &[SampleAsset]) -> Result<(), String> {
        self.samples.buffers.prune(now, assets)?;
        self.graph.prune(now)
    }

    /// Sends reloaded strip and return controls to change exactly at `timetag`.
    fn apply_control_updates(&mut self, timetag: u64) -> Result<(), String> {
        let updates = self.graph.take_updates();
        self.osc.set_controls_bundle(timetag, &updates)
    }

    fn play(&mut self, clock: &PlaybackClock, events: &[PlaybackEvent]) -> Result<(), String> {
        let mut groups = events.chunk_by(|a, b| a.at == b.at);
        groups.try_for_each(|group| self.dispatch(clock, group))
    }

    /// Sends simultaneous events as one bundle once their buffers and strips are reserved.
    fn dispatch(&mut self, clock: &PlaybackClock, group: &[PlaybackEvent]) -> Result<(), String> {
        let at = group[0].at;
        sleep_until(clock.dispatch_start, at)?;
        warn_if_late(clock, at)?;
        group.iter().try_for_each(|event| self.reserve(event))?;
        self.send_bundle(clock.timetag(at)?, group)
    }

    /// Keeps the event's buffer and strip through its end.
    fn reserve(&mut self, event: &PlaybackEvent) -> Result<(), String> {
        let event_end = event.end()?;
        self.end = self.end.max(event_end);
        if let Some(track) = &event.route {
            self.graph.mark_used(track, event_end);
        }
        self.samples.reserve(event, event_end)
    }

    fn send_bundle(&self, timetag: u64, group: &[PlaybackEvent]) -> Result<(), String> {
        let lookups = Lookups {
            sample: &|asset: &SampleAsset| self.samples.buffers.buffer_id(asset),
            route: &|track: &str| self.graph.route_bus(track),
        };
        self.osc.play_events_bundle(timetag, group, &lookups)
    }

    /// Waits for source and return tails before releasing the graph.
    fn finish(&mut self, clock: &PlaybackClock, result: Result<(), String>) -> Result<(), String> {
        sleep_until(clock.audible_start, self.end.saturating_add(self.graph.tail()))?;
        result?;
        self.graph.finish()?;
        self.osc.verify_commands()
    }
}

/// Sample buffers, with sample SynthDefs loaded the first time a score uses samples.
struct SamplePlayer<'a> {
    osc: &'a OscClient,
    synthdef_dir: &'a Path,
    definitions_loaded: bool,
    buffers: SampleBuffers<&'a OscClient>,
}

impl<'a> SamplePlayer<'a> {
    fn new(osc: &'a OscClient, synthdef_dir: &'a Path) -> Self {
        Self { osc, synthdef_dir, definitions_loaded: false, buffers: SampleBuffers::new(osc) }
    }

    fn prepare(&mut self, score: &Score) -> Result<(), String> {
        let assets = score.used_sample_assets();
        if !assets.is_empty() {
            self.load_definitions()?;
        }
        self.buffers.prepare(assets.iter())
    }

    fn load_definitions(&mut self) -> Result<(), String> {
        if !self.definitions_loaded {
            for name in SAMPLE_SYNTHDEFS {
                let path = self.synthdef_dir.join(format!("{name}.scsyndef"));
                self.osc.load_synthdef(&path)?;
            }
            self.definitions_loaded = true;
        }
        Ok(())
    }

    fn reserve(&mut self, event: &PlaybackEvent, end: Duration) -> Result<(), String> {
        match &event.sound {
            PlaybackSound::Sample { asset, .. } => self.buffers.mark_used(asset, end),
            PlaybackSound::Synth(_) => Ok(()),
        }
    }
}

fn run_watch(options: CliOptions, input: String) -> Result<(), String> {
    let watcher = ScoreWatcher::from_path(PathBuf::from(input), options.mute_track.clone())?;
    if options.dry_run {
        return dry_watch(watcher, options.bars);
    }
    play_watch(watcher, &options)
}

fn dry_watch(mut watcher: ScoreWatcher, bars: Option<usize>) -> Result<(), String> {
    for _ in 0..bars.unwrap_or(1) {
        let (bar, warning) = watcher.next_bar()?;
        report_reload_warning(warning);
        print_dry_run(&bar.playback);
    }
    Ok(())
}

fn report_reload_warning(warning: Option<String>) {
    if let Some(warning) = warning {
        eprintln!("reload ignored: {warning}");
    }
}

fn play_watch(mut watcher: ScoreWatcher, options: &CliOptions) -> Result<(), String> {
    let osc = connect_server(options)?;
    watcher.start_background_poller(Duration::from_millis(50));
    let mut engine = Engine::new(&osc, &options.synthdef);
    let (first_bar, warning) = watcher.next_bar_with(&mut |score| engine.prepare(score))?;
    report_reload_warning(warning);
    let clock = PlaybackClock::start()?;
    WatchRunner::new(watcher, engine, clock).play(first_bar, options.bars)
}

#[derive(Default)]
struct WatchStats {
    bar_index: usize,
    total_notes: usize,
}

struct WatchRunner<'a> {
    watcher: ScoreWatcher,
    engine: Engine<'a>,
    clock: PlaybackClock,
    stats: WatchStats,
}

impl<'a> WatchRunner<'a> {
    fn new(watcher: ScoreWatcher, engine: Engine<'a>, clock: PlaybackClock) -> Self {
        Self { watcher, engine, clock, stats: WatchStats::default() }
    }

    fn play(&mut self, first: ScheduledBar, bars: Option<usize>) -> Result<(), String> {
        let result = self.play_bars(first, bars);
        // Keep buffers and strips alive until the last audible sound completes.
        self.engine.finish(&self.clock, result)?;
        eprintln!("scsynth accepted {} note commands", self.stats.total_notes);
        Ok(())
    }

    fn play_bars(&mut self, first: ScheduledBar, bars: Option<usize>) -> Result<(), String> {
        self.prune()?;
        let mut bar = first;
        self.play_bar(&bar)?;
        while self.has_bars_left(bars) {
            bar = self.advance(&bar)?;
            self.play_bar(&bar)?;
        }
        Ok(())
    }

    /// Waits for the bar boundary, releases finished resources, then takes the next bar.
    fn advance(&mut self, bar: &ScheduledBar) -> Result<ScheduledBar, String> {
        self.wait_boundary(bar)?;
        self.prune()?;
        self.next_bar()
    }

    fn has_bars_left(&self, bars: Option<usize>) -> bool {
        bars.is_none_or(|limit| self.stats.bar_index < limit)
    }

    fn prune(&mut self) -> Result<(), String> {
        let now = Instant::now().saturating_duration_since(self.clock.audible_start);
        self.engine.prune(now, &self.watcher.sample_assets())
    }

    fn next_bar(&mut self) -> Result<ScheduledBar, String> {
        let engine = &mut self.engine;
        let (bar, warning) = self.watcher.next_bar_with(&mut |score| engine.prepare(score))?;
        report_reload_warning(warning);
        Ok(bar)
    }

    fn play_bar(&mut self, bar: &ScheduledBar) -> Result<(), String> {
        self.engine.apply_control_updates(self.clock.timetag(bar.start)?)?;
        self.engine.play(&self.clock, &bar.playback)?;
        self.engine.osc.verify_commands()?;
        self.stats.total_notes += bar.playback.len();
        self.stats.bar_index += 1;
        Ok(())
    }

    fn wait_boundary(&self, bar: &ScheduledBar) -> Result<(), String> {
        let boundary = bar.start.checked_add(bar.duration).ok_or(OUT_OF_RANGE)?;
        sleep_until(self.clock.dispatch_start, boundary)
    }
}

fn print_dry_run(events: &[PlaybackEvent]) {
    events.iter().for_each(print_dry_run_event);
}

fn print_dry_run_event(event: &PlaybackEvent) {
    match &event.sound {
        PlaybackSound::Synth(note) => print_synth_note(note),
        PlaybackSound::Sample { name, .. } => print_sample_note(event, name),
    }
    if let Some(track) = &event.route {
        println!("  -> strip {track}");
    }
}

fn print_synth_note(note: &ScheduledNote) {
    let (at, gain) = (note.at.as_secs_f64(), note.tone.gain);
    let (frequency, duration, amp) = (note.frequency, note.duration_seconds, note.amplitude);
    let output_amp = amp * gain;
    println!(
        "{at:.3}s freq={frequency:.3}Hz dur={duration:.3}s amp={amp:.3} gain={gain:.3} output_amp={output_amp:.3}"
    );
}

fn print_sample_note(event: &PlaybackEvent, name: &str) {
    println!(
        "{:.3}s sample={} dur={:.3}s amp={:.3}",
        event.at.as_secs_f64(),
        name,
        event.duration.as_secs_f64(),
        event.amplitude
    );
}

// Runtime clock bridge: musical offsets stay independent of OSC and wall time.
const DISPATCH_LEAD: Duration = Duration::from_millis(200);

struct PlaybackClock {
    dispatch_start: Instant,
    audible_start: Instant,
    wall_start: SystemTime,
}

impl PlaybackClock {
    fn start() -> Result<Self, String> {
        Self::new(Instant::now(), SystemTime::now())
    }

    fn new(monotonic: Instant, wall: SystemTime) -> Result<Self, String> {
        Ok(Self {
            dispatch_start: monotonic,
            audible_start: after(monotonic, DISPATCH_LEAD)?,
            wall_start: future_wall(wall)?,
        })
    }

    fn timetag(&self, offset: Duration) -> Result<u64, String> {
        let time = self.wall_start.checked_add(offset).ok_or(OUT_OF_RANGE)?;
        Ok(system_time_to_ntp(time))
    }
}

fn future_wall(start: SystemTime) -> Result<SystemTime, String> {
    start.checked_add(DISPATCH_LEAD).ok_or_else(|| OUT_OF_RANGE.to_owned())
}

const OUT_OF_RANGE: &str = "scheduled time is out of range";

fn after(start: Instant, offset: Duration) -> Result<Instant, String> {
    start.checked_add(offset).ok_or_else(|| OUT_OF_RANGE.to_owned())
}

fn warn_if_late(clock: &PlaybackClock, at: Duration) -> Result<(), String> {
    if Instant::now() >= after(clock.audible_start, at)? {
        let seconds = at.as_secs_f64();
        eprintln!("warning: late OSC dispatch at {seconds:.3}s; keeping planned timetag");
    }
    Ok(())
}

fn sleep_until(start: Instant, offset: Duration) -> Result<(), String> {
    let due = after(start, offset)?;
    thread::sleep(due.saturating_duration_since(Instant::now()));
    Ok(())
}

#[cfg(test)]
mod timing_tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    const WALL: Duration = Duration::from_secs(1_000);

    fn clock_at(monotonic: Instant) -> PlaybackClock {
        PlaybackClock::new(monotonic, UNIX_EPOCH + WALL).unwrap()
    }

    #[test]
    fn playback_clock_keeps_the_first_note_200ms_ahead() {
        let monotonic = Instant::now();
        let clock = clock_at(monotonic);
        let lead = Duration::from_millis(200);
        assert_eq!(clock.audible_start.duration_since(monotonic), lead);
        let first = clock.timetag(Duration::ZERO).unwrap();
        assert_eq!(first, system_time_to_ntp(UNIX_EPOCH + WALL + lead));
    }

    #[test]
    fn playback_clock_spaces_all_intervals_evenly() {
        let clock = clock_at(Instant::now());
        let first = clock.timetag(Duration::ZERO).unwrap();
        let expected = [(250, 1_u64 << 30), (500, 1 << 31), (750, 3 << 30), (2000, 2 << 32)];
        for (ms, fraction) in expected {
            let tag = clock.timetag(Duration::from_millis(ms)).unwrap();
            assert_eq!(tag - first, fraction);
        }
    }

    #[test]
    fn playback_clock_dispatches_from_the_monotonic_origin() {
        let monotonic = Instant::now();
        assert_eq!(clock_at(monotonic).dispatch_start, monotonic);
    }

    #[test]
    fn playback_clock_keeps_planned_timetags_and_rejects_overflow() {
        let clock = clock_at(Instant::now());
        let first = clock.timetag(Duration::ZERO).unwrap();
        // Even after dispatch falls behind, planned timestamps must not be rebased.
        assert_eq!(clock.timetag(Duration::ZERO).unwrap(), first);
        assert!(clock.timetag(Duration::MAX).is_err());
    }
}
