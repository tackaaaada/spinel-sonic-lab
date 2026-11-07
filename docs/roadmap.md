# Roadmap and remaining work

[日本語](roadmap.ja.md)

Status as of 2026-11-01 (implementation, tests and the author's listening checks). The version column marks development stages, not released versions (the Cargo package version is 0.1.0).

| Stage | Focus | Status | Remaining |
|---|---|---|---|
| v0.1 | Spinel → Rust → SuperCollider sound | Done; confirmed by listening | — |
| v0.2 | Oscillators, ADSR, filter | Done | None at this granularity |
| v0.3 | 16-step sequencer, multi-track | Done | None at this granularity |
| v0.4 | Ruby-like music DSL | Basics and 4-bar in-track patterns done: per-bar 4–64th grids, named-pitch `hits`, hit length and velocity, and console-style H/A, Dynamics, EQ, Aux, Pan and Fader syntax | 8-bar syntax and per-track loop lengths. Dynamics/EQ/Aux play through persistent channel strips (not yet checked by listening) |
| v0.5 | Live coding | Recompile on save and bar-boundary switching done; a cutoff change was heard about 2–3 beats after saving | Exact switch boundary not measured; no interactive REPL |
| v0.6 | Presets and sampler | Code, SynthDef generation, OSC playback and sound confirmed on the host; the techno sketch's kick/bass balance is OK for now | Hardware check of saved edits; other arrangements judged individually |
| v1.0 | Integrated music-making environment | Completion criteria not defined | Define the workflow and acceptance criteria |

## Where things are implemented

- **Timing:** `core/src/main.rs`, `core/src/osc.rs`. Bundles carry future NTP timetags with a 200 ms lead.
- **Tones:** `core/src/model.rs`, `synthdefs/analog_bass.scd` — Saw/Sine/Pulse/Noise, ADSR, low-pass filter, filter LFO, gain.
- **Patterns and DSL:** `composer/lib/score_writer.rb`, `composer/lib/track_score.rb` — pitch names, chords, arpeggios, 16-step patterns, multi-bar loops, named tracks, per-track tones, muting; plus the 4-bar `track` / `loop bars: 4` / `bar grid:` / `hits` syntax.
- **Live editing:** `core/src/source.rs`, `watch.rs`, `schedule.rs` — Spinel compilation, background validation, bar-boundary switching, keeping the active score on invalid edits. Edits are accepted until 200 ms before the audible boundary.
- **Presets:** `soft`, `bright`, `wobble`, `kick`, `snare`, `hat`, expanded by the composer into ordinary `tone` events.
- **Tests:** `core/tests/`, `tests/check_spinel.sh`, plus `mise run audio-check` for offline audio measurements.

## Sampler (v0.6)

Whole-file, original-pitch playback is implemented: sample declarations and `sample_note` events, DSL helpers, WAV validation before any OSC, merged scheduling with synth notes, buffer lifetime management, and rollback if engine preparation fails on a live reload. `mise run play-samples` and `mise run watch-samples` run it. The drum WAVs in `samples/` are rendered from the project's own SynthDefs (see [samples/README.md](../samples/README.md)).

## v1.0 completion criteria (undecided)

For a CLI-centred environment the candidate workflow is: prepare a composition → validate → listen → live-edit → export WAV. Preview WAVs exist, but there is no product CLI that exports an arbitrary user score to WAV. GUI, interactive REPL, MIDI input and external plugins are out of scope for the current MVP.

## Long-term sound goals (proposed, not implemented)

The author wants to build tracks whose tone moves step by step, with drum sounds inspired by the Roland TR-1000 and bass sounds inspired by the TB-303 / Devil Fish. Reproducing any machine's full circuit or feature set is not claimed.

Today `tone` applies per score or track, and the sampler offers original-pitch whole-file playback with per-step velocity. Per-step cutoff, slides between notes, accent-linked volume and filter, resonance and overdrive are not implemented.

Candidate steps:

1. Pick short reference sounds and patterns to compare by ear; measure peak/RMS separately.
2. Express per-step volume and tone in the composition layer, adding only needed parameters to the backward-compatible NDJSON, with Rust validation and simultaneous-timing tests. No OSC or node IDs in music events.
3. Design 303-style accent and slide as single-voice behaviour across notes; treat Devil Fish extras later.
4. For TR-1000-style drums, start with a few sounds and per-step variation rather than every engine and effect.
