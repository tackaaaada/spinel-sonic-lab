use std::cell::RefCell;
use std::time::Duration;

use spinel_sonic_lab::mixer::{
    ChannelGraph, GraphBackend, NodeRelease, NodeSpec, NodeUpdate, RETURN_GROUP, REVERB_TAIL,
    STRIP_GROUP,
};
use spinel_sonic_lab::model::Score;
use spinel_sonic_lab::osc::{Lookups, RouteResolver, encode_routed_playback_bundle};
use spinel_sonic_lab::schedule::{PlaybackEvent, PlaybackSound, Scheduler};

const CONSOLE: &str = concat!(
    "{\"type\":\"tempo\",\"bpm\":120,\"bars\":1}\n",
    "{\"type\":\"aux_return\",\"name\":\"room\",\"reverb\":{\"room\":0.5,\"damp\":0.5,\"wet\":0.5},\"fader_db\":-6}\n",
    "{\"type\":\"channel\",\"track\":\"lowtrack\",\"ha_gain_db\":6,\"pan\":-0.5,\"fader_db\":-12,",
    "\"compressor\":{\"threshold_db\":-20,\"ratio\":4,\"attack_ms\":10,\"release_ms\":100},",
    "\"eq_bell\":{\"frequency_hz\":200,\"gain_db\":-3,\"q\":2},",
    "\"aux_send\":{\"return\":\"room\",\"level_db\":-20}}\n",
    "{\"type\":\"channel\",\"track\":\"lead\",\"ha_gain_db\":0,\"pan\":1,\"fader_db\":-6}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"lowtrack\",\"note\":36,\"beat\":0,\"duration_beats\":1,\"velocity\":0.5}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"lead\",\"note\":60,\"beat\":0,\"duration_beats\":1,\"velocity\":0.5}\n",
);
const COMPRESSOR: &str =
    ",\"compressor\":{\"threshold_db\":-20,\"ratio\":4,\"attack_ms\":10,\"release_ms\":100}";
const EQ_BELL: &str = "\"eq_bell\":{\"frequency_hz\":200,\"gain_db\":-3,\"q\":2},";

#[derive(Default)]
struct FakeBackend {
    log: RefCell<Vec<String>>,
    fail_on: Option<&'static str>,
}

impl FakeBackend {
    fn record(&self, entry: String) -> Result<(), String> {
        self.log.borrow_mut().push(entry);
        Ok(())
    }

    fn log(&self) -> Vec<String> {
        self.log.borrow().clone()
    }

    fn entries(&self) -> usize {
        self.log.borrow().len()
    }
}

impl GraphBackend for &FakeBackend {
    fn start(&self) -> Result<(), String> {
        self.record("start".to_owned())
    }
    fn add_node(&self, spec: &NodeSpec<'_>) -> Result<(), String> {
        if self.fail_on == Some(spec.def) {
            return Err(format!("{} failed", spec.def));
        }
        let controls: Vec<String> = spec.controls.iter().map(|(k, v)| format!("{k}={v}")).collect();
        let (def, node, group) = (spec.def, spec.node, spec.group);
        self.record(format!("add {def} {node} in {group} {}", controls.join(",")))
    }
    fn free_node(&self, node: i32) -> Result<(), String> {
        self.record(format!("free {node}"))
    }
}

fn console() -> Score {
    Score::parse_ndjson(CONSOLE).unwrap()
}

fn console_plan() -> Vec<PlaybackEvent> {
    Scheduler::plan_playback(&console()).unwrap()
}

fn prepared<'b>(backend: &'b FakeBackend, source: &str) -> ChannelGraph<&'b FakeBackend> {
    let mut graph = ChannelGraph::new(backend);
    graph.prepare(&Score::parse_ndjson(source).unwrap()).unwrap();
    graph
}

fn control(update: &NodeUpdate, name: &str) -> f32 {
    update.controls.iter().find(|(k, _)| *k == name).unwrap().1
}

fn assert_close(actual: f32, expected: f32, tolerance: f32) {
    assert!((actual - expected).abs() < tolerance, "{actual} != {expected}");
}

fn routed_bundle(events: &[PlaybackEvent], route: &RouteResolver<'_>) -> Vec<u8> {
    let lookups = Lookups { sample: &|_| Ok(1), route };
    encode_routed_playback_bundle(9, events, &lookups).unwrap()
}

fn contains(packet: &[u8], bytes: &[u8]) -> bool {
    packet.windows(bytes.len()).any(|window| window == bytes)
}

#[test]
fn processed_tracks_route_to_strip_at_unity_and_center() {
    let plan = console_plan();
    let bass = plan.iter().find(|e| e.route.as_deref() == Some("lowtrack")).unwrap();
    // The strip applies H/A, Pan, and Fader, so the source stays at unity and center.
    assert_close(bass.amplitude, 0.1, 1e-6);
    assert_eq!(bass.pan, None);
    let PlaybackSound::Synth(note) = &bass.sound else { panic!() };
    assert_eq!(note.pan, None);
    assert_close(note.amplitude, 0.1, 1e-6);
}

