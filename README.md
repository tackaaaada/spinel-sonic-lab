# Spinel Sonic Lab

A minimal CLI music environment: write a bass line in Spinel-compatible Ruby, compile it to a native composer, pass its NDJSON events to Rust, and play an `analog_bass` SynthDef through SuperCollider's `scsynth`.

The v0.1 four-note example remains available. Spinel can set the filter cutoff, ADSR envelope, filter LFO, output gain, and oscillator (`saw`, `sine`, `pulse`, or `noise`) for a score or per named track. The composer also offers tone and drum presets (`soft`, `bright`, `wobble`, `kick`, `snare`, `hat`), named pitches, chords, arpeggios, 16-step patterns, and multi-bar pattern loops. The Rust CLI groups simultaneous notes into OSC `#bundle` packets and can loop a score file and switch to valid edits at the next bar boundary (`--watch`). There is no GUI, external plugin, MIDI input, or custom DSP.

## Development environment

- macOS with an available CoreAudio output device
- Rust and Cargo (tested with Rust 1.96)
- A C compiler, `make`, `git`, Git LFS, and Ruby for building Spinel
- SuperCollider with `sclang` and `scsynth` (tested with 3.14.1)

Run commands below from this repository's root. If SuperCollider was installed as a macOS app, its binaries are usually at `/Applications/SuperCollider.app/Contents/MacOS/sclang` and `/Applications/SuperCollider.app/Contents/Resources/scsynth`. Add those locations to `PATH` or use the full paths.

The WAV samples and audio previews are stored with Git LFS. After cloning, run
`git lfs pull` before running sampler tests or playing the sampler example. A
file beginning with `version https://git-lfs.github.com/spec/v1` is an LFS
pointer, not playable WAV audio.

Activate the project hooks with `git config core.hooksPath .githooks`. That
setting replaces Git LFS's own hooks, so the project's `pre-push` runs the
static check and then `git lfs pre-push`; without it, a pushed WAV would reach
GitHub as a pointer with no audio. Git LFS writes its own hooks into
`.githooks` only when `pre-push` is missing there, such as after checking out a
commit from before the hooks existed; delete those untracked copies.
`bash tests/check_git_hooks.sh` checks this with a local remote.

## Build Spinel

