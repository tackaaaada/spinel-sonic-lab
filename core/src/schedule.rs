use std::time::Duration;

use crate::model::{BassTone, MusicEvent, Score};

const BEATS_PER_BAR: f64 = 4.0;

#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledNote {
    pub at: Duration,
    pub frequency: f32,
    pub duration_seconds: f32,
    pub amplitude: f32,
    pub tone: BassTone,
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
        Self {
            score,
            bar_in_score: 0,
            next_bar_start: Duration::ZERO,
        }
    }

    pub fn score(&self) -> &Score {
        &self.score
    }

    pub fn sample_assets(&self) -> Vec<crate::sample::SampleAsset> {
        self.score.used_sample_assets()
    }

    pub fn reload_ndjson(&mut self, input: &str, mute_track: Option<&str>) -> Result<(), String> {
        let mut score = parse_and_validate_score(input, None)?;
        // Clock-free reloads can reuse known snapshots, but cannot read new files.
        // ScoreWatcher prepares changed assets through source.rs instead.
        for (name, path) in &score.samples {
            if self.score.samples.get(name) == Some(path) {
                if let Some(asset) = self.score.sample_assets.get(name) {
                    score.sample_assets.insert(name.clone(), asset.clone());
                }
            }
        }
        Scheduler::plan_playback(&score)?;
        if let Some(track) = mute_track {
            score = score.without_track(track)?;
        }
        self.apply_score(score);
        Ok(())
    }

    pub(crate) fn apply_score(&mut self, score: Score) {
        self.score = score;
        self.bar_in_score = 0;
    }

    pub fn next_bar(&mut self) -> Result<ScheduledBar, String> {
        let start = self.next_bar_start;
        let transport = Transport::new(self.score.bpm)?;
        let duration = transport.beat_time(BEATS_PER_BAR)?;
        let bar_start_beat = (self.bar_in_score as f64) * BEATS_PER_BAR;
        let bar_end_beat = bar_start_beat + BEATS_PER_BAR;
        let notes = self
            .score
            .events
            .iter()
            .filter(|event| event.beat >= bar_start_beat && event.beat < bar_end_beat)
            .map(|event| {
                let offset = transport.beat_time(event.beat - bar_start_beat)?;
                let at = start
                    .checked_add(offset)
                    .ok_or_else(|| "scheduled time is out of range".to_owned())?;
                plan_event(
                    event,
                    at,
                    transport,
                    self.score.tone_for_track(&event.track),
                )
            })
            .collect::<Result<Vec<_>, String>>()?;
        let mut playback = Scheduler::plan_playback(&self.score)?;
        let begin = transport.beat_time(bar_start_beat)?;
        let end = transport.beat_time(bar_end_beat)?;
        playback.retain(|event| event.at >= begin && event.at < end);
        for event in &mut playback {
            event.at = start
                .checked_add(event.at - begin)
                .ok_or("scheduled time is out of range")?;
            if let PlaybackSound::Synth(note) = &mut event.sound {
                note.at = event.at;
            }
        }
        let bars = score_bar_count(&self.score);
        self.bar_in_score = (self.bar_in_score + 1) % bars;
        self.next_bar_start = start
            .checked_add(duration)
            .ok_or_else(|| "scheduled time is out of range".to_owned())?;
        Ok(ScheduledBar {
            playback,
            start,
            duration,
            notes,
        })
    }
}

pub fn parse_and_validate_score(input: &str, mute_track: Option<&str>) -> Result<Score, String> {
    let mut score = Score::parse_ndjson(input)?;
    Transport::new(score.bpm)?.beat_time(BEATS_PER_BAR)?;
    Scheduler::plan(&score)?;
    for event in &score.sample_events {
        Transport::new(score.bpm)?.beat_time(event.beat)?;
    }
    if let Some(track) = mute_track {
        score = score.without_track(track)?;
    }
    Ok(score)
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
    Sample {
        name: String,
        asset: crate::sample::SampleAsset,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackEvent {
    pub at: Duration,
    pub duration: Duration,
    pub amplitude: f32,
    pub sound: PlaybackSound,
}

impl PlaybackEvent {
    pub fn end(&self) -> Result<Duration, String> {
        self.at
            .checked_add(self.duration)
            .ok_or_else(|| "scheduled end is out of range".to_owned())
    }
}

pub struct Scheduler;

impl Scheduler {
    pub fn plan_playback(score: &Score) -> Result<Vec<PlaybackEvent>, String> {
        let notes = Self::plan(score)?;
        let mut ordered = score
            .events
            .iter()
            .zip(notes)
            .map(|(event, note)| {
                let duration = Duration::from_secs_f64(f64::from(note.duration_seconds));
                (
                    event.order,
                    PlaybackEvent {
                        at: note.at,
                        duration,
                        amplitude: note.amplitude,
                        sound: PlaybackSound::Synth(note),
                    },
                )
            })
            .collect::<Vec<_>>();
        let transport = Transport::new(score.bpm)?;
        for event in &score.sample_events {
            let asset = score
                .sample_assets
                .get(&event.sample)
                .ok_or_else(|| format!("sample {} has not been prepared", event.sample))?;
            ordered.push((
                event.order,
                PlaybackEvent {
                    at: transport.beat_time(event.beat)?,
                    duration: asset.duration(),
                    amplitude: event.velocity as f32,
                    sound: PlaybackSound::Sample {
                        name: event.sample.clone(),
                        asset: asset.clone(),
                    },
                },
            ));
        }
        ordered.sort_by(|a, b| a.1.at.cmp(&b.1.at).then(a.0.cmp(&b.0)));
        let result = ordered
            .into_iter()
            .map(|(_, event)| event)
            .collect::<Vec<_>>();
        for event in &result {
            event.end()?;
        }
        Ok(result)
    }

    pub fn plan(score: &Score) -> Result<Vec<ScheduledNote>, String> {
        let transport = Transport::new(score.bpm)?;
        score
            .events
            .iter()
            .map(|event| {
                let at = transport.beat_time(event.beat)?;
                plan_event(event, at, transport, score.tone_for_track(&event.track))
            })
            .collect()
    }
}

pub fn group_notes_by_time(
    notes: &[ScheduledNote],
) -> impl Iterator<Item = (Duration, &[ScheduledNote])> {
    notes
        .chunk_by(|left, right| left.at == right.at)
        .map(|group| (group[0].at, group))
}

fn plan_event(
    event: &MusicEvent,
    at: Duration,
    transport: Transport,
    tone: BassTone,
) -> Result<ScheduledNote, String> {
    let length = transport.beat_time(event.duration_beats)?;
    let duration_seconds = length.as_secs_f64() as f32;
    if !duration_seconds.is_finite() || duration_seconds <= 0.0 {
        return Err("note duration cannot be represented as an OSC float".to_owned());
    }
    Ok(ScheduledNote {
        at,
        frequency: midi_to_frequency(event.note),
        duration_seconds,
        amplitude: (event.velocity * 0.2) as f32,
        tone,
    })
}

fn score_bar_count(score: &Score) -> usize {
    score
        .events
        .iter()
        .map(|event| event.beat)
        .chain(score.sample_events.iter().map(|event| event.beat))
        .map(|beat| (beat / BEATS_PER_BAR).floor() as usize + 1)
        .max()
        .unwrap_or(1)
        .max(1)
}

fn checked_duration(seconds: f64) -> Result<Duration, String> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err("scheduled time is out of range".to_owned());
    }
    Duration::try_from_secs_f64(seconds).map_err(|_| "scheduled time is out of range".to_owned())
}