#[test]
fn simple_channels_keep_event_levels() {
    let plan = console_plan();
    let lead = plan.iter().find(|e| e.route.is_none()).unwrap();
    assert_eq!(lead.pan, Some(1.0));
    assert_close(lead.amplitude, 0.1 * 0.501_187, 1e-5);
}

#[test]
fn routed_bundle_writes_bus_but_no_track_name() {
    let packet = routed_bundle(&console_plan(), &|track| {
        assert_eq!(track, "lowtrack");
        Ok(130)
    });
    assert!(contains(&packet, b"out\0"));
    assert!(contains(&packet, &130_i32.to_be_bytes()));
    assert!(!contains(&packet, b"lowtrack"));
    assert!(!contains(&packet, b"room"));
}

#[test]
fn unrouted_encoder_refuses_to_drop_a_strip_route() {
    let plan = console_plan();
    assert!(spinel_sonic_lab::osc::encode_playback_bundle(9, &plan, |_| Ok(1)).is_err());
}

#[test]
fn strip_buses_connect_the_route_and_the_aux_return() {
    let backend = FakeBackend::default();
    let graph = prepared(&backend, CONSOLE);
    let strip = graph.strip_controls("lowtrack").unwrap();
    let ret = graph.return_controls("room").unwrap();
    assert_eq!(control(&strip, "auxBus"), control(&ret, "inBus"));
    let route = graph.route_bus("lowtrack").unwrap();
    assert_eq!(route as f32, control(&strip, "inBus"));
}

#[test]
fn bypassed_modules_are_identity_controls() {
    let source = CONSOLE.replace(COMPRESSOR, "").replace(EQ_BELL, "");
    let backend = FakeBackend::default();
    let strip = prepared(&backend, &source).strip_controls("lowtrack").unwrap();
    assert_eq!(control(&strip, "ratio"), 1.0);
    assert_eq!(control(&strip, "eqDb"), 0.0);
}

#[test]
fn scores_without_processing_never_start_the_graph() {
    let backend = FakeBackend::default();
    let simple = include_str!("../../composer/examples/console_grid.ndjson");
    let graph = prepared(&backend, simple);
    assert!(backend.log().is_empty());
    assert_eq!(graph.tail(), Duration::ZERO);
}

#[test]
fn failed_preparation_keeps_the_active_graph() {
    let fail_on = Some("channel_strip");
    let backend = FakeBackend { fail_on, ..FakeBackend::default() };
    let mut graph = ChannelGraph::new(&backend);
    assert!(graph.prepare(&console()).is_err());
    assert!(graph.route_bus("lowtrack").is_err());
    // The return created before the failure is released immediately.
    let log = backend.log();
    assert!(log.last().unwrap().starts_with("free "), "{log:?}");
}

/// Freeing a group frees its nodes; the return's reverb tail is the last use.
#[test]
fn releasing_all_frees_both_groups_after_the_last_use() {
    let backend = FakeBackend::default();
    let mut graph = prepared(&backend, CONSOLE);
    graph.mark_used("lowtrack", Duration::from_secs(2));
    let after = Duration::from_secs(2) + REVERB_TAIL;
    let groups = [STRIP_GROUP, RETURN_GROUP].map(|node| NodeRelease { node, after });
    assert_eq!(graph.release_all(), groups);
    assert!(graph.release_all().is_empty(), "the graph starts again on the next prepare");
    assert!(!backend.log().iter().any(|l| l.starts_with("free")), "freed by the engine");
}

fn console_strip_score() -> Score {
    let source = include_str!("../../composer/examples/console_strip.ndjson");
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../composer/examples");
    spinel_sonic_lab::source::prepare_score(source, &base, None).unwrap()
}

#[test]
fn console_strip_example_routes_grid_hits_through_strips() {
    let plan = Scheduler::plan_playback(&console_strip_score()).unwrap();
    let first: Vec<_> = plan.iter().take_while(|e| e.at == Duration::ZERO).collect();
    let routes: Vec<_> = first.iter().map(|e| e.route.as_deref().unwrap()).collect();
    assert_eq!(routes, ["kick", "bass"], "kick and bass share one bundle at the downbeat");
    assert!(plan.iter().all(|e| e.route.is_some() && e.pan.is_none()));
    let packet = routed_bundle(&plan[..2], &|t| Ok(if t == "kick" { 128 } else { 132 }));
    assert_eq!(packet.windows(4).filter(|x| *x == b"out\0").count(), 2);
}

#[test]
fn console_strip_mute_keeps_a_shared_return_until_no_strip_sends() {
    let without_bass = console_strip_score().without_track("bass").unwrap();
    let backend = FakeBackend::default();
    let mut graph = ChannelGraph::new(&backend);
    graph.prepare(&without_bass).unwrap();
    assert!(graph.return_controls("room").is_some(), "hat still sends to room");
    graph.prepare(&without_bass.without_track("hat").unwrap()).unwrap();
    assert!(graph.return_controls("room").is_none());
    assert_eq!(graph.tail(), Duration::ZERO);
}

