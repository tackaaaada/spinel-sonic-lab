//! Audio-engine ownership of channel strips and Aux returns.
//!
//! Music events carry only a logical route (the track name). This module owns
//! bus numbers, node IDs, boundary-timed control updates, and node lifetime.
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use crate::model::{AuxReturn, ChannelStrip, Compressor, Score};

pub const REVERB_TAIL: Duration = Duration::from_secs(3);
pub const STRIP_GROUP: i32 = 1900;
pub const RETURN_GROUP: i32 = 1901;
const NULL_BUS: i32 = 126;
const FIRST_BUS: i32 = 128;
const LAST_BUS: i32 = 1022;
const FIRST_NODE: i32 = 2000;

type GraphResult<T> = Result<T, String>;
pub type Controls = Vec<(&'static str, f32)>;
type NodeMap = BTreeMap<String, GraphNode>;

/// Control values for one engine node, sent at a bar boundary on reload.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeUpdate {
    pub node: i32,
    pub controls: Controls,
}

/// One engine node to create at the tail of `group`.
pub struct NodeSpec<'a> {
    pub def: &'a str,
    pub node: i32,
    pub group: i32,
    pub controls: &'a [(&'static str, f32)],
}

pub trait GraphBackend {
    /// Load definitions and create the strip group followed by the return group.
    fn start(&self) -> GraphResult<()>;
    fn add_node(&self, spec: &NodeSpec<'_>) -> GraphResult<()>;
    fn free_node(&self, node: i32) -> GraphResult<()>;
    fn stop(&self) -> GraphResult<()>;
}

/// The SynthDef and group shared by every node of one kind.
#[derive(Clone, Copy)]
struct Slot {
    def: &'static str,
    group: i32,
}

const STRIP_SLOT: Slot = Slot { def: "channel_strip", group: STRIP_GROUP };
const RETURN_SLOT: Slot = Slot { def: "aux_return", group: RETURN_GROUP };

/// A node the reloaded score needs, before it is reused or created.
struct NodePlan {
    slot: Slot,
    bus: i32,
    controls: Controls,
}

impl NodePlan {
    fn new(slot: Slot, bus: i32, controls: Controls) -> Self {
        Self { slot, bus, controls }
    }

    fn spec(&self, node: i32) -> NodeSpec<'_> {
        NodeSpec { def: self.slot.def, node, group: self.slot.group, controls: &self.controls }
    }
}

#[derive(Debug, Clone)]
struct GraphNode {
    node: i32,
    bus: i32,
    send: Option<String>,
    controls: Controls,
    used_until: Duration,
}

impl GraphNode {
    fn created(node: i32, plan: NodePlan) -> Self {
        Self {
            node,
            bus: plan.bus,
            send: None,
            controls: plan.controls,
            used_until: Duration::ZERO,
        }
    }

    fn update(&self) -> NodeUpdate {
        NodeUpdate { node: self.node, controls: self.controls.clone() }
    }
}

struct Retiring {
    node: i32,
    bus: i32,
    free_after: Duration,
}

impl From<GraphNode> for Retiring {
    fn from(node: GraphNode) -> Self {
        Self { node: node.node, bus: node.bus, free_after: node.used_until }
    }
}

/// Strips by track and returns by name.
#[derive(Default)]
struct Active {
    strips: NodeMap,
    returns: NodeMap,
}

impl Active {
    fn nodes(&self) -> impl Iterator<Item = &GraphNode> {
        self.strips.values().chain(self.returns.values())
    }

    fn is_empty(&self) -> bool {
        self.strips.is_empty() && self.returns.is_empty()
    }

    /// The input bus of the named return, or a silent bus when there is none.
    fn return_bus(&self, name: Option<&str>) -> i32 {
        let node = name.and_then(|name| self.returns.get(name));
        node.map_or(NULL_BUS, |node| node.bus)
    }
}

#[derive(Default)]
struct Draft {
    active: Active,
    updates: Vec<NodeUpdate>,
    created: Vec<i32>,
    buses: BTreeSet<i32>,
}

