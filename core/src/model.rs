use std::collections::BTreeMap;
use std::ops::RangeInclusive;

use serde_json::{Map, Value};

type JsonObject = Map<String, Value>;
type ModelResult<T> = Result<T, String>;
type NumberRange = RangeInclusive<f64>;
/// Levels such as velocity, sustain, LFO depth and reverb amounts.
const UNIT_RANGE: NumberRange = 0.0..=1.0;

#[derive(Debug, Clone, PartialEq)]
pub struct SampleEvent {
    pub sample: String,
    pub track: String,
    pub beat: f64,
    pub velocity: f64,
    pub order: usize,
}

impl SampleEvent {
    fn new(sample: &str, track: &str, beat: f64, velocity: f64, order: usize) -> Self {
        Self { sample: sample.to_owned(), track: track.to_owned(), beat, velocity, order }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MusicEvent {
    pub order: usize,
    pub instrument: String,
    pub track: String,
    pub note: u8,
    pub beat: f64,
    pub duration_beats: f64,
    pub velocity: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Oscillator {
    Saw,
    Sine,
    Pulse,
    Noise,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BassTone {
    pub oscillator: Oscillator,
    pub cutoff_hz: f32,
    pub attack_seconds: f32,
    pub decay_seconds: f32,
    pub sustain_level: f32,
    pub release_seconds: f32,
    pub lfo_rate_hz: f32,
    pub lfo_depth: f32,
    pub gain: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChannelStrip {
    pub ha_gain_db: f32,
    pub pan: f32,
    pub fader_db: f32,
    pub compressor: Option<Compressor>,
    pub eq_bell: Option<BellEq>,
    pub aux_send: Option<AuxSend>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Compressor {
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BellEq {
    pub frequency_hz: f32,
    pub gain_db: f32,
    pub q: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuxSend {
    pub return_name: String,
    pub level_db: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuxReturn {
    pub room: f32,
    pub damp: f32,
    pub wet: f32,
    pub fader_db: f32,
}

impl ChannelStrip {
    pub fn linear_level(&self) -> f32 {
        10.0_f32.powf((self.ha_gain_db + self.fader_db) / 20.0)
    }

    pub fn needs_graph(&self) -> bool {
        self.compressor.is_some() || self.eq_bell.is_some() || self.aux_send.is_some()
    }

    fn from_object(object: &JsonObject, line: usize) -> Result<Self, String> {
        check_channel_fields(object, line)?;
        let levels = channel_levels(object, line)?;
        let controls = channel_controls(object, line)?;
        Ok(Self::from_parts(levels, controls))
    }

    fn from_parts(levels: (f32, f32, f32), controls: ChannelControls) -> Self {
        Self {
            ha_gain_db: levels.0,
            pan: levels.1,
            fader_db: levels.2,
            compressor: controls.0,
            eq_bell: controls.1,
            aux_send: controls.2,
        }
    }
}

type ChannelControls = (Option<Compressor>, Option<BellEq>, Option<AuxSend>);

fn check_channel_fields(object: &JsonObject, line: usize) -> ModelResult<()> {
    let required = &["type", "track", "ha_gain_db", "pan", "fader_db"];
    check_fields_with_optional(object, required, &["compressor", "eq_bell", "aux_send"], line)
}

fn channel_levels(object: &JsonObject, line: usize) -> ModelResult<(f32, f32, f32)> {
    let ha = get_f64(object, "ha_gain_db", line)?;
    let pan = get_f64(object, "pan", line)?;
    let fader = get_f64(object, "fader_db", line)?;
    validate_channel_levels(ha, pan, fader, line)?;
    Ok((ha as f32, pan as f32, fader as f32))
}

fn validate_channel_levels(ha: f64, pan: f64, fader: f64, line: usize) -> ModelResult<()> {
    check_range(ha, -96.0..=24.0, "ha_gain_db must be -96..24", line)?;
    check_range(pan, -1.0..=1.0, "pan must be -1..1", line)?;
    check_range(fader, -96.0..=24.0, "fader_db must be -96..24", line)
}

fn channel_controls(object: &JsonObject, line: usize) -> ModelResult<ChannelControls> {
    let compressor = optional_nested(object, "compressor", line, Compressor::from_value)?;
    let eq_bell = optional_nested(object, "eq_bell", line, BellEq::from_value)?;
    let aux_send = optional_nested(object, "aux_send", line, AuxSend::from_value)?;
    Ok((compressor, eq_bell, aux_send))
}

fn optional_nested<T>(
    object: &JsonObject,
    key: &str,
    line: usize,
    parse: fn(&Value, usize) -> ModelResult<T>,
) -> ModelResult<Option<T>> {
    object.get(key).map(|value| parse(value, line)).transpose()
}

impl Compressor {
    fn from_value(value: &Value, line: usize) -> Result<Self, String> {
        let object = nested_object(value, "compressor", line)?;
        check_fields(object, COMPRESSOR_FIELDS, line)?;
        let values = CompressorValues::read(object, line)?;
        values.validate(line)?;
        Ok(values.into_compressor())
    }
}

const COMPRESSOR_FIELDS: &[&str] = &["threshold_db", "ratio", "attack_ms", "release_ms"];

struct CompressorValues {
    threshold_db: f64,
    ratio: f64,
    attack_ms: f64,
    release_ms: f64,
}

impl CompressorValues {
    fn read(object: &JsonObject, line: usize) -> ModelResult<Self> {
        Ok(Self {
            threshold_db: get_f64(object, "threshold_db", line)?,
            ratio: get_f64(object, "ratio", line)?,
            attack_ms: get_f64(object, "attack_ms", line)?,
            release_ms: get_f64(object, "release_ms", line)?,
        })
    }

    fn validate(&self, line: usize) -> ModelResult<()> {
        self.validate_threshold(line)?;
        check_compressor_value(self.ratio, 1.0..=20.0, "ratio", "1..20", line)?;
        self.validate_times(line)
    }

    fn validate_threshold(&self, line: usize) -> ModelResult<()> {
        check_compressor_value(self.threshold_db, -60.0..=0.0, "threshold_db", "-60..0", line)
    }

    fn validate_times(&self, line: usize) -> ModelResult<()> {
        check_compressor_value(self.attack_ms, 0.1..=200.0, "attack_ms", "0.1..200", line)?;
        check_compressor_value(self.release_ms, 5.0..=2000.0, "release_ms", "5..2000", line)
    }

    fn into_compressor(self) -> Compressor {
        Compressor {
            threshold_db: self.threshold_db as f32,
            ratio: self.ratio as f32,
            attack_ms: self.attack_ms as f32,
            release_ms: self.release_ms as f32,
        }
    }
}

fn check_compressor_value(
    value: f64,
    range: NumberRange,
    field: &str,
    bounds: &str,
    line: usize,
) -> ModelResult<()> {
    let message = format!("compressor {field} must be {bounds}");
    check_range(value, range, &message, line)
}

impl BellEq {
    fn from_value(value: &Value, line: usize) -> Result<Self, String> {
        let object = nested_object(value, "eq_bell", line)?;
        check_fields(object, &["frequency_hz", "gain_db", "q"], line)?;
        let values = BellValues::read(object, line)?;
        values.validate(line)?;
        Ok(values.into_bell())
    }
}

struct BellValues {
    frequency_hz: f64,
    gain_db: f64,
    q: f64,
}

impl BellValues {
    fn read(object: &JsonObject, line: usize) -> ModelResult<Self> {
        Ok(Self {
            frequency_hz: get_f64(object, "frequency_hz", line)?,
            gain_db: get_f64(object, "gain_db", line)?,
            q: get_f64(object, "q", line)?,
        })
    }

    fn validate(&self, line: usize) -> ModelResult<()> {
        check_range(
            self.frequency_hz,
            20.0..=20000.0,
            "eq_bell frequency_hz must be 20..20000",
            line,
        )?;
        self.validate_gain(line)
    }

    fn validate_gain(&self, line: usize) -> ModelResult<()> {
        check_range(self.gain_db, -18.0..=18.0, "eq_bell gain_db must be -18..18", line)?;
        check_range(self.q, 0.1..=10.0, "eq_bell q must be 0.1..10", line)
    }

    fn into_bell(self) -> BellEq {
        BellEq {
            frequency_hz: self.frequency_hz as f32,
            gain_db: self.gain_db as f32,
            q: self.q as f32,
        }
    }
}

impl AuxSend {
    fn from_value(value: &Value, line: usize) -> Result<Self, String> {
        let object = nested_object(value, "aux_send", line)?;
        check_fields(object, &["return", "level_db"], line)?;
        let return_name = get_str(object, "return", line)?;
        validate_name(return_name, line)?;
        Ok(Self { return_name: return_name.to_owned(), level_db: aux_send_level(object, line)? })
    }
}

fn aux_send_level(object: &JsonObject, line: usize) -> ModelResult<f32> {
    let value = get_f64(object, "level_db", line)?;
    check_range(value, -96.0..=0.0, "aux_send level_db must be -96..0", line)?;
    Ok(value as f32)
}

impl AuxReturn {
    fn from_object(object: &JsonObject, line: usize) -> Result<Self, String> {
        check_fields(object, &["type", "name", "reverb", "fader_db"], line)?;
        let reverb = reverb_object(object, line)?;
        check_fields(reverb, &["room", "damp", "wet"], line)?;
        let values = reverb_values(reverb, line)?;
        let fader_db = aux_return_fader(object, line)?;
        Ok(Self::from_values(values, fader_db))
    }

    fn from_values(values: (f32, f32, f32), fader_db: f32) -> Self {
        Self { room: values.0, damp: values.1, wet: values.2, fader_db }
    }
}

fn reverb_object(object: &JsonObject, line: usize) -> ModelResult<&JsonObject> {
    let value = object.get("reverb").ok_or_else(|| format!("line {line}: missing reverb"))?;
    nested_object(value, "reverb", line)
}

fn reverb_values(reverb: &JsonObject, line: usize) -> ModelResult<(f32, f32, f32)> {
    let room = get_f64(reverb, "room", line)?;
    let damp = get_f64(reverb, "damp", line)?;
    let wet = get_f64(reverb, "wet", line)?;
    validate_reverb(room, damp, wet, line)?;
    Ok((room as f32, damp as f32, wet as f32))
}

fn validate_reverb(room: f64, damp: f64, wet: f64, line: usize) -> ModelResult<()> {
    for (value, key) in [(room, "room"), (damp, "damp"), (wet, "wet")] {
        check_reverb_value(value, key, line)?;
    }
    Ok(())
}

fn check_reverb_value(value: f64, key: &str, line: usize) -> ModelResult<()> {
    check_range(value, UNIT_RANGE, &format!("reverb {key} must be 0..1"), line)
}

fn aux_return_fader(object: &JsonObject, line: usize) -> ModelResult<f32> {
    let value = get_f64(object, "fader_db", line)?;
    check_range(value, -96.0..=24.0, "aux return fader_db must be -96..24", line)?;
    Ok(value as f32)
}

impl Default for BassTone {
    fn default() -> Self {
        DEFAULT_BASS_TONE
    }
}

const DEFAULT_BASS_TONE: BassTone = BassTone {
    oscillator: Oscillator::Saw,
    cutoff_hz: 0.0, // Track the note at four times its fundamental.
    attack_seconds: 0.01,
    decay_seconds: 0.0,
    sustain_level: 1.0,
    release_seconds: 0.09,
    lfo_rate_hz: 0.0,
    lfo_depth: 0.0,
    gain: 2.8,
};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Score {
    pub bpm: f64,
    pub loop_bars: Option<usize>,
    pub bass_tone: BassTone,
    pub track_tones: BTreeMap<String, BassTone>,
    pub channels: BTreeMap<String, ChannelStrip>,
    pub aux_returns: BTreeMap<String, AuxReturn>,
    pub events: Vec<MusicEvent>,
    pub samples: BTreeMap<String, String>,
    pub sample_events: Vec<SampleEvent>,
    pub sample_assets: BTreeMap<String, crate::sample::SampleAsset>,
}

impl Score {
    fn with_tempo(bpm: f64, loop_bars: Option<usize>) -> Self {
        Self { bpm, loop_bars, ..Self::default() }
    }

    pub fn parse_ndjson(input: &str) -> Result<Self, String> {
        let mut lines = nonblank_lines(input);
        let (index, first) = lines.next().ok_or("score is empty")?;
        let (bpm, loop_bars) = parse_tempo(first, index + 1)?;
        let mut state = ParseState::new(bpm, loop_bars);
        state.consume(lines)?;
        state.finish()
    }

    pub fn used_sample_assets(&self) -> Vec<crate::sample::SampleAsset> {
        self.sample_assets
            .iter()
            .filter(|(name, _)| self.sample_events.iter().any(|e| &e.sample == *name))
            .map(|(_, asset)| asset.clone())
            .collect()
    }

    pub fn tone_for_track(&self, track: &str) -> BassTone {
        self.track_tones.get(track).copied().unwrap_or(self.bass_tone)
    }

    pub fn channel_for_track(&self, track: &str) -> Option<&ChannelStrip> {
        self.channels.get(track)
    }

    pub fn without_track(&self, name: &str) -> Result<Self, String> {
        if !self.has_track(name) {
            return Err(format!("unknown track: {name}"));
        }
        let score = self.removed_track(name);
        score.ensure_playback_after_mute()?;
        Ok(score)
    }

    fn removed_track(&self, name: &str) -> Self {
        let mut score = self.clone();
        score.events.retain(|event| event.track != name);
        score.sample_events.retain(|event| event.track != name);
        score.channels.remove(name);
        score
    }

    fn has_track(&self, track: &str) -> bool {
        self.events.iter().any(|event| event.track == track)
            || self.sample_events.iter().any(|event| event.track == track)
    }

    fn ensure_playback_after_mute(&self) -> Result<(), String> {
        if self.events.is_empty() && self.sample_events.is_empty() {
            return Err("muting this track leaves no notes".to_owned());
        }
        Ok(())
    }
}

fn nonblank_lines(input: &str) -> impl Iterator<Item = (usize, &str)> {
    input.lines().enumerate().filter(|(_, line)| !line.trim().is_empty())
}

struct ParseState {
    score: Score,
    playback_seen: bool,
    tone_seen: bool,
}

fn parse_tempo(first: &str, line: usize) -> Result<(f64, Option<usize>), String> {
    let tempo = parse_object(first, line)?;
    check_fields_with_optional(&tempo, &["type", "bpm"], &["bars"], line)?;
    validate_tempo_type(&tempo, line)?;
    let bpm = get_f64(&tempo, "bpm", line)?;
    validate_bpm(bpm, line)?;
    Ok((bpm, parse_bars(&tempo, line)?))
}

fn validate_tempo_type(tempo: &JsonObject, line: usize) -> Result<(), String> {
    if get_str(tempo, "type", line)? != "tempo" {
        return Err(format!("line {line}: first event must be tempo"));
    }
    Ok(())
}

fn validate_bpm(bpm: f64, line: usize) -> Result<(), String> {
    if !bpm.is_finite() || bpm <= 0.0 {
        return Err(format!("line {line}: bpm must be finite and positive"));
    }
    Ok(())
}

fn parse_bars(tempo: &JsonObject, line: usize) -> Result<Option<usize>, String> {
    let Some(value) = tempo.get("bars") else {
        return Ok(None);
    };
    match value.as_u64() {
        Some(bars @ 1..=1024) => Ok(Some(bars as usize)),
        _ => Err(invalid_bars(line)),
    }
}

fn invalid_bars(line: usize) -> String {
    format!("line {line}: bars must be an integer from 1 to 1024")
}

impl ParseState {
    fn consume<'a>(&mut self, lines: impl Iterator<Item = (usize, &'a str)>) -> Result<(), String> {
        for (index, line) in lines {
            self.apply_record(line, index + 1)?;
        }
        Ok(())
    }

    fn new(bpm: f64, loop_bars: Option<usize>) -> Self {
        Self { score: Score::with_tempo(bpm, loop_bars), playback_seen: false, tone_seen: false }
    }

    fn apply_record(&mut self, text: &str, line: usize) -> Result<(), String> {
        let object = parse_object(text, line)?;
        match get_str(&object, "type", line)? {
            "sample_note" => self.sample_note(&object, line),
            "note" => self.note(&object, line),
            kind => self.declaration(kind, &object, line),
        }
    }

    fn declaration(&mut self, kind: &str, object: &JsonObject, line: usize) -> Result<(), String> {
        match kind {
            "aux_return" => self.aux_return(object, line),
            "channel" => self.channel(object, line),
            "tone" => self.tone(object, line),
            "sample" => self.sample(object, line),
            other => Err(format!("line {line}: unsupported event type {other}")),
        }
    }

    fn before_playback(&self, line: usize, kind: &str) -> Result<(), String> {
        if self.playback_seen {
            return Err(format!("line {line}: {kind}"));
        }
        Ok(())
    }

    fn aux_return(&mut self, object: &JsonObject, line: usize) -> Result<(), String> {
        self.before_playback(line, "aux_return must precede playback")?;
        let name = get_str(object, "name", line)?;
        validate_name(name, line)?;
        let effect = AuxReturn::from_object(object, line)?;
        self.add_aux(name, effect, line)
    }

    fn add_aux(&mut self, name: &str, effect: AuxReturn, line: usize) -> Result<(), String> {
        let previous = self.score.aux_returns.insert(name.to_owned(), effect);
        if previous.is_some() {
            return Err(format!("line {line}: duplicate aux return {name}"));
        }
        Ok(())
    }

    fn channel(&mut self, object: &JsonObject, line: usize) -> Result<(), String> {
        self.before_playback(line, "channel must precede playback")?;
        let track = get_str(object, "track", line)?;
        validate_name(track, line)?;
        let channel = ChannelStrip::from_object(object, line)?;
        self.add_channel(track, channel, line)
    }

    fn add_channel(&mut self, track: &str, channel: ChannelStrip, line: usize) -> ModelResult<()> {
        let previous = self.score.channels.insert(track.to_owned(), channel);
        if previous.is_some() {
            return Err(format!("line {line}: duplicate channel for track {track}"));
        }
        Ok(())
    }

    fn tone(&mut self, object: &JsonObject, line: usize) -> Result<(), String> {
        self.before_playback(line, "tone must appear once before notes")?;
        let tone = BassTone::from_object(object, line)?;
        if let Some(track) = optional_track(object, line)? {
            self.track_tone(track, tone, line)
        } else {
            self.default_tone(tone, line)
        }
    }

    fn track_tone(&mut self, track: &str, tone: BassTone, line: usize) -> Result<(), String> {
        let previous = self.score.track_tones.insert(track.to_owned(), tone);
        if previous.is_some() {
            return Err(format!("line {line}: duplicate tone for track {track}"));
        }
        Ok(())
    }

    fn default_tone(&mut self, tone: BassTone, line: usize) -> Result<(), String> {
        if self.tone_seen {
            return Err(format!("line {line}: tone must appear once before notes"));
        }
        self.score.bass_tone = tone;
        self.tone_seen = true;
        Ok(())
    }

    fn sample(&mut self, object: &JsonObject, line: usize) -> Result<(), String> {
        self.before_playback(line, "sample declarations must precede playback")?;
        check_fields(object, &["type", "name", "path"], line)?;
        let name = get_str(object, "name", line)?;
        validate_name(name, line)?;
        let path = get_str(object, "path", line)?;
        validate_sample_path(path, line)?;
        self.insert_sample(name, path, line)
    }

    fn insert_sample(&mut self, name: &str, path: &str, line: usize) -> Result<(), String> {
        let previous = self.score.samples.insert(name.to_owned(), path.to_owned());
        if previous.is_some() {
            return Err(format!("line {line}: duplicate sample {name}"));
        }
        Ok(())
    }

    fn sample_note(&mut self, object: &JsonObject, line: usize) -> Result<(), String> {
        self.playback_seen = true;
        let event = self.parse_sample_note(object, line)?;
        self.score.sample_events.push(event);
        Ok(())
    }

    fn parse_sample_note(&self, object: &JsonObject, line: usize) -> Result<SampleEvent, String> {
        check_sample_note_fields(object, line)?;
        let sample = self.sample_name(object, line)?;
        let beat = sample_beat(object, line)?;
        let velocity = sample_velocity(object, line)?;
        let track = optional_track(object, line)?.unwrap_or("main");
        Ok(SampleEvent::new(sample, track, beat, velocity, line))
    }

    fn sample_name<'a>(&self, object: &'a JsonObject, line: usize) -> Result<&'a str, String> {
        let sample = get_str(object, "sample", line)?;
        if !self.score.samples.contains_key(sample) {
            return Err(format!("line {line}: undeclared sample {sample}"));
        }
        Ok(sample)
    }

    fn note(&mut self, object: &JsonObject, line: usize) -> Result<(), String> {
        self.playback_seen = true;
        self.score.events.push(MusicEvent::from_object(object, line)?);
        Ok(())
    }

    fn finish(mut self) -> Result<Score, String> {
        self.validate_playback()?;
        self.validate_channels()?;
        self.validate_aux_returns()?;
        self.validate_bars()?;
        self.score.events.sort_by(|a, b| a.beat.total_cmp(&b.beat));
        Ok(self.score)
    }

    fn validate_playback(&self) -> Result<(), String> {
        if self.score.events.is_empty() && self.score.sample_events.is_empty() {
            return Err("score needs at least one note".to_owned());
        }
        Ok(())
    }

    fn validate_channels(&self) -> Result<(), String> {
        for track in self.score.channels.keys() {
            if !self.has_track(track) {
                return Err(format!("channel has no playback track: {track}"));
            }
        }
        Ok(())
    }

    fn has_track(&self, track: &str) -> bool {
        self.score.has_track(track)
    }

    fn validate_aux_returns(&self) -> Result<(), String> {
        for channel in self.score.channels.values() {
            if let Some(send) = &channel.aux_send
                && !self.score.aux_returns.contains_key(&send.return_name)
            {
                return Err(format!("undeclared aux return: {}", send.return_name));
            }
        }
        Ok(())
    }

    fn validate_bars(&self) -> Result<(), String> {
        if let Some(bars) = self.score.loop_bars {
            let end_beat = bars as f64 * 4.0;
            if self.event_outside(end_beat) {
                return Err("playback event starts outside declared bars".to_owned());
            }
        }
        Ok(())
    }

    fn event_outside(&self, end_beat: f64) -> bool {
        self.score.events.iter().any(|event| event.beat >= end_beat)
            || self.score.sample_events.iter().any(|event| event.beat >= end_beat)
    }
}

fn check_sample_note_fields(object: &JsonObject, line: usize) -> Result<(), String> {
    check_fields_with_optional(object, &["type", "sample", "beat", "velocity"], &["track"], line)
}

fn check_beat(beat: f64, line: usize) -> Result<(), String> {
    if !beat.is_finite() || beat < 0.0 {
        return Err(format!("line {line}: beat must be finite and nonnegative"));
    }
    Ok(())
}

fn sample_beat(object: &JsonObject, line: usize) -> Result<f64, String> {
    let beat = get_f64(object, "beat", line)?;
    check_beat(beat, line)?;
    Ok(beat)
}

fn sample_velocity(object: &JsonObject, line: usize) -> Result<f64, String> {
    let velocity = get_f64(object, "velocity", line)?;
    check_range(velocity, UNIT_RANGE, "velocity must be between 0 and 1", line)?;
    Ok(velocity)
}

fn validate_sample_path(path: &str, line: usize) -> Result<(), String> {
    if path.is_empty() || path.contains('\0') {
        return Err(format!("line {line}: invalid sample path"));
    }
    Ok(())
}

impl BassTone {
    fn from_object(object: &JsonObject, line: usize) -> ModelResult<Self> {
        check_tone_fields(object, line)?;
        validate_tone_instrument(object, line)?;
        let values = ToneValues::read(object, line)?;
        values.validate(object, line)?;
        Ok(values.into_tone())
    }
}

const TONE_REQUIRED: &[&str] =
    &["type", "instrument", "cutoff_hz", "attack_seconds", "release_seconds"];
const TONE_OPTIONAL: &[&str] =
    &["track", "oscillator", "decay_seconds", "sustain_level", "lfo_rate_hz", "lfo_depth", "gain"];

fn check_tone_fields(object: &JsonObject, line: usize) -> ModelResult<()> {
    check_fields_with_optional(object, TONE_REQUIRED, TONE_OPTIONAL, line)
}

fn validate_tone_instrument(object: &JsonObject, line: usize) -> ModelResult<()> {
    if get_str(object, "instrument", line)? != "analog_bass" {
        return Err(format!("line {line}: unsupported tone instrument"));
    }
    Ok(())
}

struct ToneValues {
    cutoff_hz: f64,
    envelope: EnvelopeValues,
    modulation: ModulationValues,
    gain: f64,
    oscillator: Oscillator,
}

impl ToneValues {
    fn read(object: &JsonObject, line: usize) -> ModelResult<Self> {
        Ok(Self {
            cutoff_hz: get_f64(object, "cutoff_hz", line)?,
            envelope: EnvelopeValues::read(object, line)?,
            modulation: ModulationValues::read(object, line)?,
            gain: optional_f64(object, "gain", 1.0, line)?,
            oscillator: parse_oscillator(object, line)?,
        })
    }

    fn validate(&self, object: &JsonObject, line: usize) -> ModelResult<()> {
        self.validate_cutoff(line)?;
        self.envelope.validate(line)?;
        self.modulation.validate(object, line)?;
        check_range(self.gain, 0.0..=4.0, "gain must be 0..4", line)
    }

    fn validate_cutoff(&self, line: usize) -> ModelResult<()> {
        check_range(self.cutoff_hz, 40.0..=18_000.0, "cutoff_hz must be 40..18000", line)
    }

    fn into_tone(self) -> BassTone {
        let mut tone = DEFAULT_BASS_TONE;
        tone.cutoff_hz = self.cutoff_hz as f32;
        tone.gain = self.gain as f32;
        tone.oscillator = self.oscillator;
        self.envelope.apply_to(&mut tone);
        self.modulation.apply_to(&mut tone);
        tone
    }
}

struct EnvelopeValues {
    attack: f64,
    decay: f64,
    sustain: f64,
    release: f64,
}

impl EnvelopeValues {
    fn read(object: &JsonObject, line: usize) -> ModelResult<Self> {
        Ok(Self {
            attack: get_f64(object, "attack_seconds", line)?,
            decay: optional_f64(object, "decay_seconds", 0.0, line)?,
            sustain: optional_f64(object, "sustain_level", 1.0, line)?,
            release: get_f64(object, "release_seconds", line)?,
        })
    }

    fn validate(&self, line: usize) -> ModelResult<()> {
        check_range(self.attack, 0.0..=5.0, "attack_seconds must be 0..5", line)?;
        self.validate_release(line)?;
        check_range(self.decay, 0.0..=5.0, "decay_seconds must be 0..5", line)?;
        check_range(self.sustain, UNIT_RANGE, "sustain_level must be 0..1", line)
    }

    fn validate_release(&self, line: usize) -> ModelResult<()> {
        check_range(self.release, 0.0..=5.0, "release_seconds must be 0..5", line)
    }

    fn apply_to(self, tone: &mut BassTone) {
        tone.attack_seconds = self.attack as f32;
        tone.decay_seconds = self.decay as f32;
        tone.sustain_level = self.sustain as f32;
        tone.release_seconds = self.release as f32;
    }
}

struct ModulationValues {
    rate: f64,
    depth: f64,
}

impl ModulationValues {
    fn read(object: &JsonObject, line: usize) -> ModelResult<Self> {
        Ok(Self {
            rate: optional_f64(object, "lfo_rate_hz", 0.0, line)?,
            depth: optional_f64(object, "lfo_depth", 0.0, line)?,
        })
    }

    fn validate(&self, object: &JsonObject, line: usize) -> ModelResult<()> {
        validate_lfo_presence(object, line)?;
        if object.contains_key("lfo_rate_hz") {
            check_range(self.rate, 0.1..=20.0, "lfo_rate_hz must be 0.1..20", line)?;
        }
        check_range(self.depth, UNIT_RANGE, "lfo_depth must be 0..1", line)
    }

    fn apply_to(self, tone: &mut BassTone) {
        tone.lfo_rate_hz = self.rate as f32;
        tone.lfo_depth = self.depth as f32;
    }
}

fn validate_lfo_presence(object: &JsonObject, line: usize) -> ModelResult<()> {
    if object.contains_key("lfo_rate_hz") != object.contains_key("lfo_depth") {
        return Err(format!("line {line}: lfo_rate_hz and lfo_depth must appear together"));
    }
    Ok(())
}

fn parse_oscillator(object: &JsonObject, line: usize) -> ModelResult<Oscillator> {
    match object.get("oscillator") {
        None => Ok(Oscillator::Saw),
        Some(Value::String(name)) if name == "saw" => Ok(Oscillator::Saw),
        Some(Value::String(name)) if name == "sine" => Ok(Oscillator::Sine),
        Some(Value::String(name)) if name == "pulse" => Ok(Oscillator::Pulse),
        Some(Value::String(name)) if name == "noise" => Ok(Oscillator::Noise),
        _ => Err(invalid_oscillator(line)),
    }
}

fn invalid_oscillator(line: usize) -> String {
    format!("line {line}: oscillator must be saw, sine, pulse, or noise")
}

impl MusicEvent {
    fn from_object(object: &JsonObject, line_number: usize) -> Result<Self, String> {
        check_note_fields(object, line_number)?;
        let instrument = note_instrument(object, line_number)?;
        let track = optional_track(object, line_number)?.unwrap_or("main");
        let values = NoteValues::parse(object, line_number)?;
        Ok(values.into_event(instrument, track, line_number))
    }
}

const NOTE_FIELDS: &[&str] = &["type", "instrument", "note", "beat", "duration_beats", "velocity"];

fn check_note_fields(object: &JsonObject, line: usize) -> ModelResult<()> {
    check_fields_with_optional(object, NOTE_FIELDS, &["track"], line)
}

fn note_instrument(object: &JsonObject, line_number: usize) -> ModelResult<&str> {
    let instrument = get_str(object, "instrument", line_number)?;
    if instrument != "analog_bass" {
        return Err(format!("line {line_number}: unsupported instrument {instrument}"));
    }
    Ok(instrument)
}

struct NoteValues {
    note: u8,
    beat: f64,
    duration_beats: f64,
    velocity: f64,
}

impl NoteValues {
    fn parse(object: &JsonObject, line: usize) -> ModelResult<Self> {
        let values = Self::read(object, line)?;
        values.validate(line)?;
        Ok(values)
    }

    fn read(object: &JsonObject, line: usize) -> ModelResult<Self> {
        Ok(Self {
            note: note_number(object, line)?,
            beat: get_f64(object, "beat", line)?,
            duration_beats: get_f64(object, "duration_beats", line)?,
            velocity: get_f64(object, "velocity", line)?,
        })
    }

    fn into_event(self, instrument: &str, track: &str, line: usize) -> MusicEvent {
        let (note, beat) = (self.note, self.beat);
        let (duration_beats, velocity) = (self.duration_beats, self.velocity);
        let (instrument, track) = (instrument.to_owned(), track.to_owned());
        MusicEvent { order: line, instrument, track, note, beat, duration_beats, velocity }
    }

    fn validate(&self, line: usize) -> ModelResult<()> {
        check_beat(self.beat, line)?;
        check_note_duration(self.duration_beats, line)?;
        check_range(self.velocity, UNIT_RANGE, "velocity must be between 0 and 1", line)
    }
}

fn note_number(object: &JsonObject, line_number: usize) -> ModelResult<u8> {
    let note = object
        .get("note")
        .and_then(Value::as_u64)
        .filter(|note| *note <= 127)
        .ok_or_else(|| format!("line {line_number}: note must be an integer from 0 to 127"))?
        as u8;
    Ok(note)
}

fn check_note_duration(duration: f64, line: usize) -> ModelResult<()> {
    if !duration.is_finite() || duration <= 0.0 {
        return Err(format!("line {line}: duration_beats must be finite and positive"));
    }
    Ok(())
}

fn parse_object(line: &str, line_number: usize) -> Result<JsonObject, String> {
    let value: Value = serde_json::from_str(line)
        .map_err(|error| format!("line {line_number}: invalid JSON: {error}"))?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| format!("line {line_number}: event must be a JSON object"))
}

fn nested_object<'a>(value: &'a Value, name: &str, line: usize) -> Result<&'a JsonObject, String> {
    value.as_object().ok_or_else(|| format!("line {line}: {name} must be an object"))
}

fn check_fields(object: &JsonObject, expected: &[&str], line: usize) -> Result<(), String> {
    check_fields_with_optional(object, expected, &[], line)
}

fn check_fields_with_optional(
    object: &JsonObject,
    expected: &[&str],
    optional: &[&str],
    line: usize,
) -> ModelResult<()> {
    check_extra(object, expected, optional, line)?;
    check_missing_fields(object, expected, line)
}

fn check_extra(
    object: &JsonObject,
    expected: &[&str],
    optional: &[&str],
    line: usize,
) -> ModelResult<()> {
    let unknown = unknown_key(object, expected, optional);
    unknown.map_or(Ok(()), |key| Err(unknown_field_error(key, line)))
}

fn unknown_field_error(key: &str, line: usize) -> String {
    format!("line {line}: unknown field {key}")
}

fn unknown_key<'a>(
    object: &'a JsonObject,
    expected: &[&str],
    optional: &[&str],
) -> Option<&'a str> {
    object.keys().find(|key| !known_field(key, expected, optional)).map(String::as_str)
}

