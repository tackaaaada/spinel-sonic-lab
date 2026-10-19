use spinel_sonic_lab::model::Score;

const SAMPLE_SCORE: &str = concat!(
    "{\"type\":\"tempo\",\"bpm\":120}\n",
    "{\"type\":\"sample\",\"name\":\"kick\",\"path\":\"../../samples/kick.wav\"}\n",
    "{\"type\":\"sample_note\",\"sample\":\"kick\",\"beat\":0,\"velocity\":0.5,\"track\":\"drums\"}\n",
    "{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":36,\"beat\":0,\"duration_beats\":0.5,\"velocity\":0.5}\n"
);

#[test]
fn sample_declarations_and_triggers_accept_mixed_and_sample_only_scores() {
    let score = Score::parse_ndjson(SAMPLE_SCORE).unwrap();
    let sample_only = SAMPLE_SCORE.lines().take(3).collect::<Vec<_>>().join("\n");
    assert!(Score::parse_ndjson(&sample_only).is_ok());
    assert!(score.without_track("drums").is_ok());
    assert!(score.without_track("main").is_ok());
    assert!(score.without_track("unknown").is_err());
}

#[test]
fn sample_errors_are_rejected_even_on_a_muted_track() {
    for bad in [
        SAMPLE_SCORE.replace("\"sample\":\"kick\"", "\"sample\":\"missing\""),
        SAMPLE_SCORE.replace("\"velocity\":0.5", "\"velocity\":1.1"),
        SAMPLE_SCORE.replace("\"beat\":0", "\"beat\":-1"),
        SAMPLE_SCORE.replace("\"name\":\"kick\"", "\"name\":\"bad/name\""),
        SAMPLE_SCORE.replace("\"path\":\"../../samples/kick.wav\"", "\"path\":\"\""),
        SAMPLE_SCORE.replace("\"sample\":\"kick\"", "\"sample\":\"kick\",\"rate\":2"),
    ] {
        assert!(Score::parse_ndjson(&bad).is_err(), "{bad}");
    }
    let duplicate = SAMPLE_SCORE.replace(
        "{\"type\":\"sample_note\"",
        "{\"type\":\"sample\",\"name\":\"kick\",\"path\":\"x.wav\"}\n{\"type\":\"sample_note\"",
    );
    assert!(Score::parse_ndjson(&duplicate).is_err());
    let late =
        format!("{SAMPLE_SCORE}{{\"type\":\"sample\",\"name\":\"late\",\"path\":\"x.wav\"}}\n");
    assert!(Score::parse_ndjson(&late).is_err());
}

#[test]
fn wav_snapshot_checks_payload_and_format_before_muting() {
    use spinel_sonic_lab::sample::SampleAsset;
    let path = std::path::PathBuf::from("kick.wav");
    let wav = include_bytes!("../../samples/kick.wav").to_vec();
    let asset = SampleAsset::from_wav(path.clone(), wav.clone()).unwrap();
    assert_eq!(asset.channels, 2);
    assert_eq!(asset.frames, 16832);
    assert_eq!(asset.sample_rate, 48000);
    assert!(asset.duration() > std::time::Duration::from_millis(350));
    for bytes in [vec![], wav[..40].to_vec(), wav[..wav.len() - 1].to_vec()] {
        assert!(SampleAsset::from_wav(path.clone(), bytes).is_err());
    }
    let mut float = wav.clone();
    float[20..22].copy_from_slice(&3_u16.to_le_bytes());
    assert!(SampleAsset::from_wav(path.clone(), float).is_err());
    let mut channels = wav.clone();
    channels[22..24].copy_from_slice(&3_u16.to_le_bytes());
    assert!(SampleAsset::from_wav(path, channels).is_err());
}

