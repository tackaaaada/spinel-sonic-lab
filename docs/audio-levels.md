# Audio level check

The user's earlier listening check found the bass quiet. The examples now carry linear gain values chosen from separate Saw, Sine, and Pulse renders. All eight previews were regenerated with the current SynthDef as stereo, 16-bit WAV at 44.1 kHz. Each uses the same four-note BPM 120 timing and a 1.4-second file length, so their full-file RMS values are comparable. The channels contain the same signal.

| Preview | Gain | Peak (dBFS) | RMS (dBFS) |
|---|---:|---:|---:|
| Original Saw | 2.8 | -13.5 | -21.2 |
| Bright Saw | 2.6 | -12.4 | -20.9 |
| Sine | 1.1 | -15.1 | -20.4 |
| Pulse | 1.5 | -11.8 | -20.6 |
| Soft preset | 1.6 | -12.4 | -20.6 |
| Wobble preset | 3.5 | -11.6 | -20.8 |
| Static filter comparison | 3.5 | -11.5 | -20.2 |
| ADSR example | 3.0 | -13.0 | -26.0 |

The ADSR example retains a quieter average because it decays to a sustain level of 0.25. Matching its RMS to the other tones would obscure that envelope change. No preview reached full scale. These measurements cover one note at a time; overlapping notes can sum and clip.

The separate melody and chords preview has up to four simultaneous notes. At 44.1 kHz, its 1.5-second render measured a peak of −9.7 dBFS and an RMS of −21.9 dBFS. It is not included in the single-note comparison table because its note density and file length differ.

The 16-step bass preview uses short notes and a different rhythm. At 44.1 kHz, its 2.4-second stereo render measured a peak of −15.1 dBFS and an RMS of −28.4 dBFS. It is not comparable to the four-note table because its note lengths and file duration differ. No clipping occurred.

The bass-only track preview removes the E2 and G2 counter notes from that rhythm. At 44.1 kHz, its 2.0-second stereo render measured a peak of −15.1 dBFS and an RMS of −29.9 dBFS. Its shorter length means this RMS is not a direct loudness match to the full rhythm.

The drum presets (`kick` on Sine with pitch-drop transient and gain 3.8, `snare` on tonal shell + WhiteNoise with gain 3.4, and `hat` on high-passed WhiteNoise with gain 2.9) were rendered in a 2.0-second 16-step pattern at 44.1 kHz. Rendered individually at velocity 0.8, `kick` measured a peak of −5.2 dBFS (RMS −15.7 dBFS), `snare` measured a peak of −5.8 dBFS (RMS −25.7 dBFS), and `hat` measured a peak of −7.5 dBFS (RMS −29.4 dBFS); the combined drum bar measured a peak of −0.3 dBFS (on simultaneous kick + snare hits) and an RMS of −15.2 dBFS with no clipping.

In the 6-track `composer/bin/live_session.rb` mix (`kick`, `snare`, `hat`, `bass`, `chords`, and `melody`), each pitched track uses a distinct filter cutoff and gain (`bass` Saw at 1800 Hz / gain 3.4, `chords` Pulse at 1200 Hz / gain 1.8, `melody` Saw at 3800 Hz / gain 3.4) so the three pitched parts sit within 1 dB RMS of each other (`bass` −23.6 dBFS, `chords` −23.8 dBFS, `melody` −22.8 dBFS) and the full 6-track bar measures a peak of −1.9 dBFS and an RMS of −14.9 dBFS with no clipping.

`gain` is a linear multiplier after velocity and envelope. It accepts 0–4 in a `tone` event. A tone event without `gain` keeps the previous 1.0 behavior; the no-tone built-in bass now uses 2.8. RMS is only an approximate comparison of average signal level, not a perceptual loudness measurement or a human listening check. The user should judge the final balance on their output device.

## Generated one-shot drum assets

