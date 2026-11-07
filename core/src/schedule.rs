use std::ops::Range;
use std::time::Duration;

use crate::model::{BassTone, ChannelStrip, MusicEvent, SampleEvent, Score};
use crate::sample::SampleAsset;

const BEATS_PER_BAR: f64 = 4.0;

#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledNote {
    pub at: Duration,
    pub frequency: f32,
    pub duration_seconds: f32,
    pub amplitude: f32,
    pub tone: BassTone,
    pub pan: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledBar {
    pub playback: Vec<PlaybackEvent>,
    pub start: Duration,
    pub duration: Duration,
    pub notes: Vec<ScheduledNote>,
}

#[derive(Debug, Clone)]
pub struct LiveScheduler {
    score: Score,
    bar_in_score: usize,
    next_bar_start: Duration,
}

impl LiveScheduler {
    pub fn new(score: Score) -> Self {
        Self { score, bar_in_score: 0, next_bar_start: Duration::ZERO }
    }

    pub fn score(&self) -> &Score {
        &self.score
    }

    pub fn sample_assets(&self) -> Vec<SampleAsset> {
        self.score.used_sample_assets()
    }

    pub fn reload_ndjson(&mut self, input: &str, mute_track: Option<&str>) -> Result<(), String> {
        let mut score = parse_and_validate_score(input, None)?;
        self.reuse_known_assets(&mut score);
        Scheduler::plan_playback(&score)?;
        if let Some(track) = mute_track {
            score = score.without_track(track)?;
        }
        self.apply_score(score);
        Ok(())
    }

    /// Clock-free reloads can reuse known snapshots, but cannot read new files.
    /// ScoreWatcher prepares changed assets through source.rs instead.
    fn reuse_known_assets(&self, score: &mut Score) {
        for (name, path) in &score.samples {
            let unchanged = self.score.samples.get(name) == Some(path);
            if let Some(asset) = self.score.sample_assets.get(name).filter(|_| unchanged) {
                score.sample_assets.insert(name.clone(), asset.clone());
            }
        }
    }

    pub(crate) fn apply_score(&mut self, score: Score) {
        self.score = score;
        self.bar_in_score = 0;
    }

    pub fn next_bar(&mut self) -> Result<ScheduledBar, String> {
        let window = self.current_window()?;
        let duration = window.transport.beat_time(BEATS_PER_BAR)?;
        let notes = self.bar_notes(&window)?;
        let playback = self.bar_playback(&window)?;
        let start = window.start;
        self.advance(start, duration)?;
        Ok(ScheduledBar { playback, start, duration, notes })
    }

    fn current_window(&self) -> Result<BarWindow, String> {
        let transport = Transport::new(self.score.bpm)?;
        let first_beat = (self.bar_in_score as f64) * BEATS_PER_BAR;
        Ok(BarWindow { transport, start: self.next_bar_start, first_beat })
    }

    /// Notes that start in this bar, timed from the bar's absolute start.
    fn bar_notes(&self, window: &BarWindow) -> Result<Vec<ScheduledNote>, String> {
        let planner = Planner::new(&self.score)?;
        let in_bar = self.score.events.iter().filter(|event| window.contains(event.beat));
        in_bar.map(|event| planner.note(event, window.time_of(event.beat)?)).collect()
    }

    /// Playback events of this bar, moved from score time to the bar's absolute start.
    fn bar_playback(&self, window: &BarWindow) -> Result<Vec<PlaybackEvent>, String> {
        let mut playback = Scheduler::plan_playback(&self.score)?;
        let score_times = window.score_times()?;
        playback.retain(|event| score_times.contains(&event.at));
        playback.iter_mut().try_for_each(|event| window.shift(event, score_times.start))?;
        Ok(playback)
    }

    fn advance(&mut self, start: Duration, duration: Duration) -> Result<(), String> {
        self.bar_in_score = (self.bar_in_score + 1) % score_bar_count(&self.score);
        self.next_bar_start = start.checked_add(duration).ok_or_else(|| OUT_OF_RANGE.to_owned())?;
        Ok(())
    }
}

const OUT_OF_RANGE: &str = "scheduled time is out of range";

/// One bar of the score, placed at its absolute start time.
struct BarWindow {
    transport: Transport,
    start: Duration,
    first_beat: f64,
}

