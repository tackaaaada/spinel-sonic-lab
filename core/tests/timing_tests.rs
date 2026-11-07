use spinel_sonic_lab::osc::system_time_to_ntp;
use std::fs;
use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime};

const NTP_SECOND: u64 = 1 << 32;
const SYNCED: &[u8] = b"/synced\0,i\0\0\0\0\0\x01";
const SYNTHDEF_DONE: &[u8] = b"/done\0\0\0,s\0\0/d_load\0";
const BASS_SYNTHDEF: &str = "../synthdefs/analog_bass.scsyndef";

#[test]
#[ignore = "requires loopback UDP permission; run explicitly on the audio host"]
fn cli_sends_future_timestamps_with_even_spacing_and_waits_for_last_note() {
    check_bass_timing(&[], &[0, 250, 500, 750], 1);
    let watched = [0, 250, 500, 750, 2000, 2250, 2500, 2750];
    check_bass_timing(&["--watch", "--bars", "2"], &watched, 2);
}

#[test]
#[ignore = "requires loopback UDP permission; run explicitly on the audio host"]
fn cli_preloads_samples_then_frees_buffers_after_the_full_file_finishes() {
    let dir = TempDir::create("spinel-udp-sample");
    let wav = kick_wav();
    let score = write_sample_score(&dir.0, &wav);
    let server = FakeServer::bind();
    let child = spawn_sample_score(&server, &dir.0, &score);
    assert!(SampleExchange::new(server, wav).run_until_freed());
    assert_success(child);
}

/// A loopback stand-in for scsynth that records and answers OSC packets.
struct FakeServer {
    socket: UdpSocket,
    buffer: [u8; 4096],
}

impl FakeServer {
    fn bind() -> Self {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let buffer = [0; 4096];
        Self { socket, buffer }
    }

    fn port(&self) -> String {
        self.socket.local_addr().unwrap().port().to_string()
    }

    fn recv(&mut self) -> (Vec<u8>, SocketAddr) {
        let (size, peer) = self.socket.recv_from(&mut self.buffer).unwrap();
        (self.buffer[..size].to_vec(), peer)
    }

    fn reply(&self, packet: &[u8], peer: SocketAddr) {
        self.socket.send_to(packet, peer).unwrap();
    }

    /// Answers SynthDef loads and syncs; ignores every other command.
    fn acknowledge(&self, packet: &[u8], peer: SocketAddr) {
        match osc_address(packet) {
            "/d_load" => self.reply(SYNTHDEF_DONE, peer),
            "/sync" => self.reply(SYNCED, peer),
            _ => {}
        }
    }

    fn answer_sync(&mut self) {
        let (packet, peer) = self.recv();
        assert_eq!(osc_address(&packet), "/sync");
        self.reply(SYNCED, peer);
    }
}

fn check_bass_timing(flags: &[&str], offsets_ms: &[u64], final_syncs: usize) {
    let mut server = FakeServer::bind();
    let child = spawn_bass(&server, flags);
    let received = collect_bundle_times(&mut server, offsets_ms.len());
    // Answer the final /sync messages, which may follow the final audible note.
    (0..final_syncs).for_each(|_| server.answer_sync());
    assert_success(child);
    assert_bass_timing(&received, offsets_ms);
}

