# Drum samples

[日本語](README.ja.md)

Kick, snare and hat WAVs rendered by SuperCollider from the project's own `analog_bass` drum synthesis (see `recipes/`). No third-party audio is used; the files are under CC BY 4.0 ([LICENSE-AUDIO](../LICENSE-AUDIO)). They let the sampler play drums at original pitch and speed instead of the synthesized drums. With the audio server running, `mise run play-samples` plays them.

| File | Format | Length | Peak | RMS |
|---|---|---:|---:|---:|
| [kick.wav](kick.wav) | 48 kHz / stereo / PCM16 | 0.3507 s | −7.66 dBFS | −16.64 dBFS |
| [snare.wav](snare.wav) | same | 0.3013 s | −8.10 dBFS | −24.13 dBFS |
| [hat.wav](hat.wav) | same | 0.1813 s | −10.45 dBFS | −26.84 dBFS |

RMS covers the whole file including trailing silence and does not imply matched perceived loudness. Each sample is checked not to clip on its own; check the mix when layering them. The WAVs are stored with Git LFS (`git lfs pull`).

## Regenerate

```sh
mise run samples
```

Or step by step from the repository root:

```sh
sclang synthdefs/analog_bass.scd
cargo build --offline --manifest-path core/Cargo.toml --bin render_nrt
core/target/debug/render_nrt samples/recipes/kick.ndjson samples/kick.wav 0.35
core/target/debug/render_nrt samples/recipes/snare.ndjson samples/snare.wav 0.30
core/target/debug/render_nrt samples/recipes/hat.ndjson samples/hat.wav 0.18
cargo test --offline --manifest-path core/Cargo.toml --test drum_asset_tests
```

Rendering uses `scsynth -N` (non-realtime), so no audio device, live server or UDP connection is needed. `render_nrt` builds the NRT score with the same Rust scheduler and OSC encoder as live playback. The SynthDef load and the sound share logical time 0, so no loading silence precedes the sample.

The recipes in `recipes/*.ndjson` are fixed ordinary scores (BPM 60, so one beat is one second), based on the current presets at velocity ≈0.6 (amp 0.12) and BPM 120 drum lengths. Editing the composition presets later does not change these recipes.

- The kick is deterministic and matches the old Python script's output byte for byte. The committed `kick.wav` was rendered before a SynthDef regeneration and differs by 1 LSB in one frame.
- Snare and hat use noise, so each regeneration differs (over 10 runs: peak up to 1.6 dB, RMS under 0.5 dB).
- NRT block boundaries can make a file a few ms longer than requested.

## Verify

`mise run check-samples` (`core/tests/drum_asset_tests.rs`) checks the format, length against the request, non-silence, peak headroom, sound within the first 10 ms and quiet in the last 10 ms. It does not depend on exact file hashes. `mise run audio-check` also renders the recipes and requires the kick within 1 LSB of the committed file and the snare/hat to match in length with RMS within ±1 dB. Whether the sounds suit your music still needs listening.

## Adding third-party samples

Keep the original license and record each file in [ATTRIBUTION.md](ATTRIBUTION.md). Use only CC0 or CC BY sounds (for example from Freesound); avoid NonCommercial ones.
