# Console channel and Grid DSL contract (working design)

SSL and Neve are references for the vertical module layout only. This work
does not target the sound of either manufacturer's consoles.

This document defines the authoring and signal-flow contract. The full
authoring form below compiles with Spinel and plays: H/A, Dynamics, EQ, Aux
Send and return, Pan, Fader, and Grid all have an audio path. The Aux
operation is named `send_to` because Spinel interprets `send` as dynamic
method dispatch. NRT renders measure each module's processing (see
`docs/audio-levels.md`); the host listening check is still pending.

## Authoring example

```ruby
score = TrackScore.new(133)
score.sample_file :kick, "../../samples/kick.wav"

score.aux_return :room do
  reverb room: 0.35, damp: 0.45, wet: 0.25
  fader db: -12
end

score.track :kick do
  use_sample :kick
  ha gain_db: 0
  dynamics do
    compressor threshold_db: -18, ratio: 3, attack_ms: 10, release_ms: 100
  end
  eq do
    bell frequency_hz: 80, gain_db: 2, q: 1
  end
  aux do
    send_to :room, level_db: -18
  end
  pan position: 0
  fader db: -6

  loop bars: 4 do
    bar grid: 16 do
      hits 1, 5, 9, 13, velocity: 0.65
    end
  end
end

score.track :bass do
  use_synth :analog_bass
  ha gain_db: 0
  dynamics do
    compressor threshold_db: -16, ratio: 2, attack_ms: 15, release_ms: 120
  end
  eq do
    bell frequency_hz: 180, gain_db: -2, q: 1
  end
  aux do
    send_to :room, level_db: -24
  end
  pan position: 0
  fader db: -6

  loop bars: 4 do
    bar grid: 16 do
      hits 1, 5, note: "C3", duration_beats: 0.5, velocity: 0.7
      hits 9, note: "E3", duration_beats: 1.0, velocity: 0.8
    end
  end
end

score.emit
```

The source is selected once for a track. Existing `use_sample` and
`use_synth` inside `loop` remain valid during migration. New channel modules
appear in this order: `ha`, `dynamics`, `eq`, `aux`, `pan`, `fader`. A module may
be omitted; omission is an identity/bypass operation. Reordering or repeating
a module is an error before emitting any NDJSON. The `loop` follows channel
configuration. The source, channel configuration, and all Grid bars are
validated before output. A score may mix tracks with and without a channel
strip, including old NDJSON scores.

`ha gain_db:` is an input trim, not a model of a particular preamp circuit.
The first Dynamics processor is a feed-forward compressor. The EQ is a set of
ordered bands; this first form names a bell band. The Aux send taps the signal
after EQ and before Pan and Fader. The return mixes into the stereo master
after channel faders. `pan position:` is in the range -1 (left) to +1 (right), with 0
center. `fader db:` is a channel output level in decibels. Stereo samples
retain two channels; their pan control acts as a stereo balance. These choices
must be checked with audio measurements and listening before claiming the
specified processing behavior.

`bar grid:` still controls **hit position** only. `duration_beats:` controls
synth note length in quarter-note beats and does not change when the Grid
resolution changes. `velocity:` controls the strength of all positions in one
`hits` call; use separate calls for different strengths. Synth hits default to
one Grid step or 0.25 beat without a Grid, at velocity 0.65. Sample hits accept
velocity and retain whole-file playback; `duration_beats:` on a sample hit is
an error. Legacy `bar { hits 1, 2, 3, 3.75 }` remains one-based beat-position
syntax. Grid steps remain one-based integers.

## Runtime boundary (implemented)

The intended path is source synth/sample → channel audio bus → H/A →
compressor → EQ → Aux tap → Pan → Fader → stereo master. The Aux tap also
routes to a named return bus → return effect → return fader → stereo master.
For phase one, one explicit `send` targets one declared return. Missing return
names, duplicate returns, and invalid parameter ranges fail before audio
engine preparation. The parser validates the complete score before muting;
muting removes that track's source and sends while preserving returns used by
unmuted tracks. Old scores without channels use their existing direct path.

The NDJSON extension will carry music and channel settings, not OSC packet
bytes, audio bus numbers, or SuperCollider node IDs. Rust owns validated
channel configuration and scheduling. The OSC/audio layer owns buses, effect
nodes, ordering, and resource lifetime. Watch reloads prepare the new channel
graph before committing at the next bar boundary; invalid graph or engine
preparation keeps the active graph playing. Simultaneous hit bundles and the
200 ms timetag lead remain intact.

## Channel graph for Dynamics, EQ, and Aux (implemented)

The current per-event H/A and Fader multiplier is equivalent to a channel
strip only while there is no processor between them. A compressor must see
the **sum of simultaneous sounds on its track**; processing each note or
sample independently would produce a different result. The next audio path
therefore needs a stereo audio bus per active track, followed by a persistent
strip node. Source nodes run before strip nodes, and strip nodes before return
nodes. The strip reads its track bus and applies input trim, linked stereo
compression, the bell EQ, an Aux send tap, stereo pan/balance, and the Fader
in that order. A return node reads the named Aux bus, applies its effect and
Fader, then adds to the stereo master.

Bus numbers, group IDs, and node IDs belong to the audio engine. Scheduled
events should carry a logical route key, resolved to a bus by the prepared
graph at dispatch. Old notes and sample tails keep their graph until their
reserved end; an Aux effect may need an additional bounded tail. A watch
reload prepares new buses and nodes before the bar boundary, then switches
new triggers to them only if all score, sample, and graph preparation succeeds.
Failed preparation leaves the active graph running. The old graph retires
after its last source and effect tail, avoiding a cut at the boundary.

## Implementation and verification sequence

1. **Done in code and deterministic tests:** add `hits` velocity and
   `duration_beats`; preserve the old Grid and no-Grid forms. Verify actual
   MIDI values, beat offsets, durations, and simultaneous groups in Rust.
2. **Done for authoring and validation:** add all ordered module calls and
   Aux returns with a single channel declaration per configured track;
   compile with real Spinel and validate the NDJSON contract in Rust.
3. **Done in code, NRT and deterministic tests:** route one synth track and
   one sample track through H/A, Pan, and Fader; measure output level and
   stereo position. Preserve existing default output for scores without
   channel declarations. User listening on the host remains pending.
4. **Done in code, NRT and deterministic tests (2026-11-07):** persistent
   channel graph (`core/src/mixer.rs`), compressor, bell EQ, named Aux send and
   reverb return. `tests/check_channel_audio.py` verifies bypass identity,
   gain reduction, H/A before the compressor, bell boost on/off frequency,
   pre-Fader Aux tap with return tail, and strip pan law.
5. **Done except listening:** `composer/bin/emit_console_strip.rb` combines
   Grid and all modules; Rust tests cover routing, simultaneous bundles, mute
   with a shared return, staged boundary updates, failed preparation, and
   retirement. On host `scsynth -u 57111`, finite watch playback, a live Fader
   edit (confirmed with `/s_get` on the strip node), and an invalid reload
   (ignored, playback continued) ran without server errors; the root group was
   empty afterward. User listening on the host remains pending.

Known limits: one compressor, one bell, and one send per channel; one reverb
type of return; processed tracks always route through the strip while simple
H/A/Pan/Fader channels keep the per-event path; strip and return node IDs
start at 2000, groups are 1900/1901, and buses are allocated from 128, so a
server shared with other clients must not use those IDs.

The target is a working console-style signal path with Grid composition.