impl BarWindow {
    fn contains(&self, beat: f64) -> bool {
        beat >= self.first_beat && beat < self.first_beat + BEATS_PER_BAR
    }

    /// The absolute time of a beat inside this bar.
    fn time_of(&self, beat: f64) -> Result<Duration, String> {
        let offset = self.transport.beat_time(beat - self.first_beat)?;
        self.start.checked_add(offset).ok_or_else(|| OUT_OF_RANGE.to_owned())
    }

    /// This bar's span measured from the start of the score.
    fn score_times(&self) -> Result<Range<Duration>, String> {
        let begin = self.transport.beat_time(self.first_beat)?;
        Ok(begin..self.transport.beat_time(self.first_beat + BEATS_PER_BAR)?)
    }

    fn shift(&self, event: &mut PlaybackEvent, score_begin: Duration) -> Result<(), String> {
        event.at = self.start.checked_add(event.at - score_begin).ok_or(OUT_OF_RANGE)?;
        if let PlaybackSound::Synth(note) = &mut event.sound {
            note.at = event.at;
        }
        Ok(())
    }
}

pub fn parse_and_validate_score(input: &str, mute_track: Option<&str>) -> Result<Score, String> {
    let score = Score::parse_ndjson(input)?;
    validate_timing(&score)?;
    match mute_track {
        Some(track) => score.without_track(track),
        None => Ok(score),
    }
}

/// Every note and sample trigger must have a representable time at the score's tempo.
fn validate_timing(score: &Score) -> Result<(), String> {
    let transport = Transport::new(score.bpm)?;
    transport.beat_time(BEATS_PER_BAR)?;
    Scheduler::plan(score)?;
    score.sample_events.iter().try_for_each(|event| transport.beat_time(event.beat).map(drop))
}

#[derive(Debug, Clone, Copy)]
pub struct Transport {
    bpm: f64,
}

impl Transport {
    pub fn new(bpm: f64) -> Result<Self, String> {
        if !bpm.is_finite() || bpm <= 0.0 {
            return Err("bpm must be finite and positive".to_owned());
        }
        Ok(Self { bpm })
    }

    pub fn beat_time(self, beat: f64) -> Result<Duration, String> {
        if !beat.is_finite() || beat < 0.0 {
            return Err("beat must be finite and nonnegative".to_owned());
        }
        let seconds = beat * 60.0 / self.bpm;
        checked_duration(seconds)
    }
}

