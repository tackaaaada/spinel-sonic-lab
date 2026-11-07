# NDJSON score protocol

[日本語の概要](README.ja.md) (this English document is normative)

UTF-8 text, one JSON object per line. Blank lines are ignored. The first object must be a tempo event. It may be followed by `tone` and `sample` declarations, then one or more `note` or `sample_note` playback events. The entire score is validated before playback. Unknown fields and event types are errors.

```json
{"type":"tempo","bpm":120}
{"type":"tone","instrument":"analog_bass","oscillator":"saw","cutoff_hz":1800.0,"attack_seconds":0.005,"release_seconds":0.060,"gain":2.600}
{"type":"note","instrument":"analog_bass","note":36,"beat":0.0,"duration_beats":0.5,"velocity":0.8}
```

`bpm` and `duration_beats` must be positive finite numbers. `beat` must be a nonnegative finite number. `note` is an integer from 0 to 127; the octave convention is C2=36 and C3=48. `velocity` is between 0 and 1. The only synthesized note instrument is `analog_bass`; recorded sounds use `sample_note` instead. Events are played in ascending beat order; events with the same beat retain their input order. One beat means one quarter note.

The tempo event may include optional `"bars"`, an integer from 1 to 1024. It sets the score's loop length in 4-beat bars, including silent bars. Playback events must start before `bars × 4` beats; a sample may finish after that boundary. Older scores without `bars` keep the existing behavior: loop length is inferred from the latest playback event's start beat. `bars` is music timing metadata and is never sent as OSC.

Each note may have a `track` string. The name is 1–32 ASCII letters, digits, `_`, or `-`; a note without it belongs to `main`. Tracks share the score BPM and `analog_bass` instrument. A `tone` event may also include `"track"` to configure the tone for that specific track; an untracked `tone` event sets the default score tone for `main` and any track without its own `tone` event. At most one untracked `tone` and one `tone` per track name may appear before the notes. The runtime can omit one named track with `--mute-track NAME` after validating the whole score and before scheduling. An unknown track or a mute that leaves no notes is an error. Track names are music metadata and are not sent as OSC controls. See `composer/examples/tracks.ndjson`.

Optional `tone` events must appear after tempo and before all playback events; `sample` declarations may be interleaved with them. `oscillator` may be `saw`, `sine`, `pulse`, or `noise`; if omitted, it defaults to `saw` for compatibility with earlier scores. `cutoff_hz` is 40–18000 Hz. `attack_seconds`, `decay_seconds`, and `release_seconds` are each 0–5 seconds. `sustain_level` is 0–1. Decay and sustain are optional; older scores default to 0 seconds and level 1. The envelope starts at 0, rises to 1, decays to the sustain level, holds, and releases to 0. The stages fit inside each note's `duration_beats`; if attack, decay, and release exceed the note duration, they are scaled. When `tone` is absent, the v0.1 sound settings are used. See `composer/examples/adsr_bass.ndjson` for an ADSR example.

Optional `lfo_rate_hz` and `lfo_depth` must appear together. The rate is 0.1–20 Hz, and the depth is 0–1. A sine LFO moves the low-pass cutoff between `cutoff_hz × (1 - lfo_depth)` and `cutoff_hz`, clipped to 40–18000 Hz. Its phase restarts for each note. Omitted fields default to zero modulation, preserving older scores. See `composer/examples/lfo_bass.ndjson`.

Optional `gain` is a linear output multiplier from 0 to 4, applied after velocity and the envelope. A `tone` event without `gain` uses 1.0 for compatibility with older tone scores. A score without a `tone` event uses the built-in bass setting with gain 2.8. All current Spinel examples and presets have calibrated gains. Individual notes in the supplied examples retain headroom; overlapping notes can sum, so the range does not guarantee a clip-free mix.

The protocol contains music and instrument settings, not OSC packets or SuperCollider node IDs. Rust converts note numbers to frequency and beat lengths to seconds and maps validated tone settings to SynthDef controls.

## Console channel settings (H/A, Pan, Fader)

```json
{"type":"channel","track":"bass","ha_gain_db":6,"pan":-1,"fader_db":-6}
```

An optional `channel` event appears after tempo and before any playback event,
alongside `tone`, `sample`, and `aux_return` declarations. At most one channel event is allowed
per track, and the named track must have at least one playback event in the
complete score. `track` uses the same name rules as notes. `ha_gain_db` and
`fader_db` are finite numbers from -96 to +24 dB; `pan` is a finite number from
-1 (left) through 0 (center) to +1 (right). All three fields are required in
the NDJSON event. Unknown fields and duplicate or late channel declarations
fail before engine preparation. A score without channel events keeps its
previous sound path. Muting validates the complete score, then removes the
muted channel and its playback events.

H/A is input trim and Fader is output level. For a channel without Dynamics,
EQ, or Aux, their combined linear multiplier is `10 ** ((ha_gain_db +
fader_db) / 20)` and applies to both synthesized and sampled events. Synth
`tone.gain` remains a separate instrument setting. Pan uses constant-power
mono placement for synth and mono samples, and stereo balance for stereo
samples. Center output is level-matched to the former dual-mono or stereo
path; hard left/right may have a higher peak, so channel settings do not
guarantee clipping headroom. Audio bus numbers and node IDs stay in the
transport/audio engine, never in this event.