#[test]
fn mixed_events_share_bundle_time_and_use_full_wav_length() {
    use spinel_sonic_lab::schedule::{PlaybackSound, Scheduler};
    use spinel_sonic_lab::source::prepare_score;
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../composer/examples");
    let score = prepare_score(SAMPLE_SCORE, &base, None).unwrap();
    let plan = Scheduler::plan_playback(&score).unwrap();
    assert_eq!(plan.len(), 2);
    assert_eq!(plan[0].at, std::time::Duration::ZERO);
    assert_eq!(plan[1].at, plan[0].at);
    assert!(matches!(plan[0].sound, PlaybackSound::Sample { .. }));
    assert!(plan[0].duration > std::time::Duration::from_millis(350));
    assert_eq!(plan[0].amplitude, 0.5);
    let muted = prepare_score(SAMPLE_SCORE, &base, Some("drums")).unwrap();
    assert_eq!(Scheduler::plan_playback(&muted).unwrap().len(), 1);
    let missing = SAMPLE_SCORE.replace("../../samples/kick.wav", "missing.wav");
    assert!(
        prepare_score(&missing, &base, Some("drums"))
            .unwrap_err()
            .contains("sample")
    );
}

#[test]
fn sample_buffers_load_once_keep_inflight_and_roll_back_failed_reload() {
    use spinel_sonic_lab::sample::SampleAsset;
    use spinel_sonic_lab::sampler::{SampleBackend, SampleBuffers};
    use std::{cell::RefCell, rc::Rc, time::Duration};
    #[derive(Clone)]
    struct Engine(Rc<RefCell<Vec<String>>>);
    impl SampleBackend for Engine {
        fn load(&self, id: i32, asset: &SampleAsset) -> Result<(), String> {
            self.0.borrow_mut().push(format!("load {id}"));
            if asset.path.to_string_lossy().contains("fail") {
                Err("load failed".into())
            } else {
                Ok(())
            }
        }
        fn free(&self, id: i32) -> Result<(), String> {
            self.0.borrow_mut().push(format!("free {id}"));
            Ok(())
        }
    }
    let log = Rc::new(RefCell::new(Vec::new()));
    let asset = SampleAsset::from_wav(
        "kick.wav".into(),
        include_bytes!("../../samples/kick.wav").to_vec(),
    )
    .unwrap();
    let mut pool = SampleBuffers::new(Engine(log.clone()));
    pool.prepare([&asset]).unwrap();
    pool.prepare([&asset]).unwrap();
    assert_eq!(&*log.borrow(), &["load 0"]);
    pool.mark_used(&asset, Duration::from_secs(3)).unwrap();
    pool.prune(Duration::from_secs(2), &[]).unwrap();
    assert_eq!(pool.buffer_id(&asset).unwrap(), 0);
    let mut failing = asset.clone();
    failing.path = "fail.wav".into();
    let mut another = asset.clone();
    another.path = "another.wav".into();
    assert!(pool.prepare([&another, &failing]).is_err());
    assert!(pool.buffer_id(&another).is_err());
    assert_eq!(pool.buffer_id(&asset).unwrap(), 0);
    pool.prune(Duration::from_secs(3), &[]).unwrap();
    assert!(pool.buffer_id(&asset).is_err());
}

#[test]
fn mixed_bundle_encodes_sample_buffer_but_no_track_or_path() {
    use spinel_sonic_lab::{
        osc::encode_playback_bundle, schedule::Scheduler, source::prepare_score,
    };
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../composer/examples");
    let score = prepare_score(SAMPLE_SCORE, &base, None).unwrap();
    let plan = Scheduler::plan_playback(&score).unwrap();
    let packet = encode_playback_bundle(123, &plan, |_| Ok(7)).unwrap();
    assert_eq!(&packet[..8], b"#bundle\0");
    assert_eq!(&packet[8..16], &123_u64.to_be_bytes());
    assert!(packet.windows(14).any(|x| x == b"sample_stereo\0"));
    assert!(!packet.windows(5).any(|x| x == b"drums"));
    assert!(!packet.windows(4).any(|x| x == b".wav"));
    let first_len = u32::from_be_bytes(packet[16..20].try_into().unwrap()) as usize;
    assert!(packet[20 + first_len + 4..].starts_with(b"/s_new\0"));
}

