use std::io::ErrorKind;
use std::net::{ToSocketAddrs, UdpSocket};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::mixer::{GraphBackend, NodeSpec, NodeUpdate, RETURN_GROUP, STRIP_GROUP};
use crate::model::Oscillator;
use crate::sample::SampleAsset;
use crate::schedule::{PlaybackEvent, PlaybackSound, ScheduledNote};

pub type Resolver<'a> = dyn Fn(&SampleAsset) -> Result<i32, String> + 'a;
pub type RouteResolver<'a> = dyn Fn(&str) -> Result<i32, String> + 'a;

/// Engine lookups for one bundle: sample buffer IDs and strip input buses.
pub struct Lookups<'a> {
    pub sample: &'a Resolver<'a>,
    pub route: &'a RouteResolver<'a>,
}

impl Lookups<'_> {
    fn out_bus(&self, event: &PlaybackEvent) -> EncodeResult<Option<i32>> {
        event.route.as_deref().map(self.route).transpose()
    }
}

fn unrouted(track: &str) -> EncodeResult<i32> {
    Err(format!("track {track} needs a channel strip bus"))
}
type EncodeResult<T> = Result<T, String>;

pub const OSC_IMMEDIATE_TIMETAG: u64 = 1;
const NTP_UNIX_OFFSET_SECONDS: u64 = 2_208_988_800;

pub struct OscClient {
    socket: UdpSocket,
}

impl OscClient {
    pub fn connect(host: &str, port: u16) -> Result<Self, String> {
        let target = resolve_target(host, port)?;
        let socket = connect_socket(target)?;
        Ok(Self { socket })
    }

    pub fn load_synthdef(&self, path: &Path) -> Result<(), String> {
        let path =
            path.canonicalize().map_err(|error| format!("SynthDef file unavailable: {error}"))?;
        let path = path.to_str().ok_or("SynthDef path must be UTF-8")?;
        self.send(&encode_load(path))?;
        self.wait_for_synthdef()
    }

    fn wait_for_synthdef(&self) -> Result<(), String> {
        loop {
            match self.recv_address("SynthDef load")?.as_deref() {
                Some("/done") => return Ok(()),
                Some("/fail") => return Err("scsynth rejected SynthDef load".to_owned()),
                _ => continue,
            }
        }
    }

    fn recv_address(&self, label: &str) -> Result<Option<String>, String> {
        let mut buffer = [0_u8; 2048];
        let size = self
            .socket
            .recv(&mut buffer)
            .map_err(|error| format!("{label} response failed: {error}"))?;
        Ok(osc_address(&buffer[..size]).map(str::to_owned))
    }

    pub fn play_note(&self, note: &ScheduledNote) -> Result<(), String> {
        self.send(&encode_s_new(note))
    }

    pub fn play_notes_bundle(&self, timetag: u64, notes: &[ScheduledNote]) -> Result<(), String> {
        if notes.is_empty() {
            return Ok(());
        }
        self.send(&encode_bundle(timetag, notes))
    }

    pub fn verify_commands(&self) -> Result<(), String> {
        self.send(&encode_sync(1))?;
        self.wait_for_sync()
    }

    fn wait_for_sync(&self) -> Result<(), String> {
        loop {
            match self.recv_address("scsynth command")?.as_deref() {
                Some("/fail") => return Err("scsynth rejected an OSC command".to_owned()),
                Some("/synced") => return Ok(()),
                _ => continue,
            }
        }
    }

    fn send(&self, bytes: &[u8]) -> Result<(), String> {
        self.socket.send(bytes).map_err(|error| format!("OSC send failed: {error}"))?;
        Ok(())
    }
}

fn connect_socket(target: std::net::SocketAddr) -> Result<UdpSocket, String> {
    let bind = if target.is_ipv4() { "127.0.0.1:0" } else { "[::1]:0" };
    let socket = UdpSocket::bind(bind).map_err(|error| format!("OSC bind failed: {error}"))?;
    configure_socket(&socket, target)?;
    Ok(socket)
}