/// Controls of the console's lowtrack strip and room return after one prepare.
fn console_controls() -> (NodeUpdate, NodeUpdate) {
    let backend = FakeBackend::default();
    let graph = prepared(&backend, CONSOLE);
    (graph.strip_controls("lowtrack").unwrap(), graph.return_controls("room").unwrap())
}

#[test]
fn graph_creates_return_before_strip() {
    let backend = FakeBackend::default();
    prepared(&backend, CONSOLE);
    let log = backend.log();
    assert_eq!(log[0], "start");
    assert!(log[1].starts_with("add aux_return "), "{log:?}");
    assert!(log[2].starts_with("add channel_strip "), "{log:?}");
}

#[test]
fn simple_channels_get_no_strip_and_returns_add_a_tail() {
    let backend = FakeBackend::default();
    let graph = prepared(&backend, CONSOLE);
    assert_eq!(backend.entries(), 3, "simple channel must not get a strip: {:?}", backend.log());
    assert!(graph.route_bus("lead").is_err());
    assert_eq!(graph.tail(), REVERB_TAIL);
}

#[test]
fn strip_controls_follow_ha_and_dynamics() {
    let (strip, _) = console_controls();
    assert_close(control(&strip, "ha"), 1.995_262, 1e-5);
    assert_close(control(&strip, "threshold"), 0.1, 1e-6);
    assert_eq!(control(&strip, "ratio"), 4.0);
    assert_close(control(&strip, "attack"), 0.01, 1e-6);
    assert_close(control(&strip, "release"), 0.1, 1e-6);
}

#[test]
fn strip_controls_follow_eq_send_pan_and_fader() {
    let (strip, _) = console_controls();
    let eq = ["eqFreq", "eqQ", "eqDb"].map(|name| control(&strip, name));
    assert_eq!(eq, [200.0, 2.0, -3.0]);
    assert_close(control(&strip, "sendLevel"), 0.1, 1e-6);
    assert_eq!(control(&strip, "pan"), -0.5);
    assert_close(control(&strip, "fader"), 0.251_189, 1e-5);
}

#[test]
fn return_controls_follow_the_aux_return() {
    let (_, ret) = console_controls();
    assert_close(control(&ret, "fader"), 0.501_187, 1e-5);
    assert_eq!(control(&ret, "wet"), 0.5);
}

#[test]
fn first_prepare_stages_no_control_updates() {
    let backend = FakeBackend::default();
    assert!(prepared(&backend, CONSOLE).take_updates().is_empty());
}

#[test]
fn reload_reuses_nodes_and_stages_control_changes_for_the_bar_boundary() {
    let backend = FakeBackend::default();
    let mut graph = prepared(&backend, CONSOLE);
    let louder = CONSOLE.replace("\"fader_db\":-12", "\"fader_db\":0");
    graph.prepare(&Score::parse_ndjson(&louder).unwrap()).unwrap();
    assert_eq!(backend.entries(), 3, "existing nodes must be reused");
    let updates = graph.take_updates();
    assert_eq!(updates.iter().map(|u| control(u, "fader")).collect::<Vec<_>>(), [1.0]);
    assert!(graph.take_updates().is_empty());
}

type Graph<'b> = ChannelGraph<&'b FakeBackend>;

/// Release times of the nodes finished by `seconds`.
fn released_at(graph: &mut Graph<'_>, seconds: Duration) -> Vec<Duration> {
    graph.release_finished(seconds).iter().map(|release| release.after).collect()
}

/// The console after `lowtrack`, last used until 2 s, is removed by a reload.
fn without_lowtrack(backend: &FakeBackend) -> Graph<'_> {
    let mut graph = prepared(backend, CONSOLE);
    graph.mark_used("lowtrack", Duration::from_secs(2));
    graph.prepare(&console().without_track("lowtrack").unwrap()).unwrap();
    assert!(graph.route_bus("lowtrack").is_err());
    graph
}

#[test]
fn removed_strip_and_return_retire_after_their_tails() {
    let backend = FakeBackend::default();
    let mut graph = without_lowtrack(&backend);
    // The strip waits for its source; the return also keeps the reverb tail.
    let (two, entries) = (Duration::from_secs(2), backend.entries());
    assert!(released_at(&mut graph, Duration::from_secs(1)).is_empty());
    assert_eq!(released_at(&mut graph, two), [two]);
    assert_eq!(released_at(&mut graph, two + REVERB_TAIL), [two + REVERB_TAIL]);
    assert_eq!(backend.entries(), entries, "the engine schedules the frees");
}

#[test]
fn a_released_strip_bus_is_not_reused_by_the_next_new_strip() {
    let backend = FakeBackend::default();
    let old_bus = prepared(&backend, CONSOLE).route_bus("lowtrack").unwrap();
    let mut graph = without_lowtrack(&backend);
    let _ = graph.release_finished(Duration::from_secs(60));
    let renamed = CONSOLE.replace("lowtrack", "newtrack");
    graph.prepare(&Score::parse_ndjson(&renamed).unwrap()).unwrap();
    // A lagging server may still run the old strip on that bus.
    assert_ne!(graph.route_bus("newtrack").unwrap(), old_bus);
}