#[test]
fn compiled_drum_example_expands_steps_and_merges_with_bass() {
    use spinel_sonic_lab::{schedule::Scheduler, source::prepare_score};
    use std::time::Duration;
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../composer/examples");
    let score = prepare_score(
        include_str!("../../composer/examples/samples.ndjson"),
        &base,
        None,
    )
    .unwrap();
    let plan = Scheduler::plan_playback(&score).unwrap();
    let groups = plan
        .chunk_by(|a, b| a.at == b.at)
        .map(|g| (g[0].at, g.len()))
        .collect::<Vec<_>>();
    assert_eq!(
        groups,
        [
            (0, 1),
            (250, 2),
            (500, 2),
            (750, 2),
            (1000, 1),
            (1250, 2),
            (1500, 2),
            (1750, 2)
        ]
        .map(|(ms, n)| (Duration::from_millis(ms), n))
    );
    let notes = Scheduler::plan(&score).unwrap();
    for (note, midi) in notes.iter().zip([36, 40, 43, 48]) {
        assert_eq!(
            note.frequency,
            spinel_sonic_lab::schedule::midi_to_frequency(midi)
        );
    }
}

#[test]
fn failed_engine_preparation_keeps_the_active_score_at_the_next_bar() {
    use spinel_sonic_lab::watch::ScoreWatcher;
    use std::{
        fs,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    struct Temp(std::path::PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }
    let path = Temp(std::env::temp_dir().join(format!(
            "spinel-sample-watch-{}.ndjson",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
    let absolute = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../samples/kick.wav")
        .canonicalize()
        .unwrap();
    let initial = SAMPLE_SCORE.replace("../../samples/kick.wav", absolute.to_str().unwrap());
    fs::write(&path.0, &initial).unwrap();
    let mut watcher = ScoreWatcher::from_path(&path.0, None).unwrap();
    watcher.next_bar().unwrap();
    fs::write(&path.0, initial.replace("\"note\":36", "\"note\":48")).unwrap();
    let (bar, warning) = watcher
        .next_bar_with(&mut |_| Err("sample load failed".into()))
        .unwrap();
    assert_eq!(warning.as_deref(), Some("sample load failed"));
    assert_eq!(bar.start, Duration::from_secs(2));
    assert_eq!(
        bar.notes[0].frequency,
        spinel_sonic_lab::schedule::midi_to_frequency(36)
    );
    // A later valid edit is committed only once its engine preparation succeeds.
    fs::write(&path.0, initial.replace("\"note\":36", "\"note\":43")).unwrap();
    let (bar, warning) = watcher.next_bar_with(&mut |_| Ok(())).unwrap();
    assert!(warning.is_none());
    assert_eq!(bar.start, Duration::from_secs(4));
    assert_eq!(
        bar.notes[0].frequency,
        spinel_sonic_lab::schedule::midi_to_frequency(43)
    );
    // Missing assets, including on a muted track, cannot replace the active score.
    fs::write(
        &path.0,
        initial.replace(absolute.to_str().unwrap(), "/missing.wav"),
    )
    .unwrap();
    let (bar, warning) = watcher.next_bar().unwrap();
    assert!(warning.is_some());
    assert_eq!(
        bar.notes[0].frequency,
        spinel_sonic_lab::schedule::midi_to_frequency(43)
    );
}

#[test]
fn sample_only_live_score_loops_bars_and_tempo_change_keeps_sample_duration() {
    use spinel_sonic_lab::{schedule::LiveScheduler, source::prepare_score};
    use std::time::Duration;
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../composer/examples");
    let source = SAMPLE_SCORE.lines().take(3).collect::<Vec<_>>().join("\n");
    let score = prepare_score(&source, &base, None).unwrap();
    let length = score.sample_assets["kick"].duration();
    let mut live = LiveScheduler::new(score);
    let first = live.next_bar().unwrap();
    assert_eq!(first.playback.len(), 1);
    assert_eq!(first.playback[0].duration, length);
    let second = live.next_bar().unwrap();
    assert_eq!(second.playback[0].at, Duration::from_secs(2));
    assert_eq!(
        second.playback[0].end().unwrap(),
        Duration::from_secs(2) + length
    );
    live.reload_ndjson(&source.replace("120", "60"), None)
        .unwrap();
    let third = live.next_bar().unwrap();
    assert_eq!(third.start, Duration::from_secs(4));
    assert_eq!(third.duration, Duration::from_secs(4));
    assert_eq!(third.playback[0].duration, length);
    assert!(prepare_score(&source, &base, Some("drums")).is_err());
}

#[test]
fn validated_snapshot_is_cleaned_up_and_independent_of_original_file() {
    use spinel_sonic_lab::source::{prepare_score, with_sample_snapshot};
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../composer/examples");
    let score = prepare_score(SAMPLE_SCORE, &base, None).unwrap();
    let asset = &score.sample_assets["kick"];
    let mut snapshot_path = None;
    let result: Result<(), String> = with_sample_snapshot(asset, |path| {
        snapshot_path = Some(path.to_owned());
        assert_ne!(path, asset.path);
        assert_eq!(
            std::fs::read(path).unwrap().as_slice(),
            asset.bytes.as_ref()
        );
        Err("engine rejected it".into())
    });
    assert!(result.is_err());
    assert!(!snapshot_path.unwrap().exists());
}

#[test]
fn cli_validates_wavs_without_an_audio_server_and_accepts_sample_only_stdin() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let mut child = Command::new(env!("CARGO_BIN_EXE_spinel-sonic-lab"))
        .args(["--dry-run", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let absolute = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../samples/kick.wav")
        .canonicalize()
        .unwrap();
    let source = SAMPLE_SCORE
        .lines()
        .take(3)
        .collect::<Vec<_>>()
        .join("\n")
        .replace("../../samples/kick.wav", absolute.to_str().unwrap());
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "0.000s sample=kick dur=0.351s amp=0.500\n"
    );
    let output = Command::new(env!("CARGO_BIN_EXE_spinel-sonic-lab"))
        .args([
            "--dry-run",
            "--watch",
            "--bars",
            "2",
            "../composer/examples/samples.ndjson",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 28);
    assert!(
        stdout
            .lines()
            .nth(14)
            .unwrap()
            .starts_with("2.000s sample=kick")
    );
}

#[test]
fn muted_sample_still_gets_its_complete_musical_timing_validation() {
    let bad = SAMPLE_SCORE.replacen("\"beat\":0", "\"beat\":1e308", 1);
    assert!(spinel_sonic_lab::schedule::parse_and_validate_score(&bad, Some("drums")).is_err());
}

#[test]
fn wav_data_must_follow_its_format_chunk() {
    use spinel_sonic_lab::sample::SampleAsset;
    let wav = include_bytes!("../../samples/kick.wav");
    let mut reordered = wav[..12].to_vec();
    reordered.extend_from_slice(&wav[36..]);
    reordered.extend_from_slice(&wav[12..36]);
    assert!(SampleAsset::from_wav("reordered.wav".into(), reordered).is_err());
}

#[test]
fn pure_live_reload_cannot_commit_an_unprepared_sample() {
    use spinel_sonic_lab::{schedule::LiveScheduler, source::prepare_score};
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../composer/examples");
    let mut live = LiveScheduler::new(prepare_score(SAMPLE_SCORE, &base, None).unwrap());
    live.next_bar().unwrap();
    let changed = SAMPLE_SCORE.replace("../../samples/kick.wav", "new.wav");
    assert!(live.reload_ndjson(&changed, None).is_err());
    assert_eq!(
        live.next_bar().unwrap().start,
        std::time::Duration::from_secs(2)
    );
}

#[test]
fn sample_mix_gives_kick_its_own_attack_and_finishes_bass_before_next_kick() {
    use spinel_sonic_lab::{model::Oscillator, schedule::Scheduler, source::prepare_score};
    use std::time::Duration;
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../composer/examples");
    let score = prepare_score(
        include_str!("../../composer/examples/samples.ndjson"),
        &base,
        None,
    )
    .unwrap();
    let bass = Scheduler::plan(&score).unwrap();
    assert_eq!(
        bass.iter().map(|n| n.at).collect::<Vec<_>>(),
        [250, 750, 1250, 1750].map(Duration::from_millis)
    );
    for (index, note) in bass.iter().enumerate() {
        assert!(
            note.at.as_secs_f64() + f64::from(note.duration_seconds) < (index + 1) as f64 * 0.5
        );
        assert_eq!(note.tone.oscillator, Oscillator::Saw);
        assert!(note.tone.attack_seconds <= 0.003);
    }
    let kicks = score
        .sample_events
        .iter()
        .filter(|e| e.sample == "kick")
        .map(|e| e.beat)
        .collect::<Vec<_>>();
    assert_eq!(kicks, [0.0, 1.0, 2.0, 3.0]);
}