fn configure_socket(socket: &UdpSocket, target: std::net::SocketAddr) -> Result<(), String> {
    socket.connect(target).map_err(|error| format!("OSC connect failed: {error}"))?;
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|error| format!("OSC timeout setup failed: {error}"))?;
    Ok(())
}

fn resolve_target(host: &str, port: u16) -> Result<std::net::SocketAddr, String> {
    let target = (host, port)
        .to_socket_addrs()
        .map_err(|error| format!("invalid OSC address: {error}"))?
        .next()
        .ok_or("OSC address did not resolve")?;
    check_loopback(target)?;
    Ok(target)
}

fn check_loopback(target: std::net::SocketAddr) -> Result<(), String> {
    if !target.ip().is_loopback() {
        return Err("MVP only supports a local OSC server".to_owned());
    }
    Ok(())
}

fn osc_address(bytes: &[u8]) -> Option<&str> {
    let end = bytes.iter().position(|byte| *byte == 0)?;
    std::str::from_utf8(&bytes[..end]).ok()
}

pub fn system_time_to_ntp(time: SystemTime) -> u64 {
    let since_unix = time.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
    let seconds = since_unix.as_secs().saturating_add(NTP_UNIX_OFFSET_SECONDS);
    let nanos = u64::from(since_unix.subsec_nanos());
    let fraction = ((nanos << 32) / 1_000_000_000) & 0xffff_ffff;
    (seconds << 32) | fraction
}

pub fn encode_bundle(timetag: u64, notes: &[ScheduledNote]) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + notes.len() * 228);
    osc_string(&mut out, "#bundle");
    out.extend_from_slice(&timetag.to_be_bytes());
    for note in notes {
        append_note(&mut out, note);
    }
    out
}

fn append_note(out: &mut Vec<u8>, note: &ScheduledNote) {
    let message = encode_s_new(note);
    osc_i32(out, message.len() as i32);
    out.extend_from_slice(&message);
}

pub fn encode_load(path: &str) -> Vec<u8> {
    let mut out = Vec::new();
    osc_string(&mut out, "/d_load");
    osc_string(&mut out, ",s");
    osc_string(&mut out, path);
    out
}

pub fn encode_s_new(note: &ScheduledNote) -> Vec<u8> {
    encode_routed_s_new(note, None)
}

fn encode_routed_s_new(note: &ScheduledNote, bus: Option<i32>) -> Vec<u8> {
    let mut out = Vec::with_capacity(232);
    write_note_header(&mut out, note, bus);
    write_note_controls(&mut out, note);
    write_pan(&mut out, note.pan);
    write_out_bus(&mut out, bus);
    out
}

fn write_note_controls(out: &mut Vec<u8>, note: &ScheduledNote) {
    write_note_levels(out, note);
    write_note_envelope(out, note);
    write_note_modulation(out, note);
    write_note_oscillator(out, note);
}

fn write_out_bus(out: &mut Vec<u8>, bus: Option<i32>) {
    if let Some(bus) = bus {
        osc_string(out, "out");
        osc_i32(out, bus);
    }
}

fn write_note_header(out: &mut Vec<u8>, note: &ScheduledNote, bus: Option<i32>) {
    osc_string(out, "/s_new");
    osc_string(out, &note_tags(note, bus));
    osc_string(out, "analog_bass");
    osc_i32(out, -1); // Auto-assigned ID; the envelope frees the node.
    osc_i32(out, 0); // Add at head.
    osc_i32(out, 0); // Root group exists in standalone scsynth.
}

fn note_tags(note: &ScheduledNote, bus: Option<i32>) -> String {
    let mut tags = String::from(",siiisfsfsfsfsfsfsfsfsfsfsfsi");
    if note.pan.is_some() {
        tags.push_str("sf");
    }
    if bus.is_some() {
        tags.push_str("si");
    }
    tags
}

fn write_note_levels(out: &mut Vec<u8>, note: &ScheduledNote) {
    osc_string(out, "freq");
    osc_f32(out, note.frequency);
    osc_string(out, "amp");
    osc_f32(out, note.amplitude);
    osc_string(out, "gain");
    osc_f32(out, note.tone.gain);
}