pub struct ChannelGraph<B: GraphBackend> {
    backend: B,
    started: bool,
    active: Active,
    retiring: Vec<Retiring>,
    updates: Vec<NodeUpdate>,
    next_node: i32,
}

impl<B: GraphBackend> ChannelGraph<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            started: false,
            active: Active::default(),
            retiring: Vec::new(),
            updates: Vec::new(),
            next_node: FIRST_NODE,
        }
    }

    /// Creates new nodes now (silent until routed); control changes wait for
    /// `take_updates`. A failure frees new nodes and keeps the active graph.
    pub fn prepare(&mut self, score: &Score) -> GraphResult<()> {
        if !score_needs_graph(score) && self.active.is_empty() {
            return Ok(());
        }
        self.ensure_started(score)?;
        Build::new(self).run(score)
    }

    fn ensure_started(&mut self, score: &Score) -> GraphResult<()> {
        if !self.started && score_needs_graph(score) {
            self.backend.start()?;
            self.started = true;
        }
        Ok(())
    }

    /// Buses stay reserved while their nodes are active or retiring.
    fn new_draft(&self) -> Draft {
        let mut buses: BTreeSet<i32> = self.active.nodes().map(|n| n.bus).collect();
        buses.extend(self.retiring.iter().map(|r| r.bus));
        Draft { buses, ..Draft::default() }
    }

    fn commit(&mut self, draft: Draft) {
        let old = std::mem::replace(&mut self.active, draft.active);
        let retired_strips = retire_missing(old.strips, &self.active.strips);
        self.retiring.extend(retired_strips);
        let retired_returns = retire_missing(old.returns, &self.active.returns);
        self.retiring.extend(retired_returns);
        self.updates.extend(draft.updates);
    }

    fn rollback(&self, draft: &Draft, error: String) -> String {
        for node in &draft.created {
            let _ = self.backend.free_node(*node);
        }
        error
    }

    pub fn route_bus(&self, track: &str) -> GraphResult<i32> {
        self.active
            .strips
            .get(track)
            .map(|node| node.bus)
            .ok_or_else(|| format!("track {track} has no prepared channel strip"))
    }

    /// Records the end of a source routed to `track`, extending its return's tail.
    pub fn mark_used(&mut self, track: &str, end: Duration) {
        let Some(strip) = self.active.strips.get_mut(track) else {
            return;
        };
        strip.used_until = strip.used_until.max(end);
        let send = strip.send.clone();
        if let Some(ret) = send.and_then(|name| self.active.returns.get_mut(&name)) {
            ret.used_until = ret.used_until.max(end + REVERB_TAIL);
        }
    }

    pub fn take_updates(&mut self) -> Vec<NodeUpdate> {
        std::mem::take(&mut self.updates)
    }

    pub fn prune(&mut self, now: Duration) -> GraphResult<()> {
        let (done, keep) =
            std::mem::take(&mut self.retiring).into_iter().partition(|r| r.free_after <= now);
        self.retiring = keep;
        done.iter().try_for_each(|r: &Retiring| self.backend.free_node(r.node))
    }

    /// Frees every node after the caller has waited for `tail`.
    pub fn finish(&mut self) -> GraphResult<()> {
        let nodes = self.all_nodes();
        nodes.iter().try_for_each(|node| self.backend.free_node(*node))?;
        self.active = Active::default();
        self.retiring.clear();
        self.stop_if_started()
    }

    fn all_nodes(&self) -> Vec<i32> {
        let retiring = self.retiring.iter().map(|r| r.node);
        self.active.nodes().map(|n| n.node).chain(retiring).collect()
    }

    fn stop_if_started(&mut self) -> GraphResult<()> {
        if std::mem::take(&mut self.started) { self.backend.stop() } else { Ok(()) }
    }

    /// Extra wait after the last source so return effects can decay.
    pub fn tail(&self) -> Duration {
        if self.active.returns.is_empty() { Duration::ZERO } else { REVERB_TAIL }
    }

    pub fn strip_controls(&self, track: &str) -> Option<NodeUpdate> {
        self.active.strips.get(track).map(GraphNode::update)
    }

    pub fn return_controls(&self, name: &str) -> Option<NodeUpdate> {
        self.active.returns.get(name).map(GraphNode::update)
    }
}