A channel with `compressor`, `eq_bell`, or `aux_send` is processed by a
persistent per-track channel strip: the track's sources play at unity and
center into a private stereo bus, and the strip applies all modules to their
sum. These fields and `aux_return` declarations are checked for structure,
ranges, and return references before engine preparation:

```json
{"type":"aux_return","name":"room","reverb":{"room":0.35,"damp":0.45,"wet":0.25},"fader_db":-12}
{"type":"channel","track":"kick","ha_gain_db":0,"pan":0,"fader_db":-6,"compressor":{"threshold_db":-18,"ratio":3,"attack_ms":10,"release_ms":100},"eq_bell":{"frequency_hz":80,"gain_db":2,"q":1},"aux_send":{"return":"room","level_db":-18}}
```

`compressor` is optional, with finite threshold -60..0 dB, ratio 1..20,
attack 0.1..200 ms, and release 5..2000 ms. Optional `eq_bell` has frequency
20..20000 Hz, gain -18..18 dB, and Q 0.1..10. Optional `aux_send` names one
declared return and has level -96..0 dB. An `aux_return` has a unique valid
name, `room`, `damp`, and `wet` each 0..1, and a Fader -96..24 dB. All nested
fields shown are required when their object is present; unknown fields,
duplicates, late declarations, invalid ranges, and missing returns are errors.
The full score is validated before track muting. The audio order is
source → H/A → compressor → bell EQ → Aux tap → Pan → Fader; the return effect
(fully wet reverb scaled by `wet`) and its Fader mix into the stereo master
afterward. An omitted compressor or bell is bypassed. Only returns fed by an
unmuted processed channel are instantiated. On watch reload, changed strip and
return settings apply at the next bar boundary.


## Whole-file samples (backward-compatible extension)

```json
{"type":"tempo","bpm":120}
{"type":"sample","name":"kick","path":"../../samples/kick.wav"}
{"type":"sample_note","sample":"kick","beat":0.0,"velocity":0.65,"track":"drums"}
```

`sample` declares a unique asset name (1–32 ASCII letters, digits, `_`, `-`) and a
nonempty file path with no NUL. All declarations must precede playback events.
Relative paths resolve against the original `.rb` or `.ndjson` score's directory;
stdin resolves them against the CLI's current working directory. Absolute paths
are accepted. JSON escaping applies to quotes and backslashes. Sample names,
paths, and track names are not playback OSC controls.

`sample_note` requires a declared `sample`, a finite nonnegative `beat`, and
`velocity` in 0–1. Optional `track` defaults to `main`. Its amplitude is the
velocity itself: the WAV already contains its recorded level. `tone`, gain and
ADSR settings affect synthesized notes only. There is no sample normalization,
pitch shifting, rate option, loop, or musical gate in this version. A sample
always plays once from its first frame to its last at its original pitch and
speed, even across a bar boundary or tempo change. Mono is duplicated to stereo;
stereo retains its channel layout.

Supported files are RIFF/WAVE PCM16, 1 or 2 channels, 8000–192000 Hz, at most
30 seconds and 32MiB per asset. Extensible WAV, float WAV and compressed audio
are rejected. Entire files, including RIFF/chunk lengths, alignment and data,
are validated before loading any SynthDef or sending playback commands. Every
declared file is checked, including declarations unused by playback or belonging
to a muted track. A score has at most 64MiB of declared input WAV data.

Notes and sample triggers are merged in ascending beat order; simultaneous
events preserve original input order in one bundle. Sample-only scores are valid.
`--mute-track` validates the complete score and its assets first, then removes
that track across both event types. A mute that leaves no playback events fails.
The CLI's dry-run validates WAVs and reports real sample durations without OSC.

The runtime retains snapshots of validated file bytes. It loads the required
snapshots into SuperCollider before playback and waits for each matching
`/done /b_allocRead bufferID`. Explicit buffer IDs belong to the transport layer,
never the score protocol or scheduler. Active and outstanding playback buffers
are retained until their last reserved playback end; failed reload preparation
releases new allocations and retains the active score. The buffer cache is
bounded to 64MiB of WAV input data and 1024 buffers. Use one player on a dedicated
standalone server; simultaneous independent clients sharing buffer IDs are not
supported. Normal finite completion releases the buffers; force-killing the
process may require restarting its server to reclaim them.

For watch, WAV bytes are refreshed when the composition source changes; editing
only the WAV does not trigger a reload. Source compilation and WAV validation
happen on the worker. Engine loading of new assets occurs at the dispatch bar
boundary, before committing the candidate score. It can exceed the 200ms lead on
a slow load; the CLI then reports late dispatch rather than rebasing the timeline.
Already loaded unchanged assets are reused. Invalid files or engine load failures
keep the active score. Initial loading occurs before the common playback origin.