Follow the [official Spinel build instructions](https://github.com/matz/spinel). Spinel is a source-built AOT compiler, not a Gem:

```sh
git clone https://github.com/matz/spinel.git ../spinel
cd ../spinel
make deps
make
./bin/spinel --version
cd ../spinel-sonic-lab
export SPINEL_BIN=../spinel/bin/spinel
```

If `make` chooses macOS's old system Ruby and reports that `prism` cannot be loaded, put a current Ruby installation first on `PATH` before running `make` again. The composer itself needs no Ruby runtime after Spinel compilation.

In this workspace Spinel has already been built at `../.tools/spinel/bin/spinel`; use `export SPINEL_BIN=../.tools/spinel/bin/spinel` instead of cloning it again.

## Build Rust and the SynthDef

```sh
cargo build --manifest-path core/Cargo.toml
sclang synthdefs/analog_bass.scd
```

The `sclang` command creates `synthdefs/analog_bass.scsyndef`. The generated file is ignored by Git. The definition has selectable Saw, Sine, Pulse, and WhiteNoise sources, a low-pass filter with optional LFO, ADSR envelope, stereo output, and automatic node release. Its controls are `freq` (Hz), `amp` (0–0.2 in this CLI), `gain` (0–4 from the score), `dur` (seconds), `cutoff` (Hz), `attack`, `decay`, and `release` (seconds), `sustain` (0–1), `lfoRate` (Hz), `lfoDepth` (0–1), `osc` (0=Saw, 1=Sine, 2=Pulse, 3=WhiteNoise), and `out` (default 0).

## Play the example

In terminal 1, start a standalone local audio server:

```sh
scsynth -u 57110
```

Wait for `SuperCollider 3 server ready`. In terminal 2, compile the Spinel composition and pipe it into Rust:

```sh
"$SPINEL_BIN" composer/bin/emit_bass.rb -o composer/bin/emit_bass
composer/bin/emit_bass | core/target/debug/spinel-sonic-lab -
```

For the brighter tone, compile and play the second composition:

```sh
"$SPINEL_BIN" composer/bin/emit_bright_bass.rb -o composer/bin/emit_bright_bass
composer/bin/emit_bright_bass | core/target/debug/spinel-sonic-lab -
```

To choose Sine or Pulse, compile the waveform example once and pass the name at runtime:

```sh
"$SPINEL_BIN" composer/bin/emit_wave_bass.rb -o composer/bin/emit_wave_bass
composer/bin/emit_wave_bass sine | core/target/debug/spinel-sonic-lab -
composer/bin/emit_wave_bass pulse | core/target/debug/spinel-sonic-lab -
```

To compare a decaying bass envelope with the brighter bass, compile and play the ADSR example:

```sh
"$SPINEL_BIN" composer/bin/emit_adsr_bass.rb -o composer/bin/emit_adsr_bass
composer/bin/emit_adsr_bass | core/target/debug/spinel-sonic-lab -
```

To hear the filter move during each note, compile and play the LFO example:

```sh
"$SPINEL_BIN" composer/bin/emit_lfo_bass.rb -o composer/bin/emit_lfo_bass
composer/bin/emit_lfo_bass | core/target/debug/spinel-sonic-lab -
```

To choose a named tone without editing its parameters, compile the preset example once. Pass `soft`, `bright`, or `wobble` when running it (`soft` is the default):

```sh
"$SPINEL_BIN" composer/bin/emit_preset_bass.rb -o composer/bin/emit_preset_bass
composer/bin/emit_preset_bass soft | core/target/debug/spinel-sonic-lab -
composer/bin/emit_preset_bass bright | core/target/debug/spinel-sonic-lab -
composer/bin/emit_preset_bass wobble | core/target/debug/spinel-sonic-lab -
```

To play two triads with a melody above them:

```sh
"$SPINEL_BIN" composer/bin/emit_melody_chords.rb -o composer/bin/emit_melody_chords
composer/bin/emit_melody_chords | core/target/debug/spinel-sonic-lab -
```

To play a one-bar 16-step bass rhythm:

```sh
"$SPINEL_BIN" composer/bin/emit_step_bass.rb -o composer/bin/emit_step_bass
composer/bin/emit_step_bass | core/target/debug/spinel-sonic-lab -
```

To play the same rhythm as two named tracks, and then hear only the bass track:

```sh
"$SPINEL_BIN" composer/bin/emit_tracks.rb -o composer/bin/emit_tracks
composer/bin/emit_tracks | core/target/debug/spinel-sonic-lab -
composer/bin/emit_tracks | core/target/debug/spinel-sonic-lab --mute-track counter -
```

`score.tone(cutoff_hz, attack_seconds, release_seconds, oscillator = "saw", gain = 1.0)` writes one tone event before the notes. It applies to every note in that score. The original composition omits it and uses the built-in bass gain of 2.8. The original sound comparison examples use the same four pitches and BPM.
`score.tone_adsr(cutoff_hz, attack_seconds, decay_seconds, sustain_level, release_seconds, oscillator = "saw", gain = 1.0)` adds the decay and sustain controls. Older tone events without gain default to 1.0. The envelope's total length is the note duration; long stages are shortened to fit.
`score.tone_lfo(cutoff_hz, attack_seconds, decay_seconds, sustain_level, release_seconds, lfo_rate_hz, lfo_depth, oscillator = "saw", gain = 1.0)` adds periodic filter movement. `lfo_rate_hz` is 0.1–20 and `lfo_depth` is 0–1. The LFO example keeps the same BPM, note positions, and note lengths as the earlier four-note examples. Its 4 Hz modulation completes one cycle per 0.25-second note.
`score.preset(name)` and `score.track_preset(track, name)` select one of the built-in tone settings in Spinel (`soft`, `bright`, `wobble`, `kick`, `snare`, or `hat`). `soft` uses Sine and a gentle envelope; `bright` is the bright Saw setting; `wobble` is the LFO setting; `kick` uses a low Sine envelope; `snare` and `hat` use filtered WhiteNoise envelopes. Presets emit ordinary `tone` events and do not add a new Rust or OSC protocol type. An unknown name stops the composer with an error.

`score.note("C#4", beat, duration_beats, velocity)` accepts uppercase A–G, an optional `#`, and a single-digit octave. The result must fit MIDI 0–127; C2 is 36, C#4 is 61, and G9 is 127. `score.midi_note(67, ...)` accepts a MIDI integer directly. `score.chord(["C3", "E3", "G3"], beat, duration_beats, velocity)` emits at least two notes at the same beat, in the given order. `score.arpeggio(["E4", "G4", "C5"], start_beat, step_beats, duration_beats, velocity)` (and `track_arpeggio`) validates all note names and positive `step_beats` first, then emits sequential notes spaced by `step_beats`. Note event beat and duration values are written to four decimal places; velocity remains at three decimal places.

`score.steps("C2", "x---x---x---x---", 0.0, 0.25, 0.20, 0.55)` defines exactly 16 slots: `x` plays the named note and `-` rests. `score.loop_steps("C2", pattern, bars, step_beats, duration_beats, velocity)` (and `track_loop_steps`) repeats a 16-step pattern across `bars` consecutive bars. `score.drum_steps("kick", pattern, start_beat, velocity = 0.75)` and `score.drum_loop_steps("kick", pattern, bars, velocity = 0.75)` emit 16th-note drum patterns on the `"kick"`, `"snare"`, or `"hat"` track. Invalid note names, drum names, pattern lengths, symbols, bar counts, and nonpositive spacing stop composition before that pattern emits notes.

The four-bar track DSL is in `composer/bin/emit_four_bar_kick.rb`. It declares the WAV asset first, then groups the selected sample and rhythm inside the kick track:

```ruby
require_relative "../lib/track_score"

score = TrackScore.new(120)
score.sample_file(:bd_303, "../../samples/kick.wav")
score.track :kick do
  loop bars: 4 do
    use_sample :bd_303
    # bar { hits 1, 2, 3, 4 }
    bar { hits 1, 2, 3, 3.75 }
  end
end
score.emit
```

`hits` uses one-based beat positions within a 4/4 bar. Positions must lie from 1 through 4 on a sixteenth-note grid; `3.75` becomes offset 2.75 beats from the bar start. Active `bar` lines repeat to fill four bars. The sample name `bd_303` is an alias for the current `samples/kick.wav`, not a claim of TB-303 or TR-series sound reproduction. This first version accepts `loop bars: 4`; eight-bar composition is not implemented yet. The generated tempo event includes `"bars":4`, so a silent final bar still contributes to the loop length. To check the score without audio, run `core/target/debug/spinel-sonic-lab --dry-run --watch --bars 4 composer/bin/emit_four_bar_kick.rb` after building the Rust CLI.

For sound, start `mise run audio` in one terminal, then run `mise run play-four-bar-kick` in another.

For per-bar rhythm and melody grids, see `composer/bin/emit_grid_demo.rb`:

```ruby
score.track :kick do
  loop bars: 4 do
    use_sample :kick
    bar grid: 64 do
      hits 1, 17, 33, 49, 64
    end
  end
end

score.track :bass do
  loop bars: 4 do
    use_synth :analog_bass
    bar grid: 16 do
      hits 1, 5, note: "C3"
      hits 9, 13, note: "E3"
    end
  end
end
```

Each `bar` may set `grid: 4`, `8`, `16`, `32`, or `64`. In a gridded bar, `hits` takes integer step numbers from 1 through the chosen grid size. Sample hits use `use_sample`; synth hits specify a note name and last one grid step by default. A `hits` call may set `velocity:` from 0 to 1. Synth hits may also set `duration_beats:` to a positive quarter-note beat length; the length stays the same if the Grid resolution changes. For example, `hits 1, 5, note: "C3", duration_beats: 0.5, velocity: 0.7` makes both notes half a beat long. Sample hits play the whole WAV and reject `duration_beats:`. Several `hits` calls in one bar can form a melody with different lengths and strengths. A bar without `grid:` retains the earlier one-based beat-position notation, including fractional positions such as `3.75`. Four decimal places keep 64th-note times exact in the generated score. Run `core/target/debug/spinel-sonic-lab --dry-run --watch --bars 4 composer/bin/emit_grid_demo.rb` to inspect the generated timing without audio.

Each track can be written like a console channel module, top to bottom: source → `ha` → `dynamics` → `eq` → `aux` → `pan` → `fader`, then the Grid `loop` (see [console-grid-dsl.md](docs/console-grid-dsl.md)). Modules may be omitted but not reordered or repeated. H/A input trim, Pan, and Fader alone work with Grid hits:

```ruby
score.track :bass do
  use_synth :analog_bass
  ha gain_db: 0
  pan position: 1
  fader db: -6
  loop bars: 4 do
    bar grid: 16 do
      hits 1, note: "C3", duration_beats: 0.5, velocity: 0.5
    end
  end
end
```

`composer/bin/emit_console_grid.rb` pairs a left-panned sample kick with a right-panned synth bass. After starting `mise run audio` in another terminal, run `mise run play-console-grid`.

The full strip adds a compressor, a bell EQ, and an Aux send to a declared reverb return:

```ruby
score.aux_return :room do
  reverb room: 0.6, damp: 0.4, wet: 0.5
  fader db: -6
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
  pan position: -0.2
  fader db: -6
  loop bars: 4 do
    bar grid: 16 do
      hits 1, 4, note: "C2", duration_beats: 0.5, velocity: 0.7
    end
  end
end
```

Such a track plays through a persistent stereo channel strip that processes the sum of its notes; the Aux send is taken after EQ and before Pan/Fader. `composer/bin/emit_console_strip.rb` combines a compressed kick, a hat with a send, and a processed bass. Run `mise run synthdef` once (it now also builds `channel_strip` and `aux_return`), then `mise run play-console-strip`, or `mise run watch-console-strip` to edit strip settings live; changes apply at the next bar. `--dry-run` marks routed events with `-> strip TRACK`. Neither console example has had a listening check yet.

`score.track_steps("counter", "E2", pattern, 0.0, 0.25, 0.20, 0.45)`, `score.track_note("bass", "C2", 0.0, 0.5, 0.7)`, `score.track_chord("chords", ["C3", "E3", "G3"], 0.0, 1.0, 0.3)`, and `score.track_arpeggio("melody", ["E4", "G4", "C5"], 0.0, 0.5, 0.35, 0.4)` write notes with a track name. `score.track_preset("chords", "soft")` (or `track_tone` / `track_tone_adsr` / `track_tone_lfo`) assigns an independent tone to that track before the notes; tracks without a dedicated tone use the score's default tone. Names use 1–32 ASCII letters, digits, `_`, or `-`; ordinary `steps`, `note`, `chord`, `arpeggio`, and older note events belong to the implicit `main` track. `--mute-track counter` removes that track after Rust validates the complete score, then schedules the remaining notes.

Rust loads the generated SynthDef with `/d_load`, waits for completion, groups simultaneous notes at the same beat into a single OSC `#bundle` packet containing `/s_new` messages, and checks for server failure replies. The default OSC destination is `127.0.0.1:57110`; `--host` accepts loopback addresses only, and `--port` changes the port. `--synthdef PATH` selects the compiled definition. `--watch FILE` accepts either an `.ndjson` score or a Spinel `.rb` composition, loops playback in 4-beat bars, compiles and stages valid file changes on a background worker thread, and switches at the next bar boundary (`--bars COUNT` stops after `COUNT` bars). Invalid edits log a warning and keep the current score playing. For offline inspection without `scsynth`:

```sh
core/target/debug/spinel-sonic-lab --dry-run composer/examples/bass.ndjson
core/target/debug/spinel-sonic-lab --dry-run --watch --bars 2 composer/bin/emit_bass.rb
```

The example uses C2=36, E2=40, G2=43, C3=48. See [the protocol](protocol/README.md) for event fields and validation.

Compare the [original Saw preview](docs/bass-preview.wav), [brighter Saw preview](docs/bright-bass-preview.wav), [Sine preview](docs/sine-bass-preview.wav), [Pulse preview](docs/pulse-bass-preview.wav), and [ADSR preview](docs/adsr-bass-preview.wav). They were regenerated with calibrated gain using SuperCollider's non-realtime engine and contain four non-silent notes. The ADSR preview uses the same Saw oscillator and pitches as the brighter Saw preview; its decay to a 0.25 sustain level intentionally makes the later part of each note quieter.

Compare the [static filter preview](docs/static-filter-preview.wav) with the [LFO preview](docs/lfo-bass-preview.wav). Their pitches, note positions, note lengths, and envelope are identical; only LFO depth differs (0 versus 0.9). Both use the same BPM 120 rhythm as the earlier four-note examples.

Compare the three preset sounds: [soft](docs/soft-preset-preview.wav), [bright](docs/bright-bass-preview.wav), and [wobble](docs/lfo-bass-preview.wav). All use the same four-note BPM 120 rhythm. Their rendered RMS levels are within 0.3 dB of each other. Peak and RMS measurements for every preview are in [audio levels](docs/audio-levels.md). RMS is a comparison aid, not a substitute for listening; adjust `gain` in a composition if needed.

The [melody and chords preview](docs/melody-chords-preview.wav) contains ten note events, including four simultaneous notes at two points. Its measured peak is −9.7 dBFS. This example uses a lower per-note velocity and gain 3.0 to leave room for overlapping voices.

The [16-step bass preview](docs/step-bass-preview.wav) plays eight notes from the layered patterns. Its 2.4-second render has a measured peak of −15.1 dBFS. The rendering includes 0.05 seconds of silence at the start for SynthDef loading.

The named-track example has the same eight-note sound as that preview. Compare it with the [bass-only preview](docs/bass-track-preview.wav), which removes the `counter` track and keeps four C2 notes. The latter peaks at −15.1 dBFS and includes the same 0.05-second loading gap.

## Tests

```sh
cargo test --offline --manifest-path core/Cargo.toml
sh tests/check_spinel.sh
mise run audio-check   # needs scsynth and generated SynthDefs; no audio device
```

The Rust tests need no SuperCollider connection. Scheduling and bar-transition tests do not sleep; the background-worker test waits for an actual channel notification with a bounded timeout. `core/tests/live_tests.rs` covers bar-boundary changes, tempo edits, failed reloads, and Spinel recompilation. The Spinel test actually compiles and runs the compositions, including Sine, Pulse, all three preset choices, melody with chords, 16-step rhythm, and named tracks, then compares their NDJSON output to the checked-in examples. It checks sharp-note conversion, invalid pitch names, invalid step patterns, and invalid track names. Remove `--offline` if Cargo dependencies are not yet cached.

`mise run audio-check` renders NDJSON scores offline with `scsynth -N` through the same scheduler, channel graph, sample buffers and OSC encoders as live playback, then measures the WAVs: oscillator levels, pan law, compressor, bell EQ, pre-Fader Aux with reverb tail, and the drum recipes. These are signal checks, not a listening check. `mise run render -- INPUT OUTPUT.wav [SECONDS]` renders any `.ndjson` or Spinel `.rb` score to a WAV the same way, which is handy for listening without starting the audio server.

## Troubleshooting

- **`sclang` or `scsynth` not found:** use the full paths inside `SuperCollider.app` or add them to `PATH`.
- **No `.scsyndef` file:** run `sclang synthdefs/analog_bass.scd` from the project root and inspect its output for compilation errors.
- **SynthDef load times out:** confirm that standalone `scsynth -u 57110` is ready, that the CLI points to the same port, and that local UDP is available.
- **Server logs show `/s_new` errors:** inspect the `scsynth` terminal. Standalone scsynth uses root group 0; the CLI sends notes there.
- **No audible sound:** check the CoreAudio output device chosen by `scsynth`, system volume, and the server log. Start with a low volume. The CLI's successful exit confirms command acceptance, not what was heard at the speakers.
- **Rhythm rushes or stumbles, and the server prints `late 2.4…`:** scsynth's clock has drifted from the system clock, so it plays late bundles on arrival. This happens on long-running servers in VMs whose emulated audio device advances in coarse jumps (Omarchy in QEMU fixes PipeWire's quantum at 4096 frames). Restart `mise run audio` before listening, avoid changing the quantum while it runs, or use real hardware. An offline render (`mise run render -- INPUT OUT.wav`) is not affected. Buffer and node frees are stamped after their last sound, so a drifting server no longer prints `Buffer UGen: no buffer data`.
- **Spinel build cannot load `prism`:** use a current Ruby first on `PATH`, then rerun `make`. The build also needs its `make deps` step.

