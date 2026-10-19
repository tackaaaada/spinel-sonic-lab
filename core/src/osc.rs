use std::net::{ToSocketAddrs, UdpSocket};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::model::Oscillator;
use crate::schedule::ScheduledNote;

pub const OSC_IMMEDIATE_TIMETAG: u64 = 1;
const NTP_UNIX_OFFSET_SECONDS: u64 = 2_208_988_800;

pub struct OscClient {
    socket: UdpSocket,
}

impl OscClient {
    pub fn connect(host: &str, port: u16) -> Result<Self, String> {
        let target = (host, port)
            .to_socket_addrs()
            .map_err(|error| format!("invalid OSC address: {error}"))?
            .next()
            .ok_or("OSC address did not resolve")?;
        if !target.ip().is_loopback() {
            return Err("MVP only supports a local OSC server".to_owned());
        }
        let bind = if target.is_ipv4() {
            "127.0.0.1:0"
        } else {
            "[::1]:0"
        };
        let socket = UdpSocket::bind(bind).map_err(|error| format!("OSC bind failed: {error}"))?;
        socket
            .connect(target)
            .map_err(|error| format!("OSC connect failed: {error}"))?;
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .map_err(|error| format!("OSC timeout setup failed: {error}"))?;
        Ok(Self { socket })
    }

    pub fn load_synthdef(&self, path: &Path) -> Result<(), String> {
        let path = path
            .canonicalize()
            .map_err(|error| format!("SynthDef file unavailable: {error}"))?;
        let path = path.to_str().ok_or("SynthDef path must be UTF-8")?;
        self.send(&encode_load(path))?;
        let mut buffer = [0_u8; 2048];
        loop {
            let size = self
                .socket
                .recv(&mut buffer)
                .map_err(|error| format!("SynthDef load response failed: {error}"))?;
            let address = osc_address(&buffer[..size]);
            match address {
                Some("/done") => return Ok(()),
                Some("/fail") => return Err("scsynth rejected SynthDef load".to_owned()),
                _ => continue,
            }
        }
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
        let mut buffer = [0_u8; 2048];
        loop {
            let size = self
                .socket
                .recv(&mut buffer)
                .map_err(|error| format!("scsynth command response failed: {error}"))?;
            match osc_address(&buffer[..size]) {
                Some("/fail") => return Err("scsynth rejected an OSC command".to_owned()),
                Some("/synced") => return Ok(()),
                _ => continue,
            }
        }
    }

    fn send(&self, bytes: &[u8]) -> Result<(), String> {
        self.socket
            .send(bytes)
            .map_err(|error| format!("OSC send failed: {error}"))?;
        Ok(())
    }
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
        let message = encode_s_new(note);
        osc_i32(&mut out, message.len() as i32);
        out.extend_from_slice(&message);
    }
    out
}

pub fn encode_load(path: &str) -> Vec<u8> {
    let mut out = Vec::new();
    osc_string(&mut out, "/d_load");
    osc_string(&mut out, ",s");
    osc_string(&mut out, path);
    out
}

