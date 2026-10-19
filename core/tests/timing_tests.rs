use spinel_sonic_lab::osc::system_time_to_ntp;
use std::net::UdpSocket;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

#[test]
#[ignore = "requires loopback UDP permission; run explicitly on the audio host"]
fn cli_sends_future_timestamps_with_even_spacing_and_waits_for_last_note() {
    for watch in [false, true] {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_spinel-sonic-lab"));
        command.args([
            "--port",
            &server.local_addr().unwrap().port().to_string(),
            "--synthdef",
            "../synthdefs/analog_bass.scsyndef",
        ]);
        if watch {
            command.args(["--watch", "--bars", "2"]);
        }
        let child = command
            .arg("../composer/examples/bass.ndjson")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut buffer = [0; 4096];
        let mut stamps = Vec::new();
        let mut first_lead = None;
        let expected = if watch { 8 } else { 4 };
        while stamps.len() < expected {
            let (size, peer) = server.recv_from(&mut buffer).unwrap();
            let packet = &buffer[..size];
            if packet.starts_with(b"/d_load\0") {
                server.send_to(b"/done\0\0\0,\0\0\0", peer).unwrap();
            } else if packet.starts_with(b"/sync\0") {
                server.send_to(b"/synced\0,i\0\0\0\0\0\x01", peer).unwrap();
            } else if packet.starts_with(b"#bundle\0") {
                let stamp = u64::from_be_bytes(packet[8..16].try_into().unwrap());
                first_lead.get_or_insert_with(|| {
                    stamp.saturating_sub(system_time_to_ntp(SystemTime::now()))
                });
                stamps.push(stamp);
            }
        }
        // Answer the final /sync (which may follow the final audible note).
        let (size, peer) = server.recv_from(&mut buffer).unwrap();
        assert!(buffer[..size].starts_with(b"/sync\0"));
        server.send_to(b"/synced\0,i\0\0\0\0\0\x01", peer).unwrap();
        if watch {
            let (size, peer) = server.recv_from(&mut buffer).unwrap();
            assert!(buffer[..size].starts_with(b"/sync\0"));
            server.send_to(b"/synced\0,i\0\0\0\0\0\x01", peer).unwrap();
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            first_lead.unwrap() > (1_u64 << 32) / 10,
            "first note needs >100ms lead; stamps={stamps:?}"
        );
        let offsets = if watch {
            vec![0, 250, 500, 750, 2000, 2250, 2500, 2750]
        } else {
            vec![0, 250, 500, 750]
        };
        for (stamp, ms) in stamps.iter().zip(offsets) {
            assert_eq!(*stamp - stamps[0], ms * (1_u64 << 32) / 1000);
        }
        let end = stamps.last().unwrap() + (1_u64 << 30);
        assert!(
            system_time_to_ntp(SystemTime::now()) >= end,
            "CLI exited before last note ended"
        );
    }
}

#[test]
#[ignore = "requires loopback UDP permission; run explicitly on the audio host"]
fn cli_preloads_samples_then_frees_buffers_after_the_full_file_finishes() {
    use std::fs;
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn string(value: &str) -> Vec<u8> {
        let mut out = value.as_bytes().to_vec();
        out.push(0);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
        out
    }
    let dir = Directory(std::env::temp_dir().join(format!(
        "spinel-udp-sample-{}-{}",
        std::process::id(),
        system_time_to_ntp(SystemTime::now())
    )));
    fs::create_dir(&dir.0).unwrap();
    for name in ["analog_bass", "sample_mono", "sample_stereo"] {
        // This fake server tests command transport, not SynthDef validity.
        fs::write(dir.0.join(format!("{name}.scsyndef")), []).unwrap();
    }
    let wav = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../samples/kick.wav")
        .canonicalize()
        .unwrap();
    let source = format!(
        "{{\"type\":\"tempo\",\"bpm\":120}}\n{{\"type\":\"sample\",\"name\":\"kick\",\"path\":{}}}\n{{\"type\":\"sample_note\",\"sample\":\"kick\",\"beat\":0,\"velocity\":0.5}}\n",
        serde_json::to_string(wav.to_str().unwrap()).unwrap()
    );
    let score = dir.0.join("score.ndjson");
    fs::write(&score, source).unwrap();
    let server = UdpSocket::bind("127.0.0.1:0").unwrap();
    server
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_spinel-sonic-lab"))
        .arg("--port")
        .arg(server.local_addr().unwrap().port().to_string())
        .arg("--synthdef")
        .arg(dir.0.join("analog_bass.scsyndef"))
        .arg(score)
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut buffer = [0; 4096];
    let mut loaded = false;
    let mut stamp = None;
    let mut saw_snapshot = false;
    loop {
        let (n, peer) = server.recv_from(&mut buffer).unwrap();
        let packet = &buffer[..n];
        if packet.starts_with(b"/d_load\0") {
            server.send_to(b"/done\0\0\0,s\0\0/d_load\0", peer).unwrap();
        } else if packet.starts_with(b"/b_allocRead\0") {
            assert!(stamp.is_none());
            let start = 24; // padded address(16), type tags(8), then id(4): see below
            let id_offset = 16 + 8;
            assert_eq!(&packet[id_offset..id_offset + 4], &0_i32.to_be_bytes());
            let path_start = start + 4;
            let end = packet[path_start..].iter().position(|b| *b == 0).unwrap() + path_start;
            let snapshot = std::str::from_utf8(&packet[path_start..end]).unwrap();
            assert_eq!(fs::read(snapshot).unwrap(), fs::read(&wav).unwrap());
            saw_snapshot = true;
            let mut done = string("/done");
            done.extend(string(",si"));
            done.extend(string("/b_allocRead"));
            done.extend(0_i32.to_be_bytes());
            server.send_to(&done, peer).unwrap();
            loaded = true;
        } else if packet.starts_with(b"#bundle\0") {
            assert!(loaded);
            let due = u64::from_be_bytes(packet[8..16].try_into().unwrap());
            assert!(due > system_time_to_ntp(SystemTime::now()));
            stamp = Some(due);
        } else if packet.starts_with(b"/sync\0") {
            server.send_to(b"/synced\0,i\0\0\0\0\0\x01", peer).unwrap();
        } else if packet.starts_with(b"/b_free\0") {
            let due = stamp.unwrap();
            let length = (16832_u64 << 32) / 48000;
            assert!(system_time_to_ntp(SystemTime::now()) >= due + length);
            let mut done = string("/done");
            done.extend(string(",si"));
            done.extend(string("/b_free"));
            done.extend(0_i32.to_be_bytes());
            server.send_to(&done, peer).unwrap();
            break;
        } else {
            panic!("unexpected OSC packet: {packet:?}");
        }
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(saw_snapshot);
}