fn write_note_envelope(out: &mut Vec<u8>, note: &ScheduledNote) {
    osc_string(out, "dur");
    osc_f32(out, note.duration_seconds);
    osc_string(out, "cutoff");
    osc_f32(out, note.tone.cutoff_hz);
    write_adsr(out, &note.tone);
}

fn write_adsr(out: &mut Vec<u8>, tone: &crate::model::BassTone) {
    osc_string(out, "attack");
    osc_f32(out, tone.attack_seconds);
    osc_string(out, "decay");
    osc_f32(out, tone.decay_seconds);
    osc_string(out, "sustain");
    osc_f32(out, tone.sustain_level);
    osc_string(out, "release");
    osc_f32(out, tone.release_seconds);
}

fn write_note_modulation(out: &mut Vec<u8>, note: &ScheduledNote) {
    osc_string(out, "lfoRate");
    osc_f32(out, note.tone.lfo_rate_hz);
    osc_string(out, "lfoDepth");
    osc_f32(out, note.tone.lfo_depth);
}

fn write_note_oscillator(out: &mut Vec<u8>, note: &ScheduledNote) {
    osc_string(out, "osc");
    osc_i32(out, oscillator_id(note.tone.oscillator));
}

fn oscillator_id(oscillator: Oscillator) -> i32 {
    match oscillator {
        Oscillator::Saw => 0,
        Oscillator::Sine => 1,
        Oscillator::Pulse => 2,
        Oscillator::Noise => 3,
    }
}

fn write_pan(out: &mut Vec<u8>, pan: Option<f32>) {
    if let Some(pan) = pan {
        osc_string(out, "pan");
        osc_f32(out, pan);
    }
}

pub fn encode_sync(id: i32) -> Vec<u8> {
    let mut out = Vec::new();
    osc_string(&mut out, "/sync");
    osc_string(&mut out, ",i");
    osc_i32(&mut out, id);
    out
}

fn osc_string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(value.as_bytes());
    out.push(0);
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

fn osc_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn osc_f32(out: &mut Vec<u8>, value: f32) {
    out.extend_from_slice(&value.to_bits().to_be_bytes());
}

impl crate::sampler::SampleBackend for &OscClient {
    fn load(&self, id: i32, asset: &crate::sample::SampleAsset) -> Result<(), String> {
        crate::source::with_sample_snapshot(asset, |path| {
            let path = path.to_str().ok_or("sample path must be UTF-8")?;
            self.send(&encode_alloc_read(id, path))?;
            self.wait_done("/b_allocRead", Some(id))
        })
    }
    /// Frees a buffer that never played, before any note can use it.
    fn free(&self, id: i32) -> Result<(), String> {
        self.send(&encode_b_free(id))?;
        self.wait_done("/b_free", Some(id))
    }
}

fn encode_b_free(id: i32) -> Vec<u8> {
    let mut packet = Vec::new();
    osc_string(&mut packet, "/b_free");
    osc_string(&mut packet, ",i");
    osc_i32(&mut packet, id);
    packet
}

/// A buffer free stamped at `timetag`; scsynth runs it in timetag order with notes.
pub fn encode_buffer_release(timetag: u64, id: i32) -> Vec<u8> {
    encode_messages_bundle(timetag, &[encode_b_free(id)])
}

/// A node or group free stamped at `timetag`.
pub fn encode_node_release(timetag: u64, node: i32) -> Vec<u8> {
    encode_messages_bundle(timetag, &[encode_n_free(&[node])])
}

/// Every remaining free at `timetag`, then `/sync`: scsynth answers only after
/// running them on its own clock.
pub fn encode_final_release(timetag: u64, buffers: &[i32], nodes: &[i32]) -> Vec<u8> {
    let mut messages: Vec<Vec<u8>> = buffers.iter().map(|id| encode_b_free(*id)).collect();
    if !nodes.is_empty() {
        messages.push(encode_n_free(nodes));
    }
    messages.push(encode_sync(1));
    encode_messages_bundle(timetag, &messages)
}