pub fn encode_s_new(note: &ScheduledNote) -> Vec<u8> {
    let tone = &note.tone;
    let mut out = Vec::with_capacity(224);
    osc_string(&mut out, "/s_new");
    osc_string(&mut out, ",siiisfsfsfsfsfsfsfsfsfsfsfsi");
    osc_string(&mut out, "analog_bass");
    osc_i32(&mut out, -1); // Auto-assigned ID; the envelope frees the node.
    osc_i32(&mut out, 0); // Add at head.
    osc_i32(&mut out, 0); // Root group exists in standalone scsynth.
    osc_string(&mut out, "freq");
    osc_f32(&mut out, note.frequency);
    osc_string(&mut out, "amp");
    osc_f32(&mut out, note.amplitude);
    osc_string(&mut out, "gain");
    osc_f32(&mut out, tone.gain);
    osc_string(&mut out, "dur");
    osc_f32(&mut out, note.duration_seconds);
    osc_string(&mut out, "cutoff");
    osc_f32(&mut out, tone.cutoff_hz);
    osc_string(&mut out, "attack");
    osc_f32(&mut out, tone.attack_seconds);
    osc_string(&mut out, "decay");
    osc_f32(&mut out, tone.decay_seconds);
    osc_string(&mut out, "sustain");
    osc_f32(&mut out, tone.sustain_level);
    osc_string(&mut out, "release");
    osc_f32(&mut out, tone.release_seconds);
    osc_string(&mut out, "lfoRate");
    osc_f32(&mut out, tone.lfo_rate_hz);
    osc_string(&mut out, "lfoDepth");
    osc_f32(&mut out, tone.lfo_depth);
    osc_string(&mut out, "osc");
    osc_i32(
        &mut out,
        match tone.oscillator {
            Oscillator::Saw => 0,
            Oscillator::Sine => 1,
            Oscillator::Pulse => 2,
            Oscillator::Noise => 3,
        },
    );
    out
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
            let mut packet = Vec::new();
            osc_string(&mut packet, "/b_allocRead");
            osc_string(&mut packet, ",isii");
            osc_i32(&mut packet, id);
            osc_string(&mut packet, path);
            osc_i32(&mut packet, 0);
            osc_i32(&mut packet, -1);
            self.send(&packet)?;
            self.wait_done("/b_allocRead", Some(id))
        })
    }
    fn free(&self, id: i32) -> Result<(), String> {
        let mut packet = Vec::new();
        osc_string(&mut packet, "/b_free");
        osc_string(&mut packet, ",i");
        osc_i32(&mut packet, id);
        self.send(&packet)?;
        self.wait_done("/b_free", Some(id))
    }
}

impl OscClient {
    fn wait_done(&self, command: &str, id: Option<i32>) -> Result<(), String> {
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut buffer = [0_u8; 2048];
        while std::time::Instant::now() < deadline {
            let n = self
                .socket
                .recv(&mut buffer)
                .map_err(|e| format!("{command} response failed: {e}"))?;
            let packet = &buffer[..n];
            if osc_address(packet) == Some("/fail") {
                return Err(format!("scsynth rejected {command}"));
            }
            if osc_address(packet) == Some("/done") {
                let mut expected = Vec::new();
                osc_string(&mut expected, "/done");
                osc_string(&mut expected, if id.is_some() { ",si" } else { ",s" });
                osc_string(&mut expected, command);
                if let Some(id) = id {
                    osc_i32(&mut expected, id);
                }
                if packet == expected {
                    return Ok(());
                }
            }
        }
        Err(format!("{command} response timed out"))
    }

    pub fn play_events_bundle(
        &self,
        timetag: u64,
        events: &[crate::schedule::PlaybackEvent],
        resolve: impl Fn(&crate::sample::SampleAsset) -> Result<i32, String>,
    ) -> Result<(), String> {
        if !events.is_empty() {
            self.send(&encode_playback_bundle(timetag, events, resolve)?)?;
        }
        Ok(())
    }
}

pub fn encode_playback_bundle(
    timetag: u64,
    events: &[crate::schedule::PlaybackEvent],
    resolve: impl Fn(&crate::sample::SampleAsset) -> Result<i32, String>,
) -> Result<Vec<u8>, String> {
    use crate::schedule::PlaybackSound;
    let mut out = Vec::new();
    osc_string(&mut out, "#bundle");
    out.extend_from_slice(&timetag.to_be_bytes());
    for event in events {
        let packet = match &event.sound {
            PlaybackSound::Synth(note) => encode_s_new(note),
            PlaybackSound::Sample { asset, .. } => {
                let mut packet = Vec::new();
                osc_string(&mut packet, "/s_new");
                osc_string(&mut packet, ",siiisisf");
                osc_string(
                    &mut packet,
                    if asset.channels == 1 {
                        "sample_mono"
                    } else {
                        "sample_stereo"
                    },
                );
                osc_i32(&mut packet, -1);
                osc_i32(&mut packet, 0);
                osc_i32(&mut packet, 0);
                osc_string(&mut packet, "buf");
                osc_i32(&mut packet, resolve(asset)?);
                osc_string(&mut packet, "amp");
                osc_f32(&mut packet, event.amplitude);
                packet
            }
        };
        osc_i32(&mut out, packet.len() as i32);
        out.extend(packet);
    }
    Ok(out)
}