`samples/` contains stereo PCM16 WAVs rendered at 48kHz through the existing
SuperCollider SynthDef from `samples/recipes/*.ndjson` with `mise run samples`
(the `render_nrt` tool; originally `scripts/render_drum_samples.py`).
No existing preset gains or playback levels were changed. The asset recipe uses
amp 0.12 (velocity 0.6), with existing drum gains 3.8 / 3.4 / 2.9.

| Asset | Duration (seconds) | Peak (dBFS) | Whole-file RMS (dBFS) |
|---|---:|---:|---:|
| Kick | 0.3507 | -7.66 | -16.64 |
| Snare | 0.3013 | -8.10 | -24.13 |
| Hat | 0.1813 | -10.45 | -26.84 |

Measurements include silent tails. Noise-based renders vary slightly on regeneration.
These assets have individual peak headroom; perceived balance and combined mix
headroom still require listening and mix measurements. See [asset instructions](../samples/README.md).


## Sampler example: kick/bass separation

The listener reported obscured kick low end and bass attack. The original example
started every bass note with a kick (including both at C2 for the first hit), using
the soft Sine preset with a 20ms attack. This makes masking a plausible explanation;
it does not prove a specific acoustic cancellation mechanism.

`emit_samples.rb` now places bass notes at beats 0.5/1.5/2.5/3.5, keeping the
kick pattern at 0/1/2/3. At BPM 120 each bass starts 250ms after a kick and ends
50ms before the next. The bass is Saw, cutoff 1800Hz, ADSR 0.003/0.040/0.450/0.060,
gain 3.4, velocity 0.75. Drum assets, trigger levels and note pitches are unchanged.
The bass rhythm changes intentionally; this is an arrangement choice to audition.

SuperCollider NRT renders use the existing compiled SynthDefs, 48kHz PCM16 stereo,
four repeated bars, a common 50ms loading gap and 8.2-second file lengths.
Whole-file measurements including silence:

| Render | Peak dBFS | RMS dBFS |
|---|---:|---:|
| Before mix | -5.73 | -21.16 |
| Adjusted mix | -6.94 | -20.72 |
| Kick only (unchanged) | -11.41 | -22.03 |
| Before bass only (Sine, gain 1.6, velocity 0.55) | -15.73 | -26.68 |
| Adjusted bass only (Saw, gain 3.4, velocity 0.75) | -12.60 | -27.68 |

Because bass gain changed, all four oscillator choices were also rendered at the
adjusted bass settings, same notes, timing and file length:

| Oscillator | Peak dBFS | RMS dBFS |
|---|---:|---:|
| Saw | -12.60 | -27.68 |
| Sine | -6.87 | -19.59 |
| Pulse | -6.61 | -22.15 |
| Noise | -8.36 | -28.32 |

Only Saw is selected in the example; these controls are not a loudness-matched set.
None clipped. Noise can vary per render. Measurements do not establish perceived
kick/bass separation or perceived loudness matching. On 2026-11-01 the listener
confirmed hearing the sampler example after its PipeWire output was connected;
the kick/bass separation and perceived loudness still need their assessment.
Four-bar comparisons: [before](samples-mix-before.wav), [adjusted](samples-mix-after.wav).
The engine source and built-in presets are unchanged; the changed tone belongs only
to the sampler composition.

Validation for this adjustment: Rust 43 tests, all actual Spinel composition/fixture
checks, WAV asset checks and formatting passed. Live UDP tests remain opt-in.
The required sclang regeneration was attempted with offscreen/generic Qt but
failed during `_LocalIPs` initialization in the restricted execution environment.
NRT comparison used the host-generated, existing compiled definitions; no engine
source was changed. `mise run play-samples` regenerates both definitions on the
host before the next listening check.

## 2026-11-07 console channel render

After adding H/A trim, Pan, and Fader support, both SynthDefs were regenerated
with host `sclang`. `tests/check_channel_audio.py` (since replaced by
`core/tests/audio_render_tests.rs`) rendered the new
definitions with `scsynth -N` at 48 kHz, stereo PCM16. Each synth comparison
used the same 0.7-second file, C3, velocity-derived `amp = 0.1`,
`gain = 1.0`, cutoff 1800 Hz, and center pan. The dBFS values cover the whole
file including silence.