pub fn midi_to_frequency(note: u8) -> f32 {
    (440.0_f64 * 2.0_f64.powf((f64::from(note) - 69.0) / 12.0)) as f32
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackSound {
    Synth(ScheduledNote),
    Sample { name: String, asset: SampleAsset },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackEvent {
    pub at: Duration,
    pub duration: Duration,
    pub amplitude: f32,
    pub pan: Option<f32>,
    /// Logical strip route (track name) resolved to an audio bus by the mixer.
    pub route: Option<String>,
    pub sound: PlaybackSound,
}

impl PlaybackEvent {
    pub fn end(&self) -> Result<Duration, String> {
        self.at.checked_add(self.duration).ok_or_else(|| "scheduled end is out of range".to_owned())
    }
}

pub struct Scheduler;

impl Scheduler {
    pub fn plan_playback(score: &Score) -> Result<Vec<PlaybackEvent>, String> {
        let planner = Planner::new(score)?;
        let mut ordered = planner.ordered_synth_events()?;
        ordered.extend(planner.ordered_sample_events()?);
        ordered.sort_by(|a, b| a.1.at.cmp(&b.1.at).then(a.0.cmp(&b.0)));
        let events: Vec<_> = ordered.into_iter().map(|(_, event)| event).collect();
        events.iter().try_for_each(|event| event.end().map(drop))?;
        Ok(events)
    }

    pub fn plan(score: &Score) -> Result<Vec<ScheduledNote>, String> {
        let planner = Planner::new(score)?;
        score.events.iter().map(|event| planner.note_at_beat(event)).collect()
    }
}

/// Plans one score's events at its tempo, with per-track tones and channel levels.
struct Planner<'s> {
    score: &'s Score,
    transport: Transport,
}

/// Events keyed by their score line, which orders events at the same time.
type Ordered = Vec<(usize, PlaybackEvent)>;

impl<'s> Planner<'s> {
    fn new(score: &'s Score) -> Result<Self, String> {
        Ok(Self { score, transport: Transport::new(score.bpm)? })
    }

    fn note_at_beat(&self, event: &MusicEvent) -> Result<ScheduledNote, String> {
        self.note(event, self.transport.beat_time(event.beat)?)
    }

    fn note(&self, event: &MusicEvent, at: Duration) -> Result<ScheduledNote, String> {
        let duration_seconds = note_seconds(self.transport, event.duration_beats)?;
        let channel = event_channel(self.score, &event.track);
        let amplitude = (event.velocity * 0.2) as f32 * channel_level(channel);
        let frequency = midi_to_frequency(event.note);
        let (tone, pan) = (self.score.tone_for_track(&event.track), channel.map(|c| c.pan));
        Ok(ScheduledNote { at, frequency, duration_seconds, amplitude, tone, pan })
    }

    fn ordered_synth_events(&self) -> Result<Ordered, String> {
        let events = self.score.events.iter();
        events
            .map(|event| Ok((event.order, self.synth_event(event, self.note_at_beat(event)?))))
            .collect()
    }

    fn synth_event(&self, event: &MusicEvent, note: ScheduledNote) -> PlaybackEvent {
        let duration = Duration::from_secs_f64(f64::from(note.duration_seconds));
        let (at, amplitude, pan) = (note.at, note.amplitude, note.pan);
        let route = strip_route(self.score, &event.track);
        PlaybackEvent { at, duration, amplitude, pan, route, sound: PlaybackSound::Synth(note) }
    }

    fn ordered_sample_events(&self) -> Result<Ordered, String> {
        let events = self.score.sample_events.iter();
        events.map(|event| Ok((event.order, self.sample_event(event)?))).collect()
    }

    fn sample_event(&self, event: &SampleEvent) -> Result<PlaybackEvent, String> {
        let asset = self.prepared_asset(&event.sample)?;
        let at = self.transport.beat_time(event.beat)?;
        let channel = event_channel(self.score, &event.track);
        let amplitude = (event.velocity as f32) * channel_level(channel);
        let (pan, route) = (channel.map(|c| c.pan), strip_route(self.score, &event.track));
        let sound = PlaybackSound::Sample { name: event.sample.clone(), asset: asset.clone() };
        Ok(PlaybackEvent { at, duration: asset.duration(), amplitude, pan, route, sound })
    }

    fn prepared_asset(&self, name: &str) -> Result<&'s SampleAsset, String> {
        let asset = self.score.sample_assets.get(name);
        asset.ok_or_else(|| format!("sample {name} has not been prepared"))
    }
}

pub fn group_notes_by_time(
    notes: &[ScheduledNote],
) -> impl Iterator<Item = (Duration, &[ScheduledNote])> {
    notes.chunk_by(|left, right| left.at == right.at).map(|group| (group[0].at, group))
}

fn note_seconds(transport: Transport, beats: f64) -> Result<f32, String> {
    let seconds = transport.beat_time(beats)?.as_secs_f64() as f32;
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err("note duration cannot be represented as an OSC float".to_owned());
    }
    Ok(seconds)
}

fn channel_level(channel: Option<&ChannelStrip>) -> f32 {
    channel.map_or(1.0, ChannelStrip::linear_level)
}

/// Channel levels applied per event; processed strips apply them on the bus instead.
fn event_channel<'a>(score: &'a Score, track: &str) -> Option<&'a ChannelStrip> {
    score.channel_for_track(track).filter(|channel| !channel.needs_graph())
}

fn strip_route(score: &Score, track: &str) -> Option<String> {
    score.channel_for_track(track).filter(|channel| channel.needs_graph()).map(|_| track.to_owned())
}

fn score_bar_count(score: &Score) -> usize {
    score.loop_bars.unwrap_or_else(|| bars_with_events(score))
}

fn bars_with_events(score: &Score) -> usize {
    let note_beats = score.events.iter().map(|event| event.beat);
    let beats = note_beats.chain(score.sample_events.iter().map(|event| event.beat));
    beats.map(|beat| (beat / BEATS_PER_BAR).floor() as usize + 1).max().unwrap_or(1).max(1)
}

fn checked_duration(seconds: f64) -> Result<Duration, String> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err("scheduled time is out of range".to_owned());
    }
    Duration::try_from_secs_f64(seconds).map_err(|_| "scheduled time is out of range".to_owned())
}