fn spawn_bass(server: &FakeServer, flags: &[&str]) -> Child {
    Command::new(env!("CARGO_BIN_EXE_spinel-sonic-lab"))
        .args(["--port", &server.port(), "--synthdef", BASS_SYNTHDEF])
        .args(flags)
        .arg("../composer/examples/bass.ndjson")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

/// Returns each bundle's timetag paired with the NTP time it arrived.
fn collect_bundle_times(server: &mut FakeServer, expected: usize) -> Vec<(u64, u64)> {
    let mut received = Vec::new();
    while received.len() < expected {
        let (packet, peer) = server.recv();
        server.acknowledge(&packet, peer);
        received.extend(bundle_arrival(&packet));
    }
    received
}

fn bundle_arrival(packet: &[u8]) -> Option<(u64, u64)> {
    (osc_address(packet) == "#bundle").then(|| (bundle_time(packet), now_ntp()))
}

fn assert_bass_timing(received: &[(u64, u64)], offsets_ms: &[u64]) {
    let (first_stamp, first_arrival) = received[0];
    let lead_ok = first_stamp.saturating_sub(first_arrival) > NTP_SECOND / 10;
    assert!(lead_ok, "first note needs >100ms lead: {received:?}");
    for ((stamp, _), ms) in received.iter().zip(offsets_ms) {
        assert_eq!(stamp - first_stamp, ms * NTP_SECOND / 1000);
    }
    let end = received.last().unwrap().0 + NTP_SECOND / 4;
    assert!(now_ntp() >= end, "CLI exited before last note ended");
}

/// Plays the fake-server side of loading, playing and freeing one sample.
struct SampleExchange {
    server: FakeServer,
    wav: PathBuf,
    loaded: bool,
    stamp: Option<u64>,
    freed: bool,
}

impl SampleExchange {
    fn new(server: FakeServer, wav: PathBuf) -> Self {
        let (loaded, stamp, freed) = (false, None, false);
        Self { server, wav, loaded, stamp, freed }
    }

    /// Answers packets until the buffer is freed; returns whether it was loaded.
    fn run_until_freed(mut self) -> bool {
        while !self.freed {
            self.answer_next();
        }
        self.loaded
    }

    fn answer_next(&mut self) {
        let (packet, peer) = self.server.recv();
        match osc_address(&packet) {
            "/d_load" | "/sync" => self.server.acknowledge(&packet, peer),
            "/b_allocRead" => self.load(&packet, peer),
            "#bundle" => self.schedule(&packet),
            "/b_free" => self.free(peer),
            _ => panic!("unexpected OSC packet: {packet:?}"),
        }
    }

    /// `/b_allocRead` pads its address to 16 bytes and type tags to 8,
    /// so the buffer ID is at byte 24 and the snapshot path at byte 28.
    fn load(&mut self, packet: &[u8], peer: SocketAddr) {
        assert!(self.stamp.is_none());
        assert_eq!(&packet[24..28], &0_i32.to_be_bytes());
        let snapshot = osc_string_at(packet, 28);
        assert_eq!(fs::read(snapshot).unwrap(), fs::read(&self.wav).unwrap());
        self.server.reply(&buffer_done("/b_allocRead"), peer);
        self.loaded = true;
    }

    fn schedule(&mut self, packet: &[u8]) {
        assert!(self.loaded);
        let due = bundle_time(packet);
        assert!(due > now_ntp());
        self.stamp = Some(due);
    }

    fn free(&mut self, peer: SocketAddr) {
        let length = (16832_u64 << 32) / 48000;
        assert!(now_ntp() >= self.stamp.unwrap() + length);
        self.server.reply(&buffer_done("/b_free"), peer);
        self.freed = true;
    }
}

struct TempDir(PathBuf);

impl TempDir {
    fn create(prefix: &str) -> Self {
        let name = format!("{prefix}-{}-{}", std::process::id(), now_ntp());
        let path = std::env::temp_dir().join(name);
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn kick_wav() -> PathBuf {
    let wav = Path::new(env!("CARGO_MANIFEST_DIR")).join("../samples/kick.wav");
    wav.canonicalize().unwrap()
}

fn write_sample_score(dir: &Path, wav: &Path) -> PathBuf {
    for name in ["analog_bass", "sample_mono", "sample_stereo"] {
        // This fake server tests command transport, not SynthDef validity.
        fs::write(dir.join(format!("{name}.scsyndef")), []).unwrap();
    }
    let score = dir.join("score.ndjson");
    fs::write(&score, sample_score(wav)).unwrap();
    score
}

fn sample_score(wav: &Path) -> String {
    let path = serde_json::to_string(wav.to_str().unwrap()).unwrap();
    [
        "{\"type\":\"tempo\",\"bpm\":120}\n".to_owned(),
        format!("{{\"type\":\"sample\",\"name\":\"kick\",\"path\":{path}}}\n"),
        "{\"type\":\"sample_note\",\"sample\":\"kick\",\"beat\":0,\"velocity\":0.5}\n".to_owned(),
    ]
    .concat()
}

fn spawn_sample_score(server: &FakeServer, dir: &Path, score: &Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_spinel-sonic-lab"))
        .args(["--port", &server.port(), "--synthdef"])
        .arg(dir.join("analog_bass.scsyndef"))
        .arg(score)
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

fn assert_success(child: Child) {
    let output = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
}

fn bundle_time(packet: &[u8]) -> u64 {
    u64::from_be_bytes(packet[8..16].try_into().unwrap())
}

fn osc_address(packet: &[u8]) -> &str {
    osc_string_at(packet, 0)
}

fn osc_string_at(packet: &[u8], start: usize) -> &str {
    let end = packet[start..].iter().position(|b| *b == 0).unwrap() + start;
    std::str::from_utf8(&packet[start..end]).unwrap()
}

/// Builds scsynth's `/done` reply for a buffer command on buffer 0.
fn buffer_done(command: &str) -> Vec<u8> {
    let mut done = [osc_string("/done"), osc_string(",si"), osc_string(command)].concat();
    done.extend(0_i32.to_be_bytes());
    done
}

/// Encodes an OSC string: NUL-terminated and padded to a 4-byte boundary.
fn osc_string(value: &str) -> Vec<u8> {
    let mut out = value.as_bytes().to_vec();
    out.resize(value.len() / 4 * 4 + 4, 0);
    out
}

fn now_ntp() -> u64 {
    system_time_to_ntp(SystemTime::now())
}