/// One reload's graph changes: committed together, or freed on failure.
struct Build<'g, B: GraphBackend> {
    graph: &'g mut ChannelGraph<B>,
    draft: Draft,
}

impl<'g, B: GraphBackend> Build<'g, B> {
    fn new(graph: &'g mut ChannelGraph<B>) -> Self {
        let draft = graph.new_draft();
        Self { graph, draft }
    }

    fn run(mut self, score: &Score) -> GraphResult<()> {
        match self.add_all(score) {
            Ok(()) => {
                self.graph.commit(self.draft);
                Ok(())
            }
            Err(error) => Err(self.graph.rollback(&self.draft, error)),
        }
    }

    /// Returns come first so strips can find their Aux return buses.
    fn add_all(&mut self, score: &Score) -> GraphResult<()> {
        for (name, effect) in wanted_returns(score) {
            self.add_return(name, effect)?;
        }
        for (track, channel) in wanted_strips(score) {
            self.add_strip(track, channel)?;
        }
        Ok(())
    }

    fn add_return(&mut self, name: &str, effect: &AuxReturn) -> GraphResult<()> {
        let existing = self.graph.active.returns.get(name).cloned();
        let bus = self.bus_for(existing.as_ref())?;
        let plan = NodePlan::new(RETURN_SLOT, bus, return_controls(effect, bus));
        let node = self.place(existing, plan)?;
        self.draft.active.returns.insert(name.to_owned(), node);
        Ok(())
    }

    fn add_strip(&mut self, track: &str, channel: &ChannelStrip) -> GraphResult<()> {
        let existing = self.graph.active.strips.get(track).cloned();
        let bus = self.bus_for(existing.as_ref())?;
        let send = channel.aux_send.as_ref().map(|s| s.return_name.clone());
        let aux_bus = self.draft.active.return_bus(send.as_deref());
        let plan = NodePlan::new(STRIP_SLOT, bus, strip_controls(channel, bus, aux_bus));
        let node = self.place(existing, plan)?;
        self.draft.active.strips.insert(track.to_owned(), GraphNode { send, ..node });
        Ok(())
    }

    fn bus_for(&mut self, existing: Option<&GraphNode>) -> GraphResult<i32> {
        existing.map_or_else(|| allocate_bus(&mut self.draft.buses), |node| Ok(node.bus))
    }

    /// Reuses an existing node (staging changed controls) or creates a new one.
    fn place(&mut self, existing: Option<GraphNode>, plan: NodePlan) -> GraphResult<GraphNode> {
        match existing {
            Some(node) => Ok(restage(node, plan.controls, &mut self.draft)),
            None => self.create(plan),
        }
    }

    fn create(&mut self, plan: NodePlan) -> GraphResult<GraphNode> {
        let node = self.graph.next_node;
        self.graph.next_node += 1;
        self.graph.backend.add_node(&plan.spec(node))?;
        self.draft.created.push(node);
        Ok(GraphNode::created(node, plan))
    }
}

fn restage(mut node: GraphNode, controls: Controls, draft: &mut Draft) -> GraphNode {
    if node.controls != controls {
        node.controls = controls;
        draft.updates.push(node.update());
    }
    node
}

fn retire_missing(old: NodeMap, active: &NodeMap) -> impl Iterator<Item = Retiring> + '_ {
    old.into_iter()
        .filter(|(name, _)| !active.contains_key(name))
        .map(|(_, node)| Retiring::from(node))
}

