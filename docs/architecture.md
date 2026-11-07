# Architecture

```text
Spinel source → compiled composer → NDJSON → Rust CLI → OSC/UDP → scsynth → audio device
```

## Composition layer

`composer/lib/score_writer.rb` is a deliberately small Ruby-like DSL. The compiled examples write the fixed [score protocol](../protocol/README.md) to stdout. A composition may specify a default `analog_bass` tone and per-track `tone` events before its notes. Named presets (`soft`, `bright`, `wobble`, `kick`, `snare`, `hat`) in this layer expand to validated `tone` events, so Rust and scsynth do not know preset names. Named pitches, chord arrays (`chord` / `track_chord`), sequential arpeggios (`arpeggio` / `track_arpeggio`), 16-step patterns (`steps` / `track_steps` / `drum_steps`), and multi-bar pattern loops (`loop_steps` / `track_loop_steps` / `drum_loop_steps`) all validate their arguments first and expand to ordinary MIDI note events; Rust does not parse pitch strings, chord/arpeggio syntax, or patterns. The composer does not depend on a JSON Gem, audio server, FFI, or a Ruby interpreter at runtime. The compiler is needed when the composition source changes.

## Runtime core

`core/src/model.rs` parses and validates the entire score, including optional tone settings, before any OSC is sent. `core/src/schedule.rs` converts beats to offsets using a fixed BPM and MIDI notes to frequency. It contains no file, process, or worker-thread operations. Simultaneous notes share a beat offset and retain their source order; `group_notes_by_time` yields borrowed slices with identical `at` offsets so the playback loop dispatches each simultaneous group in a single OSC `#bundle` datagram without copying notes or allocating a group collection. Tests inspect these values without waiting in real time.

Within the model parser, `Score` enforces record order and sorts validated notes; `BassTone` and `MusicEvent` each validate their own record fields. The original line number travels into validation errors. These are parser responsibilities; OSC encoding stays in the transport module.

An optional note `track` field defaults to `main` for older scores. Optional `tone` events may also specify `track` so each track can use its own oscillator, filter, envelope, LFO, and gain while falling back to the score's default tone. The CLI's `--mute-track` applies to the validated score before the scheduler runs; an unknown track or empty result fails. The scheduler resolves each note's `BassTone` onto `ScheduledNote`, so track names do not enter `ScheduledNote` or OSC.

`LiveScheduler` in `core/src/schedule.rs` steps through 4-beat bars without a real-time clock. `core/src/source.rs` owns file loading and Spinel process execution. `ScoreWatcher` in `core/src/watch.rs` detects edits and passes valid scores to the scheduler. When `--watch FILE` is enabled (with an `.ndjson` or Spinel `.rb` file), the initial score is compiled if needed and validated before loading the SynthDef or sending OSC. During playback, `ScoreWatcher::start_background_poller` delegates compilation to a `BackgroundPoller` that owns its receiver and stop flag together. The playback path only drains ready results at bar boundaries; invalid edits report a warning and keep the last valid score. Dry-run and explicit synchronous staging continue to read and compile on the caller thread.

Bar transitions, tempo changes, and successful/failed edit sequences are tested without real-time sleeps. A separate worker test waits for a result through the channel with a bounded timeout, rather than assuming compilation or polling completes within a fixed sleep. Live test files have scope-based cleanup, including when assertions fail.

`core/src/osc.rs` handles UDP and the small OSC subset required here: `/d_load`, `/s_new`, `#bundle` (with NTP timetags), and `/sync`. It waits for SynthDef loading to complete and checks server failure responses after note commands. The music model has no dependency on this transport or SuperCollider's packet format. v0.1 accepts only a loopback server.

The CLI captures a monotonic/wall-clock origin after SynthDef loading and adds a fixed 200ms playback lead. Each simultaneous group is sent 200ms before its planned audible time with an explicit NTP timetag derived from that origin. Dispatch remains paced, rather than enqueueing the whole score. End waits include the lead. Late dispatch retains the planned timetag and warns; it cannot guarantee recovery of an already missed deadline. Wall-clock changes during playback do not rebase timestamps. The current SynthDef uses Out, so this does not claim sample-accurate output.

Watch consumes fully validated reload results at dispatch bar boundaries, 200ms before the corresponding audible boundary; edits ready after this cutoff are deferred to the following bar. Tempo changes keep the shared origin. Per-bar immediate /sync checks only commands already processed; a final check after the last note ends covers finite playback. Neither /synced nor a valid packet proves human audibility.

The deterministic clock test checks first-note lead, 250ms timestamp differences, bar offsets and overflow without sleeps. The CLI loopback test is opt-in (`cargo test --offline --test timing_tests -- --ignored`) because restricted environments may forbid UDP. Run it on the audio host before relying on transport validation.

## Audio engine