pub fn encode_alloc_read(id: i32, path: &str) -> Vec<u8> {
    let mut packet = Vec::new();
    osc_string(&mut packet, "/b_allocRead");
    osc_string(&mut packet, ",isii");
    osc_i32(&mut packet, id);
    osc_string(&mut packet, path);
    osc_i32(&mut packet, 0);
    osc_i32(&mut packet, -1);
    packet
}

impl OscClient {
    fn wait_done(&self, command: &str, id: Option<i32>) -> Result<(), String> {
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let expected = expected_done(command, id);
        while std::time::Instant::now() < deadline {
            if let Some(result) = self.done_response(command, &expected)? {
                return result;
            }
        }
        Err(format!("{command} response timed out"))
    }

    fn done_response(
        &self,
        command: &str,
        expected: &[u8],
    ) -> Result<Option<Result<(), String>>, String> {
        let mut buffer = [0_u8; 2048];
        let n = self.recv_done_packet(command, &mut buffer)?;
        Ok(check_done_packet(&buffer[..n], expected, command))
    }

    fn recv_done_packet(&self, command: &str, buffer: &mut [u8]) -> Result<usize, String> {
        self.socket.recv(buffer).map_err(|e| format!("{command} response failed: {e}"))
    }

    /// Sends simultaneous events as one bundle; callers pass non-empty `chunk_by` groups.
    pub fn play_events_bundle(
        &self,
        timetag: u64,
        events: &[PlaybackEvent],
        lookups: &Lookups<'_>,
    ) -> EncodeResult<()> {
        self.send(&encode_routed_playback_bundle(timetag, events, lookups)?)
    }

    /// Schedules a buffer free without waiting: a lagging server replies only later.
    pub fn release_buffer(&self, timetag: u64, id: i32) -> EncodeResult<()> {
        self.send(&encode_buffer_release(timetag, id))
    }

    pub fn release_node(&self, timetag: u64, node: i32) -> EncodeResult<()> {
        self.send(&encode_node_release(timetag, node))
    }

    /// Sends the last frees and waits until the server has run them, so a later
    /// run cannot lose its new nodes or buffers to a free still pending here.
    pub fn release_and_confirm(
        &self,
        timetag: u64,
        buffers: &[i32],
        nodes: &[i32],
    ) -> EncodeResult<()> {
        self.send(&encode_final_release(timetag, buffers, nodes))?;
        self.wait_for_sync_within(FINAL_RELEASE_LIMIT)
    }

    /// Unlike `verify_commands`, read timeouts are retried until `limit`.
    fn wait_for_sync_within(&self, limit: Duration) -> EncodeResult<()> {
        let deadline = std::time::Instant::now() + limit;
        while std::time::Instant::now() < deadline {
            if let Some(result) = self.sync_reply()? {
                return result;
            }
        }
        Err("scsynth did not confirm the final release".to_owned())
    }

    /// The next `/synced` or `/fail`; `None` for other packets or a read timeout.
    fn sync_reply(&self) -> EncodeResult<Option<EncodeResult<()>>> {
        let mut buffer = [0_u8; 2048];
        match self.socket.recv(&mut buffer) {
            Ok(size) => Ok(sync_result(osc_address(&buffer[..size]))),
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => Ok(None),
            Err(e) => Err(format!("final release response failed: {e}")),
        }
    }

    /// Strip and return control changes, applied at `timetag` (a bar boundary).
    pub fn set_controls_bundle(&self, timetag: u64, updates: &[NodeUpdate]) -> EncodeResult<()> {
        if updates.is_empty() {
            return Ok(());
        }
        self.send(&encode_node_updates(timetag, updates))
    }
}

/// How long a lagging server may take to reach the final release.
const FINAL_RELEASE_LIMIT: Duration = Duration::from_secs(30);

fn sync_result(address: Option<&str>) -> Option<EncodeResult<()>> {
    match address {
        Some("/synced") => Some(Ok(())),
        Some("/fail") => Some(Err("scsynth rejected the final release".to_owned())),
        _ => None,
    }
}

fn expected_done(command: &str, id: Option<i32>) -> Vec<u8> {
    let mut expected = Vec::new();
    osc_string(&mut expected, "/done");
    osc_string(&mut expected, if id.is_some() { ",si" } else { ",s" });
    osc_string(&mut expected, command);
    if let Some(id) = id {
        osc_i32(&mut expected, id);
    }
    expected
}

