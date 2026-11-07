# Spinel Sonic Lab

[English](README.md)

最小構成のコマンドライン音楽環境です。Spinel互換のRubyでベースやドラムのパターンを書き、ネイティブのコンポーザーにコンパイルして、SuperColliderの `scsynth` でライブ演奏します。

```
Rubyの作曲 ──Spinel──▶ ネイティブcomposer ──NDJSON──▶ Rust CLI ──OSC──▶ scsynth
 (composer/)                                          (core/)        (synthdefs/)
```

## 技術スタック

| レイヤー | 技術 |
|---|---|
| 作曲 | Ruby(Spinelが対応するサブセット)を [Spinel](https://github.com/matz/spinel) で事前コンパイル |
| 実行・スケジューラ | Rust(edition 2024)、依存は `serde_json` のみ |
| 通信形式 | NDJSONスコアプロトコル、UDP上のOSC |
| 音声エンジン | [SuperCollider](https://supercollider.github.io/) の `scsynth`(SynthDefは `sclang` で記述) |
| 開発ツール | [mise](https://mise.jdx.dev/) タスク、Git LFS(WAV)、rustfmt/Clippy、RuboCop |
| 対応環境 | Linux(Omarchyで開発)、macOS(動作確認済み) |

## 特徴

- **Rubyで作曲、ネイティブで実行。** 音名、コード、アルペジオ、16ステップ、複数小節ループ、名前付きトラック、音色プリセットを持つ小さなDSLを [Spinel](https://github.com/matz/spinel) で事前コンパイルします。演奏時にRubyランタイムは不要です。
- **テキストのプロトコル。** composerは[NDJSONイベント](protocol/README.ja.md)を出力します。音楽イベントにOSCパケットやscsynthのノードIDは含みません。
- **正確なタイミング。** Rustのスケジューラは、同時に鳴る音を1つのOSC `#bundle` にまとめ、固定200msの先行タイムタグを付けて送ります。
- **ライブリロード。** `--watch` はスコアをループ再生し、有効な編集を次の4拍小節境界で切り替えます。壊れた編集は警告のみで、再生中のスコアを維持します。
- **サンプラーとミキサー。** モノラル/ステレオWAVを原音程で再生し、トラックごとのチャンネルストリップ(ゲイン、パン、フェーダー、コンプ、ベルEQ、リバーブへのAux送り)を使えます。
- **オフライン検証。** `--dry-run` は音を出さずに発音予定を表示し、`mise run audio-check` は `scsynth -N` で描画して測定します。

対象外: GUI、MIDI入力、プラグイン、ネットワーク演奏、独自DSP。

## クイックスタート

必要なもの: Rust(1.96以降)、Cコンパイラ、`make`、Ruby、Git LFS(Spinelのビルド用)、SuperCollider 3.14(`sclang`、`scsynth`)。詳細は[Requirements](docs/usage.md#requirements)を参照してください(英語)。

```sh
git lfs install                   # マシンごとに1回。WAV素材はGit LFSで管理しています
git clone https://github.com/tackaaaada/spinel-sonic-lab.git
cd spinel-sonic-lab
git lfs pull                      # WAVがLFSポインタのままのときだけ必要

# Spinelをこのリポジトリの隣にビルド(docs/usage.md#build-spinel)
export SPINEL_BIN=../spinel/bin/spinel

cargo build --manifest-path core/Cargo.toml
for f in analog_bass sampler channel; do sclang synthdefs/$f.scd; done   # synthdefs/*.scsyndef を生成
```

macOSでは `sclang` と `scsynth` がアプリ内(`/Applications/SuperCollider.app/Contents/MacOS/sclang`、`.../Resources/scsynth`)にあるため、`PATH` に追加してください。[プラットフォーム別の注意](docs/usage.md#platform-notes)(英語)も参照してください。

ターミナル1 — 音声サーバーを起動し、`SuperCollider 3 server ready` を待ちます:

```sh
scsynth -u 57110
```

ターミナル2 — 作曲ファイルをコンパイルしてプレーヤーへ渡します:

```sh
"$SPINEL_BIN" composer/bin/emit_bass.rb -o composer/bin/emit_bass
composer/bin/emit_bass | core/target/debug/spinel-sonic-lab -
```

音声デバイスがなければ、発音予定だけ確認できます:

```sh
core/target/debug/spinel-sonic-lab --dry-run composer/examples/bass.ndjson
```

[mise](https://mise.jdx.dev/)を使う場合、`mise tasks` で `mise run audio`、`mise run play-samples`、`mise run play-techno` などのショートカットを確認できます。

## DSLの例

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

`composer/bin/` に他の例があります: `emit_techno.rb`(133 BPMのテクノ試作)、`emit_console_strip.rb`(コンプ・EQ・リバーブ送りのストリップ)、`emit_samples.rb` など。

## WAVサンプラー

音声サーバーを `mise run audio` で起動後、別ターミナルで `mise run play-samples` を実行すると、キック・スネア・ハイハットと裏拍のSawベースを4小節(約8秒)演奏します。素材は `samples/*.wav` で、なければ `mise run samples` で生成します。`mise run play-techno` は4小節のテクノ試作を演奏します。詳細は[docs/usage.md](docs/usage.md#wav-sampler)(英語)と[samples/README.md](samples/README.ja.md)を参照してください。

## ドキュメント

詳細リファレンスは英語です。

| 内容 | 場所 |
|---|---|
| セットアップ、DSL、演奏例、トラブルシューティング | [docs/usage.md](docs/usage.md) |
| NDJSONイベントの形式と検証 | [protocol/README.md](protocol/README.ja.md) |
| レイヤー構成とスケジューリングの制約 | [docs/architecture.md](docs/architecture.md) |
| コンソール風グリッドDSL | [docs/console-grid-dsl.md](docs/console-grid-dsl.md) |
| 出力レベルと試聴ファイル | [docs/audio-levels.md](docs/audio-levels.md) |
| ドラム素材と再生成手順 | [samples/README.md](samples/README.ja.md) |
| ロードマップ | [docs/roadmap.md](docs/roadmap.md) |
| Omarchyでの開発・音が出ない場合 | [docs/development-omarchy.md](docs/development-omarchy.md) |

## 開発

```sh
cargo test --offline --manifest-path core/Cargo.toml   # Rustのテスト(SuperCollider不要)
SPINEL_BIN=/path/to/spinel sh tests/check_spinel.sh    # 全作曲ファイルをSpinelでコンパイル
mise run audio-check                                   # オフラインの音声測定(scsynthが必要)
```

`git config core.hooksPath .githooks` でプロジェクトのフックを有効にします。`pre-push` は静的チェックを実行し、Git LFSのオブジェクトをアップロードします。OSCのやり取りが成功しても、人が音を聞いた証拠にはなりません。試聴確認はテスト結果と分けて扱います。

## ライセンス

ソースコード、スコア、ドキュメントは[MITライセンス](LICENSE)です。このプロジェクトで作成した音声(`samples/` と `docs/` の試聴ファイル)は[CC BY 4.0](LICENSE-AUDIO)です。第三者のサンプルを追加する場合は元のライセンスを維持し、[samples/ATTRIBUTION.md](samples/ATTRIBUTION.md)に記載します。SpinelとSuperColliderは別プロジェクトで、それぞれのライセンスに従い、このリポジトリには含みません。