fn allocate_bus(buses: &mut BTreeSet<i32>) -> GraphResult<i32> {
    let bus = (FIRST_BUS..=LAST_BUS)
        .step_by(2)
        .find(|bus| !buses.contains(bus))
        .ok_or("channel bus capacity exceeded")?;
    buses.insert(bus);
    Ok(bus)
}

fn score_needs_graph(score: &Score) -> bool {
    wanted_strips(score).next().is_some()
}

fn wanted_strips(score: &Score) -> impl Iterator<Item = (&str, &ChannelStrip)> {
    score
        .channels
        .iter()
        .filter(|(_, channel)| channel.needs_graph())
        .map(|(track, channel)| (track.as_str(), channel))
}

/// Only returns fed by an active strip are instantiated; muting may drop one.
fn wanted_returns(score: &Score) -> impl Iterator<Item = (&str, &AuxReturn)> {
    let used = used_return_names(score);
    score
        .aux_returns
        .iter()
        .filter(move |(name, _)| used.contains(name.as_str()))
        .map(|(name, effect)| (name.as_str(), effect))
}

fn used_return_names(score: &Score) -> BTreeSet<&str> {
    wanted_strips(score)
        .filter_map(|(_, channel)| channel.aux_send.as_ref())
        .map(|send| send.return_name.as_str())
        .collect()
}

pub fn db_to_amp(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

/// Controls in signal order: routing, H/A, Dynamics, EQ, then Aux, Pan and Fader.
fn strip_controls(channel: &ChannelStrip, bus: i32, aux_bus: i32) -> Controls {
    let mut controls = routing_controls(bus, aux_bus);
    controls.push(("ha", db_to_amp(channel.ha_gain_db)));
    controls.extend(compressor_controls(channel));
    controls.extend(bell_controls(channel));
    controls.extend(output_controls(channel));
    controls
}

fn routing_controls(bus: i32, aux_bus: i32) -> Controls {
    vec![("inBus", bus as f32), ("auxBus", aux_bus as f32), ("out", 0.0)]
}

fn output_controls(channel: &ChannelStrip) -> Controls {
    let send = channel.aux_send.as_ref();
    vec![
        ("sendLevel", send.map_or(0.0, |s| db_to_amp(s.level_db))),
        ("pan", channel.pan),
        ("fader", db_to_amp(channel.fader_db)),
    ]
}

/// Threshold (linear), ratio, attack (ms) and release (ms).
type CompressorSettings = (f32, f32, f32, f32);

/// A bypassed compressor is ratio 1, which `Compander` passes unchanged.
const BYPASSED_COMPRESSOR: CompressorSettings = (1.0, 1.0, 10.0, 100.0);

fn compressor_controls(channel: &ChannelStrip) -> Controls {
    let settings = channel.compressor.as_ref().map(compressor_settings);
    let (threshold, ratio, attack_ms, release_ms) = settings.unwrap_or(BYPASSED_COMPRESSOR);
    vec![
        ("threshold", threshold),
        ("ratio", ratio),
        ("attack", attack_ms / 1000.0),
        ("release", release_ms / 1000.0),
    ]
}

fn compressor_settings(compressor: &Compressor) -> CompressorSettings {
    let threshold = db_to_amp(compressor.threshold_db);
    (threshold, compressor.ratio, compressor.attack_ms, compressor.release_ms)
}

/// A bypassed bell is 0 dB, which `BPeakEQ` passes unchanged.
fn bell_controls(channel: &ChannelStrip) -> Controls {
    let (frequency, q, gain) =
        channel.eq_bell.as_ref().map_or((1000.0, 1.0, 0.0), |b| (b.frequency_hz, b.q, b.gain_db));
    vec![("eqFreq", frequency), ("eqQ", q), ("eqDb", gain)]
}

fn return_controls(effect: &AuxReturn, bus: i32) -> Controls {
    vec![
        ("inBus", bus as f32),
        ("out", 0.0),
        ("room", effect.room),
        ("damp", effect.damp),
        ("wet", effect.wet),
        ("fader", db_to_amp(effect.fader_db)),
    ]
}