fn check_done_packet(packet: &[u8], expected: &[u8], command: &str) -> Option<Result<(), String>> {
    if osc_address(packet) == Some("/fail") {
        return Some(Err(format!("scsynth rejected {command}")));
    }
    if packet == expected {
        return Some(Ok(()));
    }
    None
}

/// Encodes direct-output events; a strip-routed event is an error.
pub fn encode_playback_bundle(
    timetag: u64,
    events: &[PlaybackEvent],
    resolve: impl Fn(&SampleAsset) -> Result<i32, String>,
) -> Result<Vec<u8>, String> {
    let lookups = Lookups { sample: &resolve, route: &unrouted };
    encode_routed_playback_bundle(timetag, events, &lookups)
}

pub fn encode_routed_playback_bundle(
    timetag: u64,
    events: &[PlaybackEvent],
    lookups: &Lookups<'_>,
) -> Result<Vec<u8>, String> {
    let mut out = bundle_header(timetag);
    append_playback_events(&mut out, events, lookups)?;
    Ok(out)
}

pub fn encode_node_updates(timetag: u64, updates: &[NodeUpdate]) -> Vec<u8> {
    let mut out = bundle_header(timetag);
    for update in updates {
        let message = encode_n_set(update.node, &update.controls);
        osc_i32(&mut out, message.len() as i32);
        out.extend(message);
    }
    out
}