The layer boundaries and scheduling limits are described in [architecture](docs/architecture.md).


開発段階ごとの実装状況と残作業は[ロードマップ](docs/roadmap.md)を参照してください。

SuperColliderで生成したサンプラー用の[キック・スネア・ハイハット素材](samples/README.md)を用意しています。`mise run samples`で再生成、`mise run check-samples`でWAVを検証できます。原音程でのサンプラー再生は以下の手順で使えます。


## WAVサンプラー

音声サーバーを `mise run audio` で起動後、別ターミナルで:

```sh
mise run play-samples
```

生成済みキック・スネア・ハイハットと、裏拍に置いた短いSawベースを4小節（約8秒）演奏します。
初回は `samples/*.wav` が必要です。素材がなければ `mise run samples` で生成します。
`play-samples` はRustのビルドと合成音・サンプラーのSynthDef生成を行います。

```sh
# OSCを送らず、実際のWAV長と発音予定を確認
core/target/debug/spinel-sonic-lab --dry-run composer/bin/emit_samples.rb
# スネアだけミュート
core/target/debug/spinel-sonic-lab --mute-track snare composer/bin/emit_samples.rb
# 保存した有効な編集を小節境界で反映
mise run watch-samples
# 2小節で停止し、最後のサンプルが終わるまで待つ
core/target/debug/spinel-sonic-lab --watch --bars 2 composer/bin/emit_samples.rb
```

