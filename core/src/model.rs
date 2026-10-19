use std::collections::BTreeMap;
use std::ops::RangeInclusive;

use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct SampleEvent {
    pub sample: String,
    pub track: String,
    pub beat: f64,
    pub velocity: f64,
    pub order: usize,
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

impl Default for BassTone {
    fn default() -> Self {
        Self {
            oscillator: Oscillator::Saw,
            cutoff_hz: 0.0, // Track the note at four times its fundamental.
            attack_seconds: 0.01,
            decay_seconds: 0.0,
            sustain_level: 1.0,
            release_seconds: 0.09,
            lfo_rate_hz: 0.0,
            lfo_depth: 0.0,
            gain: 2.8,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Score {
    pub bpm: f64,
    pub bass_tone: BassTone,
    pub track_tones: BTreeMap<String, BassTone>,
    pub events: Vec<MusicEvent>,
    pub samples: BTreeMap<String, String>,
    pub sample_events: Vec<SampleEvent>,
    pub sample_assets: BTreeMap<String, crate::sample::SampleAsset>,
}

impl Score {
    pub fn parse_ndjson(input: &str) -> Result<Self, String> {
        let mut lines = input
            .lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty());
        let (tempo_line, first) = lines.next().ok_or("score is empty")?;
        let tempo = parse_object(first, tempo_line + 1)?;
        check_fields(&tempo, &["type", "bpm"], tempo_line + 1)?;
        if get_str(&tempo, "type", tempo_line + 1)? != "tempo" {
            return Err(format!(
                "line {}: first event must be tempo",
                tempo_line + 1
            ));
        }
        let bpm = get_f64(&tempo, "bpm", tempo_line + 1)?;
        if !bpm.is_finite() || bpm <= 0.0 {
            return Err(format!(
                "line {}: bpm must be finite and positive",
                tempo_line + 1
            ));
        }

        let mut events = Vec::new();
        let mut samples = BTreeMap::new();
        let mut sample_events = Vec::new();
        let mut playback_seen = false;
        let mut bass_tone = BassTone::default();
        let mut track_tones = BTreeMap::new();
        let mut tone_seen = false;
        for (index, line) in lines {
            let line_number = index + 1;
            let object = parse_object(line, line_number)?;
            match get_str(&object, "type", line_number)? {
                "tone" => {
                    if playback_seen {
                        return Err(format!(
                            "line {line_number}: tone must appear once before notes"
                        ));
                    }
                    let tone = BassTone::from_object(&object, line_number)?;
                    if let Some(track) = optional_track(&object, line_number)? {
                        if track_tones.insert(track.to_owned(), tone).is_some() {
                            return Err(format!(
                                "line {line_number}: duplicate tone for track {track}"
                            ));
                        }
                    } else {
                        if tone_seen {
                            return Err(format!(
                                "line {line_number}: tone must appear once before notes"
                            ));
                        }
                        bass_tone = tone;
                        tone_seen = true;
                    }
                }
                "sample" => {
                    if playback_seen {
                        return Err(format!(
                            "line {line_number}: sample declarations must precede playback"
                        ));
                    }
                    check_fields(&object, &["type", "name", "path"], line_number)?;
                    let name = get_str(&object, "name", line_number)?;
                    validate_name(name, line_number)?;
                    let path = get_str(&object, "path", line_number)?;
                    if path.is_empty() || path.contains('\0') {
                        return Err(format!("line {line_number}: invalid sample path"));
                    }
                    if samples.insert(name.to_owned(), path.to_owned()).is_some() {
                        return Err(format!("line {line_number}: duplicate sample {name}"));
                    }
                }
                "sample_note" => {
                    playback_seen = true;
                    check_fields_with_optional(
                        &object,
                        &["type", "sample", "beat", "velocity"],
                        &["track"],
                        line_number,
                    )?;
                    let sample = get_str(&object, "sample", line_number)?;
                    if !samples.contains_key(sample) {
                        return Err(format!("line {line_number}: undeclared sample {sample}"));
                    }
                    let beat = get_f64(&object, "beat", line_number)?;
                    if !beat.is_finite() || beat < 0.0 {
                        return Err(format!(
                            "line {line_number}: beat must be finite and nonnegative"
                        ));
                    }
                    let velocity = get_f64(&object, "velocity", line_number)?;
                    check_range(
                        velocity,
                        0.0..=1.0,
                        "velocity must be between 0 and 1",
                        line_number,
                    )?;
                    sample_events.push(SampleEvent {
                        sample: sample.to_owned(),
                        track: optional_track(&object, line_number)?
                            .unwrap_or("main")
                            .to_owned(),
                        beat,
                        velocity,
                        order: line_number,
                    });
                }
                "note" => {
                    playback_seen = true;
                    events.push(MusicEvent::from_object(&object, line_number)?);
                }
                other => {
                    return Err(format!(
                        "line {line_number}: unsupported event type {other}"
                    ));
                }
            }
        }
        if events.is_empty() && sample_events.is_empty() {
            return Err("score needs at least one note".to_owned());
        }
        events.sort_by(|a, b| a.beat.total_cmp(&b.beat));
        Ok(Self {
            bpm,
            bass_tone,
            track_tones,
            events,
            samples,
            sample_events,
            sample_assets: BTreeMap::new(),
        })
    }

    pub fn used_sample_assets(&self) -> Vec<crate::sample::SampleAsset> {
        self.sample_assets
            .iter()
            .filter(|(name, _)| self.sample_events.iter().any(|e| &e.sample == *name))
            .map(|(_, asset)| asset.clone())
            .collect()
    }

    pub fn tone_for_track(&self, track: &str) -> BassTone {
        self.track_tones
            .get(track)
            .copied()
            .unwrap_or(self.bass_tone)
    }

    pub fn without_track(&self, name: &str) -> Result<Self, String> {
        if !self.events.iter().any(|event| event.track == name)
            && !self.sample_events.iter().any(|event| event.track == name)
        {
            return Err(format!("unknown track: {name}"));
        }
        let mut score = self.clone();
        score.events.retain(|event| event.track != name);
        score.sample_events.retain(|event| event.track != name);
        if score.events.is_empty() && score.sample_events.is_empty() {
            return Err("muting this track leaves no notes".to_owned());
        }
        Ok(score)
    }
}

impl BassTone {
    fn from_object(object: &Map<String, Value>, line_number: usize) -> Result<Self, String> {
        check_fields_with_optional(
            object,
            &[
                "type",
                "instrument",
                "cutoff_hz",
                "attack_seconds",
                "release_seconds",
            ],
            &[
                "track",
                "oscillator",
                "decay_seconds",
                "sustain_level",
                "lfo_rate_hz",
                "lfo_depth",
                "gain",
            ],
            line_number,
        )?;
        if get_str(object, "instrument", line_number)? != "analog_bass" {
            return Err(format!("line {line_number}: unsupported tone instrument"));
        }
        let cutoff_hz = get_f64(object, "cutoff_hz", line_number)?;
        let attack_seconds = get_f64(object, "attack_seconds", line_number)?;
        let decay_seconds = optional_f64(object, "decay_seconds", 0.0, line_number)?;
        let sustain_level = optional_f64(object, "sustain_level", 1.0, line_number)?;
        let release_seconds = get_f64(object, "release_seconds", line_number)?;
        let lfo_rate_hz = optional_f64(object, "lfo_rate_hz", 0.0, line_number)?;
        let lfo_depth = optional_f64(object, "lfo_depth", 0.0, line_number)?;
        let gain = optional_f64(object, "gain", 1.0, line_number)?;
        let oscillator = match object.get("oscillator") {
            None => Oscillator::Saw,
            Some(Value::String(name)) if name == "saw" => Oscillator::Saw,
            Some(Value::String(name)) if name == "sine" => Oscillator::Sine,
            Some(Value::String(name)) if name == "pulse" => Oscillator::Pulse,
            Some(Value::String(name)) if name == "noise" => Oscillator::Noise,
            _ => {
                return Err(format!(
                    "line {line_number}: oscillator must be saw, sine, pulse, or noise"
                ));
            }
        };
        check_range(
            cutoff_hz,
            40.0..=18_000.0,
            "cutoff_hz must be 40..18000",
            line_number,
        )?;
        check_range(
            attack_seconds,
            0.0..=5.0,
            "attack_seconds must be 0..5",
            line_number,
        )?;
        check_range(
            release_seconds,
            0.0..=5.0,
            "release_seconds must be 0..5",
            line_number,
        )?;
        check_range(
            decay_seconds,
            0.0..=5.0,
            "decay_seconds must be 0..5",
            line_number,
        )?;
        check_range(
            sustain_level,
            0.0..=1.0,
            "sustain_level must be 0..1",
            line_number,
        )?;
        if object.contains_key("lfo_rate_hz") != object.contains_key("lfo_depth") {
            return Err(format!(
                "line {line_number}: lfo_rate_hz and lfo_depth must appear together"
            ));
        }
        if object.contains_key("lfo_rate_hz") {
            check_range(
                lfo_rate_hz,
                0.1..=20.0,
                "lfo_rate_hz must be 0.1..20",
                line_number,
            )?;
        }
        check_range(lfo_depth, 0.0..=1.0, "lfo_depth must be 0..1", line_number)?;
        check_range(gain, 0.0..=4.0, "gain must be 0..4", line_number)?;
        Ok(Self {
            oscillator,
            cutoff_hz: cutoff_hz as f32,
            attack_seconds: attack_seconds as f32,
            decay_seconds: decay_seconds as f32,
            sustain_level: sustain_level as f32,
            release_seconds: release_seconds as f32,
            lfo_rate_hz: lfo_rate_hz as f32,
            lfo_depth: lfo_depth as f32,
            gain: gain as f32,
        })
    }
}

impl MusicEvent {
    fn from_object(object: &Map<String, Value>, line_number: usize) -> Result<Self, String> {
        check_fields_with_optional(
            object,
            &[
                "type",
                "instrument",
                "note",
                "beat",
                "duration_beats",
                "velocity",
            ],
            &["track"],
            line_number,
        )?;
        let instrument = get_str(object, "instrument", line_number)?;
        if instrument != "analog_bass" {
            return Err(format!(
                "line {line_number}: unsupported instrument {instrument}"
            ));
        }
        let track = optional_track(object, line_number)?.unwrap_or("main");
        let note = object
            .get("note")
            .and_then(Value::as_u64)
            .filter(|note| *note <= 127)
            .ok_or_else(|| format!("line {line_number}: note must be an integer from 0 to 127"))?
            as u8;
        let beat = get_f64(object, "beat", line_number)?;
        let duration_beats = get_f64(object, "duration_beats", line_number)?;
        let velocity = get_f64(object, "velocity", line_number)?;
        if !beat.is_finite() || beat < 0.0 {
            return Err(format!(
                "line {line_number}: beat must be finite and nonnegative"
            ));
        }
        if !duration_beats.is_finite() || duration_beats <= 0.0 {
            return Err(format!(
                "line {line_number}: duration_beats must be finite and positive"
            ));
        }
        check_range(
            velocity,
            0.0..=1.0,
            "velocity must be between 0 and 1",
            line_number,
        )?;
        Ok(Self {
            order: line_number,
            instrument: instrument.to_owned(),
            track: track.to_owned(),
            note,
            beat,
            duration_beats,
            velocity,
        })
    }
}

fn parse_object(line: &str, line_number: usize) -> Result<Map<String, Value>, String> {
    let value: Value = serde_json::from_str(line)
        .map_err(|error| format!("line {line_number}: invalid JSON: {error}"))?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| format!("line {line_number}: event must be a JSON object"))
}

fn check_fields(object: &Map<String, Value>, expected: &[&str], line: usize) -> Result<(), String> {
    check_fields_with_optional(object, expected, &[], line)
}

fn check_fields_with_optional(
    object: &Map<String, Value>,
    expected: &[&str],
    optional: &[&str],
    line: usize,
) -> Result<(), String> {
    for key in object.keys() {
        if !expected.contains(&key.as_str()) && !optional.contains(&key.as_str()) {
            return Err(format!("line {line}: unknown field {key}"));
        }
    }
    for key in expected {
        if !object.contains_key(*key) {
            return Err(format!("line {line}: missing field {key}"));
        }
    }
    Ok(())
}

fn get_str<'a>(object: &'a Map<String, Value>, key: &str, line: usize) -> Result<&'a str, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("line {line}: {key} must be a string"))
}

