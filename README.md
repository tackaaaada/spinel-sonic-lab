# Spinel Sonic Lab

[日本語](README.ja.md)

> **Status: in development (v0.x).** The DSL, the NDJSON protocol and the CLI options may change without notice. See the [roadmap](docs/roadmap.md).

A minimal command-line music environment. Write a bass line or a drum pattern in Spinel-compatible Ruby, compile it to a native composer, and play it live through SuperCollider's `scsynth`.

```
Ruby composition ──Spinel──▶ native composer ──NDJSON──▶ Rust CLI ──OSC──▶ scsynth
  (composer/)                                             (core/)        (synthdefs/)
```

## Tech stack

| Layer | Technology |
|---|---|
| Composition | Ruby (Spinel-supported subset), compiled ahead of time with [Spinel](https://github.com/matz/spinel) |
| Runtime and scheduler | Rust (edition 2024), `serde_json` only |
| Wire format | NDJSON score protocol, OSC over UDP |
| Audio engine | [SuperCollider](https://supercollider.github.io/) `scsynth` with SynthDefs written in `sclang` |
| Tooling | [mise](https://mise.jdx.dev/) tasks, Git LFS (WAV files), rustfmt/Clippy, RuboCop |
| Platforms | Linux (developed on Omarchy) and macOS (verified) |

## Features

- **Compose in Ruby, run natively.** Compositions use a small DSL (notes, chords, arpeggios, 16-step patterns, multi-bar loops, named tracks and tone presets) and are compiled ahead of time by [Spinel](https://github.com/matz/spinel); no Ruby runtime is needed to play.
- **Plain-text protocol.** The composer emits [NDJSON events](protocol/README.md). Music events never contain OSC packets or scsynth node IDs.
- **Tight timing.** The Rust scheduler sends simultaneous notes as one OSC `#bundle` with explicit future timetags (fixed 200 ms lead) to avoid jitter.
- **Live reload.** `--watch` loops a score and switches to a valid edit at the next 4-beat bar boundary; a broken edit is logged and the current score keeps playing.
- **Sampler and mixer.** Whole-file mono/stereo WAV playback, plus per-track channel strips (gain, pan, fader, compressor, bell EQ, reverb aux send).
- **Offline checks.** `--dry-run` prints the schedule without audio, and `mise run audio-check` renders scores with `scsynth -N` and measures the result.

Out of scope: GUI, MIDI input, plugins, network performance and custom DSP.

## Quick start

Requirements: Rust (1.96 or newer), a C compiler, `make`, Ruby and Git LFS (to build Spinel), and SuperCollider 3.14 (`sclang`, `scsynth`). See [Requirements](docs/usage.md#requirements) for details.

```sh
git lfs install                   # once per machine; WAV samples are stored with Git LFS
git clone https://github.com/tackaaaada/spinel-sonic-lab.git
cd spinel-sonic-lab
git lfs pull                      # only needed if the WAVs are still LFS pointers

# Build Spinel next to this repository (see docs/usage.md#build-spinel)
export SPINEL_BIN=../spinel/bin/spinel

cargo build --manifest-path core/Cargo.toml
for f in analog_bass sampler channel; do sclang synthdefs/$f.scd; done   # writes synthdefs/*.scsyndef
```

On macOS, `sclang` and `scsynth` live inside the app bundle (`/Applications/SuperCollider.app/Contents/MacOS/sclang` and `.../Resources/scsynth`); add them to `PATH`. See [platform notes](docs/usage.md#platform-notes).

Terminal 1 — start the audio server and wait for `SuperCollider 3 server ready`:

```sh
scsynth -u 57110
```

Terminal 2 — compile a composition and pipe it to the player:

```sh
"$SPINEL_BIN" composer/bin/emit_bass.rb -o composer/bin/emit_bass
composer/bin/emit_bass | core/target/debug/spinel-sonic-lab -
```

No audio device? Inspect the schedule instead:

```sh
core/target/debug/spinel-sonic-lab --dry-run composer/examples/bass.ndjson
```

If you use [mise](https://mise.jdx.dev/), `mise tasks` lists shortcuts such as `mise run audio`, `mise run play-samples` and `mise run play-techno`.

## A taste of the DSL

```ruby
require_relative "../lib/track_score"

score = TrackScore.new(120)
score.sample_file(:kick, "../../samples/kick.wav")

score.track :kick do
  loop bars: 4 do
    use_sample :kick
    bar grid: 16 do
      hits 1, 5, 9, 13
    end
  end
end

score.track :bass do
  use_synth :analog_bass
  fader db: -6
  loop bars: 4 do
    bar grid: 16 do
      hits 3, 7, note: "C2", duration_beats: 0.5, velocity: 0.7
    end
  end
end
score.emit
```

More examples are in `composer/bin/`: `emit_techno.rb` (a 133 BPM sketch), `emit_console_strip.rb` (channel strips with compressor, EQ and reverb send), `emit_samples.rb` and others.

## Documentation

| Topic | Where |
|---|---|
| Setup, DSL reference, playback recipes, troubleshooting | [docs/usage.md](docs/usage.md) |
| NDJSON event format and validation | [protocol/README.md](protocol/README.md) |
| Layers and scheduling limits | [docs/architecture.md](docs/architecture.md) |
| Console-style grid DSL | [docs/console-grid-dsl.md](docs/console-grid-dsl.md) |
| Output levels and previews | [docs/audio-levels.md](docs/audio-levels.md) |
| Drum samples and regeneration | [samples/README.md](samples/README.md) |
| Roadmap | [docs/roadmap.md](docs/roadmap.md) |

## Development

```sh
cargo test --offline --manifest-path core/Cargo.toml   # Rust tests (no SuperCollider needed)
SPINEL_BIN=/path/to/spinel sh tests/check_spinel.sh    # compiles every composition with Spinel
mise run audio-check                                   # offline audio measurements (needs scsynth)
```

Enable the project hooks with `git config core.hooksPath .githooks`; the `pre-push` hook runs static checks and uploads Git LFS objects. Note that a successful OSC exchange does not prove that anyone heard the sound; listening checks are reported separately from tests.

## License

Source code, scores and documentation are under the [MIT License](LICENSE). Audio created for this project (`samples/` and the previews in `docs/`) is under [CC BY 4.0](LICENSE-AUDIO). Third-party samples, if any, keep their own license and are listed in [samples/ATTRIBUTION.md](samples/ATTRIBUTION.md). Spinel and SuperCollider are separate projects with their own licenses and are not distributed here.