編集対象は `composer/bin/emit_samples.rb`。DSLの最小例:

```ruby
score = ScoreWriter.new
score.tempo(120)
score.sample_file("kick", "../../samples/kick.wav")
score.sample("kick", 0.0, 0.65)
score.track_sample("drums", "kick", 1.0, 0.65)
```

`sample_steps(track, name, pattern, start_beat, velocity = 1.0)` は
`x`/`-`の16ステップを0.25拍刻みに展開します。
音源宣言は全再生イベントより前へ記述します。パスは作曲ファイルからの相対指定です。
対応はPCM16のモノラル・ステレオWAV。全体を通常速度で一度再生し、
サンプルの音量は `velocity`（0–1）をそのまま掛けます。
詳細と上限は[プロトコル](protocol/README.md#whole-file-samples-backward-compatible-extension)を参照してください。


サンプラー例のキックとベースの分離を比較するには、
[変更前](docs/samples-mix-before.wav)と[調整後](docs/samples-mix-after.wav)を試聴できます。
調整後はキックを表拍に保ち、ベースを0.5拍後ろの裏拍へ移動。
ベースは3msアタックのSaw、0.4拍の長さで次のキック前に止まります。
これはサンプラー例の編曲・音色設定の変更で、スケジューラーの発音時刻補正ではありません。

## テクノ寄りの試聴スコア

`composer/bin/emit_techno.rb` は133 BPMの2小節を2回繰り返す4小節の試作です。毎拍のキック、2・4拍目のスネア、裏拍のハイハット、短いSawベースを使い、2小節目の終わりにハットとベースを少し変えます。既存のドラムWAVを使用するため、再生前にGit LFSの素材が取得されている必要があります。

音声サーバーを別ターミナルで起動した状態で:

```sh
mise run play-techno
```

音が聞こえない場合は[Omarchyの出力接続手順](docs/development-omarchy.md#コマンドは成功するが音が聞こえない場合)を確認してください。

2026-11-01の試聴では、このテクノ試作のキックとベースのバランスを利用者が現時点でOKと確認しました。