fn get_f64(object: &Map<String, Value>, key: &str, line: usize) -> Result<f64, String> {
    object
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("line {line}: {key} must be a number"))
}

fn optional_f64(
    object: &Map<String, Value>,
    key: &str,
    default: f64,
    line: usize,
) -> Result<f64, String> {
    match object.get(key) {
        None => Ok(default),
        Some(_) => get_f64(object, key, line),
    }
}

fn check_range(
    value: f64,
    range: RangeInclusive<f64>,
    message: &str,
    line: usize,
) -> Result<(), String> {
    if !value.is_finite() || !range.contains(&value) {
        return Err(format!("line {line}: {message}"));
    }
    Ok(())
}

fn optional_track(object: &Map<String, Value>, line_number: usize) -> Result<Option<&str>, String> {
    match object.get("track") {
        None => Ok(None),
        Some(Value::String(name))
            if !name.is_empty()
                && name.len() <= 32
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-') =>
        {
            Ok(Some(name.as_str()))
        }
        _ => Err(format!(
            "line {line_number}: track must be a 1..32 character ASCII name"
        )),
    }
}

fn validate_name(name: &str, line: usize) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 32
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(format!(
            "line {line}: sample name must be 1..32 ASCII letters, digits, _ or -"
        ));
    }
    Ok(())
}