fn known_field(key: &str, expected: &[&str], optional: &[&str]) -> bool {
    expected.contains(&key) || optional.contains(&key)
}

fn check_missing_fields(object: &JsonObject, expected: &[&str], line: usize) -> ModelResult<()> {
    for key in expected {
        if !object.contains_key(*key) {
            return Err(format!("line {line}: missing field {key}"));
        }
    }
    Ok(())
}

fn get_str<'a>(object: &'a JsonObject, key: &str, line: usize) -> Result<&'a str, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("line {line}: {key} must be a string"))
}

fn get_f64(object: &JsonObject, key: &str, line: usize) -> Result<f64, String> {
    object
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("line {line}: {key} must be a number"))
}

fn optional_f64(object: &JsonObject, key: &str, default: f64, line: usize) -> Result<f64, String> {
    match object.get(key) {
        None => Ok(default),
        Some(_) => get_f64(object, key, line),
    }
}

fn check_range(value: f64, range: NumberRange, message: &str, line: usize) -> ModelResult<()> {
    if !value.is_finite() || !range.contains(&value) {
        return Err(format!("line {line}: {message}"));
    }
    Ok(())
}

fn optional_track(object: &JsonObject, line_number: usize) -> Result<Option<&str>, String> {
    match object.get("track") {
        None => Ok(None),
        Some(Value::String(name)) if valid_ascii_name(name) => Ok(Some(name.as_str())),
        _ => Err(format!("line {line_number}: track must be a 1..32 character ASCII name")),
    }
}

fn validate_name(name: &str, line: usize) -> Result<(), String> {
    if !valid_ascii_name(name) {
        return Err(format!(
            "line {line}: sample name must be 1..32 ASCII letters, digits, _ or -"
        ));
    }
    Ok(())
}

fn valid_ascii_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}