fn encode_n_set(node: i32, controls: &[(&'static str, f32)]) -> Vec<u8> {
    let mut out = Vec::new();
    osc_string(&mut out, "/n_set");
    osc_string(&mut out, &format!(",i{}", "sf".repeat(controls.len())));
    osc_i32(&mut out, node);
    write_controls(&mut out, controls);
    out
}

fn write_controls(out: &mut Vec<u8>, controls: &[(&'static str, f32)]) {
    for (name, value) in controls {
        osc_string(out, name);
        osc_f32(out, *value);
    }
}

fn bundle_header(timetag: u64) -> Vec<u8> {
    let mut out = Vec::new();
    osc_string(&mut out, "#bundle");
    out.extend_from_slice(&timetag.to_be_bytes());
    out
}

fn append_playback_events(
    out: &mut Vec<u8>,
    events: &[PlaybackEvent],
    lookups: &Lookups<'_>,
) -> EncodeResult<()> {
    for event in events {
        append_playback_event(out, event, lookups)?;
    }
    Ok(())
}

fn append_playback_event(
    out: &mut Vec<u8>,
    event: &PlaybackEvent,
    lookups: &Lookups<'_>,
) -> EncodeResult<()> {
    let packet = playback_packet(event, lookups)?;
    osc_i32(out, packet.len() as i32);
    out.extend(packet);
    Ok(())
}

fn playback_packet(event: &PlaybackEvent, lookups: &Lookups<'_>) -> EncodeResult<Vec<u8>> {
    let bus = lookups.out_bus(event)?;
    match &event.sound {
        PlaybackSound::Synth(note) => Ok(encode_routed_s_new(note, bus)),
        PlaybackSound::Sample { asset, .. } => {
            let buffer_id = (lookups.sample)(asset)?;
            Ok(encode_sample_note(event, asset, buffer_id, bus))
        }
    }
}

fn encode_sample_note(
    event: &PlaybackEvent,
    asset: &SampleAsset,
    buffer_id: i32,
    bus: Option<i32>,
) -> Vec<u8> {
    let mut packet = sample_header(asset, event.pan, bus);
    write_sample_body(&mut packet, event, buffer_id, bus);
    packet
}

fn write_sample_body(out: &mut Vec<u8>, event: &PlaybackEvent, buffer_id: i32, bus: Option<i32>) {
    osc_string(out, "buf");
    osc_i32(out, buffer_id);
    osc_string(out, "amp");
    osc_f32(out, event.amplitude);
    write_pan(out, event.pan);
    write_out_bus(out, bus);
}

fn sample_header(asset: &SampleAsset, pan: Option<f32>, bus: Option<i32>) -> Vec<u8> {
    let mut out = Vec::new();
    osc_string(&mut out, "/s_new");
    osc_string(&mut out, &sample_tags(pan, bus));
    osc_string(&mut out, sample_synthdef(asset));
    // Auto-assigned ID, added at the head of the root group.
    [-1, 0, 0].iter().for_each(|value| osc_i32(&mut out, *value));
    out
}

fn sample_tags(pan: Option<f32>, bus: Option<i32>) -> String {
    let pan_tags = if pan.is_some() { "sf" } else { "" };
    let bus_tags = if bus.is_some() { "si" } else { "" };
    format!(",siiisisf{pan_tags}{bus_tags}")
}

/// Sample SynthDefs, loaded once a score uses samples: mono first, then stereo.
pub const SAMPLE_SYNTHDEFS: [&str; 2] = ["sample_mono", "sample_stereo"];

fn sample_synthdef(asset: &SampleAsset) -> &'static str {
    SAMPLE_SYNTHDEFS[usize::from(asset.channels != 1)]
}

/// Engine side of the channel graph: definitions, groups, and nodes.
pub struct GraphClient<'a> {
    pub osc: &'a OscClient,
    pub synthdef_dir: &'a Path,
}

impl GraphBackend for GraphClient<'_> {
    fn start(&self) -> EncodeResult<()> {
        self.osc.load_synthdef(&self.synthdef_dir.join("channel_strip.scsyndef"))?;
        self.osc.load_synthdef(&self.synthdef_dir.join("aux_return.scsyndef"))?;
        // Silently drop groups left by an interrupted run, then rebuild them.
        self.osc.send(&encode_quiet_group_free())?;
        self.osc.send(&encode_graph_groups())?;
        self.osc.verify_commands()
    }

    fn add_node(&self, spec: &NodeSpec<'_>) -> EncodeResult<()> {
        self.osc.send(&encode_graph_node(spec))?;
        self.osc.verify_commands()
    }

    fn free_node(&self, node: i32) -> EncodeResult<()> {
        self.osc.send(&encode_n_free(&[node]))
    }
}

fn encode_n_free(nodes: &[i32]) -> Vec<u8> {
    let mut out = Vec::new();
    osc_string(&mut out, "/n_free");
    osc_string(&mut out, &(String::from(",") + &"i".repeat(nodes.len())));
    nodes.iter().for_each(|node| osc_i32(&mut out, *node));
    out
}

fn encode_quiet_group_free() -> Vec<u8> {
    let mut quiet = Vec::new();
    osc_string(&mut quiet, "/error");
    osc_string(&mut quiet, ",i");
    osc_i32(&mut quiet, -1); // Bundle-local: a missing group is not a failure.
    encode_messages_bundle(
        OSC_IMMEDIATE_TIMETAG,
        &[quiet, encode_n_free(&[STRIP_GROUP, RETURN_GROUP])],
    )
}

/// Sources are added at the head of the root group; strips and then returns
/// run after them at its tail.
pub fn encode_graph_groups() -> Vec<u8> {
    let mut out = Vec::new();
    osc_string(&mut out, "/g_new");
    osc_string(&mut out, ",iiiiii");
    for value in [STRIP_GROUP, 1, 0, RETURN_GROUP, 3, STRIP_GROUP] {
        osc_i32(&mut out, value);
    }
    out
}

pub fn encode_graph_node(spec: &NodeSpec<'_>) -> Vec<u8> {
    let mut out = Vec::new();
    osc_string(&mut out, "/s_new");
    osc_string(&mut out, &format!(",siii{}", "sf".repeat(spec.controls.len())));
    osc_string(&mut out, spec.def);
    let add_to_tail = 1;
    [spec.node, add_to_tail, spec.group].iter().for_each(|value| osc_i32(&mut out, *value));
    write_controls(&mut out, spec.controls);
    out
}

pub fn encode_messages_bundle(timetag: u64, messages: &[Vec<u8>]) -> Vec<u8> {
    let mut out = bundle_header(timetag);
    for message in messages {
        osc_i32(&mut out, message.len() as i32);
        out.extend_from_slice(message);
    }
    out
}