| Oscillator | Peak dBFS | RMS dBFS |
|---|---:|---:|
| Saw | -27.37 | -38.14 |
| Sine | -21.96 | -30.15 |
| Pulse | -23.35 | -33.16 |
| Noise | -24.31 | -41.57 |

The test checks that center pan yields identical left/right synth channels,
hard-left pan silences the right channel, and halving the synth amplitude
halves its RMS within quantization tolerance. Both mono and stereo samples
rendered hard left with a silent right channel; the hard-left to center left
peak ratio was approximately 1.414. The hard-left sample peaks were -19.69
dBFS (mono test signal) and -12.61 dBFS (current kick WAV). Noise figures vary
across renders. These are signal measurements; the console Grid example has
not yet been heard and judged by the user.

## 2026-11-07 channel strip render

`synthdefs/channel.scd` was compiled with host `sclang` into
`channel_strip.scsyndef` and `aux_return.scsyndef`. The former `tests/check_channel_audio.py`
now also renders a 200 Hz Sine through the strip (groups and node order as in
the Rust engine) and compares RMS over 0.3–0.8 s. Source gain and oscillator
levels did not change; the table above re-measured identically except for the
usual Noise variation (-24.22 / -41.36 dBFS this run).

| Check | Result |
|---|---:|
| Bypassed strip vs direct output | within 0.5% |
| Compressor 4:1, -20 dB threshold, input peak -6 dBFS | -9.60 dB |
| +12 dB H/A into that compressor | +3.00 dB out |
| Bell +6 dB at 200 Hz (on the tone) | +6.00 dB |
| Bell +6 dB at 5 kHz (off the tone) | +0.01 dB |
| Aux return with channel Fader closed | -17.53 dBFS RMS, tail -32.29 dBFS |
| Strip hard left / center left peak | ≈1.414, right silent |

The strip adds no make-up gain, so compression lowers level. These are signal
measurements; the strip has not yet been heard and judged by the user.

## 2026-11-07 Rust NRT render through the live encoders

`tests/check_channel_audio.py` and the Python OSC encoder were replaced by
`core/tests/audio_render_tests.rs` (`mise run audio-check`). Each check is now
an NDJSON score that goes through `Scheduler`, `ChannelGraph`, `SampleBuffers`
and the same OSC encoders as live playback, recorded into a `scsynth -N` score
by `core/src/nrt.rs` instead of sent over UDP. No SynthDef, gain or level
changed. Differences from the Python checks:

- Synth files end when the 0.4 s note ends instead of at 0.7 s, so whole-file
  RMS is higher with identical peaks: Saw -27.37 / -35.71, Sine -21.96 /
  -27.73, Pulse -23.35 / -30.73 dBFS; Noise varies (about -24.5 / -39 dBFS).
- Scores cannot express cutoff 0 or 200 Hz, so the strip source is G3
  (196 Hz) with cutoff 18 kHz, and the bell is centered on 196 Hz.
- The strip source peaks at 0.2 (velocity 1), so the compressor threshold is
  -28 dB to keep the Python check's 14 dB overshoot.
- A closed Fader and send use the protocol floor of -96 dB, not linear 0; the
  dry output keeps one PCM16 step of residue (-90.3 dBFS peak).

| Check | Result |
|---|---:|
| Sample hard left (mono test / kick) | -19.69 / -12.61 dBFS peak, right silent |
| Bypassed strip vs direct output | within 0.5% |
| Compressor 4:1, 14 dB over threshold | -9.65 dB |
| +12 dB H/A into that compressor | +3.00 dB out |
| Bell +6 dB at 196 Hz / at 5 kHz | +6.00 / +0.01 dB |
| Aux return with channel Fader at -96 dB | -20.79 dBFS RMS, tail -37.67 dBFS |
| Strip hard left / center left peak | 1.40–1.42, right silent |

The new tests also fail if scsynth reports `FAILURE IN SERVER`, which the
Python script would have rendered as silence. These remain signal
measurements, not a listening check.