`synthdefs/analog_bass.scd` generates a `.scsyndef` with a selectable Saw (`0`), Sine (`1`), Pulse (`2`), or WhiteNoise (`3`) source → low-pass filter → ADSR envelope → gain → two output channels. The oscillator, cutoff, attack, decay, sustain, release, filter LFO, and gain are adjustable per score or per track. The control-rate sine LFO modulates the cutoff within a validated depth range and starts anew on each note. Optional decay, sustain, and LFO fields default to the earlier sound. Missing gain in older tone events means 1.0; the built-in no-tone bass uses 2.8. Gain is applied inside the SynthDef after the velocity-derived amplitude and envelope, leaving the scheduler independent of audio level. Each envelope fits in its note duration and frees its node at completion. `sclang` compiles the definition; standalone `scsynth` renders audio. Rust loads the compiled definition into the server before playback.

The OSC target is `127.0.0.1:57110` by default. There is no network performance, FFI, GUI, or custom DSP in this version.

## Console channel path

`TrackScore` can now place `ha gain_db:`, `pan position:`, and `fader db:` above its
four-bar Grid loop. The composer emits one validated `channel` declaration
per configured track before playback events. Rust validates channel names,
levels, pan position, duplicates, and declaration order before playback or
mute. The scheduler applies the H/A and Fader level to synth and sample
amplitudes and carries only the pan value to OSC; track names and protocol
objects do not enter the OSC packet. The generated SynthDefs use `Pan2` for
mono signals and `Balance2` for stereo samples, with a center correction that
preserves existing center output levels.

A track whose channel has Dynamics, EQ, or an Aux send is routed instead.
`schedule.rs` marks its playback events with a logical route (the track name)
and leaves their level at unity and pan at center. `mixer.rs` owns the engine
side: one private stereo bus and one persistent `channel_strip` node per routed
track, one `aux_return` node per used return, node IDs, bus numbers,
boundary-timed control updates, and retirement after the last source end (plus
a 3-second reverb tail for returns). `osc.rs` resolves the route to the bus
when encoding `/s_new` (`out`), creates a strip group and a following return
group at the tail of the root group so sources (root head) run first, and
sends `/n_set` bundles timetagged at the bar boundary. A watch reload commits
only after sample buffers and the graph are prepared; a failed node creation
frees the new nodes and keeps the active graph. Full design notes are in
[`console-grid-dsl.md`](console-grid-dsl.md).



## Sampler

`sample` declarations and `sample_note` events extend the protocol while leaving
legacy notes unchanged. `sample.rs` validates PCM16 WAV byte structure without
file access; `source.rs` resolves paths against the original score and reads all
declared assets before muting. Validated snapshots use shared immutable byte
arrays, so a source reload cannot overwrite an already reserved buffer's content.
`Scheduler::plan_playback` merges synthesized and sampled events, preserving beat
and input ordering. The scheduler uses WAV frame count and sample rate for exact
file duration, contains no OSC buffer IDs, and can be tested without a clock.
Legacy note-only `Scheduler::plan` remains available.

`sampler.rs` owns a bounded buffer pool. Its backend boundary permits tests of
load reuse, failed candidate rollback, and in-flight retention without UDP.
`osc.rs` loads snapshots via `/b_allocRead`, matches completion command and ID,
and emits mixed `/s_new` messages in one timestamped bundle. `source.rs` creates
and cleans up temporary snapshots after each load response. Mono and stereo
have separate `PlayBuf` SynthDefs in `synthdefs/sampler.scd`; `BufRateScale` keeps
original speed and `OffsetOut` preserves sample scheduling offsets. Whole-file
playback frees the node at its end. This uses SuperCollider standard UGens.

Offline rendering reuses the live path. `nrt.rs` plans a validated score with
`Scheduler::plan_playback`, prepares `SampleBuffers` and `ChannelGraph` through
a recording backend, and writes each simultaneous group with the same OSC
bundle encoder as live playback, using timetags counted from the file start.
Engine setup sits at time zero and an empty bundle sets the file length (the
last event end plus any reverb tail, or a fixed length). `nrt.rs` has no file
or process access; `source.rs::render_nrt` writes the command file, runs
`scsynth -N`, rejects `FAILURE IN SERVER` logs, and validates the WAV. The
`render_nrt` binary and `core/tests/audio_render_tests.rs` use this path, so
the audio checks measure the packets that live playback sends.

Watch preflights candidate engine resources before `LiveScheduler` changes its
active score. Source failures, invalid WAVs, muted-score errors, and engine load
failures leave the active composition in place. Buffers from earlier generations
are freed only when no longer active and past their last reserved event end.
Finite playback waits for the maximum end across all bars before dropping the
pool, including on an ordinary returned playback error. Force termination skips
Rust cleanup. One player per standalone server is required. New asset engine
loads occur on the playback thread and may consume the 200ms lead; this limit
is explicit rather than claiming unlimited seamless loading.

The synthdef task compiles both `analog_bass.scd` and `sampler.scd`.
Custom `--synthdef` paths require the generated `sample_mono.scsyndef` and
`sample_stereo.scsyndef` alongside the selected analog definition for sample scores.
Sampler render/transport integration still requires a host with sclang networking
initialization and local UDP permissions; deterministic tests do not prove audibility.
