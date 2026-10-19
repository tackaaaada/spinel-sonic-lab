# Development rules

- For future behavior changes, work in small Red-Green-Refactor cycles: make a focused test fail for the expected reason, implement the behavior, then improve the design with the test green. For pure refactors, first characterize the existing behavior and keep the tests green throughout.
- Apply design principles to concrete change costs and invariants. Keep components cohesive and dependencies explicit; avoid value objects, strategies, or other abstractions without a specific problem they solve.
- Keep the composition, runtime model/scheduler, OSC transport, and audio engine separate. Music events must not contain OSC packet data or SuperCollider node IDs.
- Maintain the NDJSON contract in `protocol/README.md`, including the optional tone event, oscillator, ADSR, LFO, and gain controls, and compatibility with v0.1 scores. When changing it, update producers, consumers, fixtures, and tests.
- Use only Spinel-supported constructs in `composer/`; compile the source with Spinel during validation. Do not silently substitute CRuby as proof of compatibility.
- Keep pitch-name, chord, and arpeggio expansion in the composer. Verify new note syntax by compiling with Spinel, and check the resulting MIDI values and simultaneous scheduling in Rust tests.
- Keep 16-step, drum-step, and multi-bar loop pattern expansion in the composer. Reject malformed patterns before emitting their notes, and verify the merged event times in Rust without real-time waits.
- Preserve legacy notes without a `track` field as `main`. Validate complete scores before applying `--mute-track`, and keep track names out of OSC. Verify both named-track and muted playback paths.
- Keep named presets in the composition layer. They should emit validated `tone` events rather than introduce preset names into the Rust or OSC protocol.
- Validate the complete score before loading the SynthDef or sending notes. Keep scheduler calculations testable without real-time waits.
- Keep file access and Spinel process execution in `source.rs`, edit detection and worker ownership in `watch.rs`, and clock-free musical scheduling in `schedule.rs`. For worker tests, synchronize on a result notification with a bounded timeout rather than a fixed sleep.
- For live score reloading (`--watch` and `--bars`), switch to a reloaded score only at the next 4-beat bar boundary after full score and `--mute-track` validation; if a reload fails validation or cannot be read, keep playing the active score and verify both paths without real-time waits.
- Keep the OSC surface small. Group simultaneous notes into `#bundle` packets to avoid inter-packet jitter. Do not add FFI, GUI, network performance, new DSP, or speculative abstractions for future engines in this MVP.
- The CLI sends explicit future NTP timetags with a fixed 200ms lead from a shared monotonic/wall-clock origin. Preserve simultaneous bundles and end waits; watch snapshots validated reloads 200ms before the audible bar boundary. Late dispatch keeps its planned timetag and warns.
- Run `cargo test --offline` in `core/`, `SPINEL_BIN=/path/to/spinel sh tests/check_spinel.sh` at the project root for all compositions, and regenerate the SynthDef with `sclang synthdefs/analog_bass.scd` after sound changes.
- Measure peak and RMS for Saw, Sine, Pulse, and Noise when changing gain. Keep the current example levels and headroom documented in `docs/audio-levels.md`; ask for subjective listening feedback before claiming a perceived loudness match.
- For audio integration, start standalone `scsynth -u 57110`, run the compiled composer into the Rust CLI, and inspect both CLI output and scsynth logs. A successful OSC exchange alone is not proof that a human heard the sound; report that distinction.


- Sampler declarations and sample_note triggers preserve legacy scores. Validate all WAVs before mute or OSC; keep WAV reads/snapshots in source.rs and pure WAV parsing in sample.rs. Keep buffer IDs and ownership in sampler.rs/osc.rs, never music events. Preserve mono/stereo original-rate whole-file playback, sample duration end waits, and mixed simultaneous bundles. Engine preparation must succeed before committing a watch reload; retain buffers through their last reserved end.
- After sampler sound changes, also regenerate sample_mono.scsyndef and sample_stereo.scsyndef with `sclang synthdefs/sampler.scd`. Use the task's offscreen/generic Qt environment on Omarchy.
