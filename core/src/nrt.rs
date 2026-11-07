//! Offline rendering: the live encoders write a `scsynth -N` command file instead of UDP.

use std::cell::RefCell;
use std::path::Path;
use std::time::Duration;

use crate::mixer::{ChannelGraph, GraphBackend, NodeSpec};
use crate::model::Score;
use crate::osc::{
    Lookups, SAMPLE_SYNTHDEFS, encode_alloc_read, encode_graph_groups, encode_graph_node,
    encode_load, encode_messages_bundle, encode_routed_playback_bundle,
};
use crate::sample::SampleAsset;
use crate::sampler::{SampleBackend, SampleBuffers};
use crate::schedule::{PlaybackEvent, Scheduler};

/// NRT timetags count seconds from the score start, not wall-clock NTP dates.
pub fn elapsed_timetag(at: Duration) -> u64 {
    let fraction = (u64::from(at.subsec_nanos()) << 32) / 1_000_000_000;
    (at.as_secs() << 32) | fraction
}

/// Length-prefixed OSC bundles in time order, the file format `scsynth -N` reads.
#[derive(Debug, Default)]
pub struct NrtScore {
    bytes: Vec<u8>,
}

impl NrtScore {
    pub fn push_bundle(&mut self, bundle: &[u8]) {
        self.bytes.extend_from_slice(&(bundle.len() as u32).to_be_bytes());
        self.bytes.extend_from_slice(bundle);
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Commands for one validated score. `length` fixes the file length; otherwise the
/// file ends when the last event and any reverb tail have finished, as live playback waits.
pub fn score_commands(
    score: &Score,
    synthdef_dir: &Path,
    length: Option<Duration>,
) -> Result<NrtScore, String> {
    let recorder = Recorder { commands: RefCell::default(), synthdef_dir };
    let end = OfflineEngine::new(&recorder).record(score, length)?;
    recorder.end_at(end);
    Ok(recorder.commands.into_inner())
}

/// Engine-side commands are queued at time zero; NRT runs them before any event.
struct Recorder<'a> {
    commands: RefCell<NrtScore>,
    synthdef_dir: &'a Path,
}

impl Recorder<'_> {
    fn setup(&self, message: Vec<u8>) {
        self.at(Duration::ZERO, &[message]);
    }

    fn at(&self, at: Duration, messages: &[Vec<u8>]) {
        self.push(&encode_messages_bundle(elapsed_timetag(at), messages));
    }

    fn push(&self, bundle: &[u8]) {
        self.commands.borrow_mut().push_bundle(bundle);
    }

    fn load_definition(&self, name: &str) -> Result<(), String> {
        let path = self.synthdef_dir.join(format!("{name}.scsyndef"));
        let path = path.to_str().ok_or("SynthDef path must be UTF-8")?;
        self.setup(encode_load(path));
        Ok(())
    }

    /// An empty bundle sets the rendered length.
    fn end_at(&self, end: Duration) {
        self.at(end, &[]);
    }
}

/// Every node and buffer lives until the file ends, so frees are not recorded.
impl SampleBackend for &Recorder<'_> {
    fn load(&self, id: i32, asset: &SampleAsset) -> Result<(), String> {
        let path = asset.path.to_str().ok_or("sample path must be UTF-8")?;
        self.setup(encode_alloc_read(id, path));
        Ok(())
    }

    fn free(&self, _id: i32) -> Result<(), String> {
        Ok(())
    }
}

impl GraphBackend for &Recorder<'_> {
    fn start(&self) -> Result<(), String> {
        self.load_definition("channel_strip")?;
        self.load_definition("aux_return")?;
        self.setup(encode_graph_groups());
        Ok(())
    }

    fn add_node(&self, spec: &NodeSpec<'_>) -> Result<(), String> {
        self.setup(encode_graph_node(spec));
        Ok(())
    }

    fn free_node(&self, _node: i32) -> Result<(), String> {
        Ok(())
    }

    fn stop(&self) -> Result<(), String> {
        Ok(())
    }
}

/// The live engine's buffer and channel-graph preparation, recorded instead of sent.
struct OfflineEngine<'r, 'a> {
    recorder: &'r Recorder<'a>,
    buffers: SampleBuffers<&'r Recorder<'a>>,
    graph: ChannelGraph<&'r Recorder<'a>>,
}

impl<'r, 'a> OfflineEngine<'r, 'a> {
    fn new(recorder: &'r Recorder<'a>) -> Self {
        let (buffers, graph) = (SampleBuffers::new(recorder), ChannelGraph::new(recorder));
        Self { recorder, buffers, graph }
    }

    /// Records preparation and every event, and returns when the file should end.
    fn record(mut self, score: &Score, length: Option<Duration>) -> Result<Duration, String> {
        self.prepare(score)?;
        let events = Scheduler::plan_playback(score)?;
        self.play(&events)?;
        length.map_or_else(|| self.natural_end(&events), Ok)
    }

    fn prepare(&mut self, score: &Score) -> Result<(), String> {
        self.recorder.load_definition("analog_bass")?;
        let assets = score.used_sample_assets();
        if !assets.is_empty() {
            SAMPLE_SYNTHDEFS.iter().try_for_each(|name| self.recorder.load_definition(name))?;
        }
        self.buffers.prepare(assets.iter())?;
        self.graph.prepare(score)
    }

    /// Simultaneous events share one bundle, as in live playback.
    fn play(&self, events: &[PlaybackEvent]) -> Result<(), String> {
        let sample = |asset: &SampleAsset| self.buffers.buffer_id(asset);
        let route = |track: &str| self.graph.route_bus(track);
        let lookups = Lookups { sample: &sample, route: &route };
        for group in events.chunk_by(|a, b| a.at == b.at) {
            let at = elapsed_timetag(group[0].at);
            self.recorder.push(&encode_routed_playback_bundle(at, group, &lookups)?);
        }
        Ok(())
    }

    fn natural_end(&self, events: &[PlaybackEvent]) -> Result<Duration, String> {
        let ends = events.iter().map(PlaybackEvent::end).collect::<Result<Vec<_>, _>>()?;
        Ok(ends.into_iter().max().unwrap_or_default() + self.graph.tail())
    }
}
