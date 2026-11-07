# SuperCollider製ドラム素材

[English](README.md)

既存の `synthdefs/analog_bass.scd` のドラム合成をSuperCollider自身で描画したWAV。
外部からダウンロードした音源は使用していない。
キックはSineとピッチ下降、スネアは音程成分とNoise、ハイハットはハイパスを通したNoise。
現在の合成ドラムの代わりに、サンプラーで原音程・通常速度で再生するための素材。
音声サーバー起動後に `mise run play-samples` でRustのサンプラー経由で演奏できる。

| 素材 | 形式 | 実際の長さ | Peak | RMS |
|---|---|---:|---:|---:|
| [kick.wav](kick.wav) | 48kHz / stereo / PCM16 | 0.3507秒 | −7.66 dBFS | −16.64 dBFS |
| [snare.wav](snare.wav) | 同上 | 0.3013秒 | −8.10 dBFS | −24.13 dBFS |
| [hat.wav](hat.wav) | 同上 | 0.1813秒 | −10.45 dBFS | −26.84 dBFS |

RMSは末尾の無音を含むファイル全体で測定した値。素材間の聴感音量一致を示すものではない。
単発でクリップしないことを確認しているが、複数音の重ね合わせには別途ミックスの確認が必要。

## 再生成

プロジェクトのルートで:

```sh
mise run samples
```

個別のコマンド:

```sh
QT_QPA_PLATFORM=offscreen QT_QPA_PLATFORMTHEME=generic sclang synthdefs/analog_bass.scd
cargo build --offline --manifest-path core/Cargo.toml --bin render_nrt
core/target/debug/render_nrt samples/recipes/kick.ndjson samples/kick.wav 0.35
core/target/debug/render_nrt samples/recipes/snare.ndjson samples/snare.wav 0.30
core/target/debug/render_nrt samples/recipes/hat.ndjson samples/hat.wav 0.18
cargo test --offline --manifest-path core/Cargo.toml --test drum_asset_tests
```

`scsynth -N` による非リアルタイム描画なので音声デバイス・起動中のライブサーバー・UDP接続は不要。
`render_nrt` はライブ再生と同じRustのスケジューラとOSCエンコーダでNRTスコアを作り、scsynthを起動する。
SynthDefのロードと音は同じ論理時刻0に置き、ロード用無音を素材の先頭へ入れない。
一時OSCスコアは出力WAVの隣に作成し、描画後に削除する。

レシピは `samples/recipes/*.ndjson` に通常のスコアとして固定している（BPM 60なので1拍=1秒）。
現行プリセットを基に、velocity 0.6相当（amp 0.12）、BPM 120時のドラム長を用いた。
既存の作曲プリセットを後から編集しても、この素材のレシピは自動では変わらない。
キックは決定的で、現在のSynthDefでは旧Pythonスクリプトの出力とバイト単位で一致する。
コミット済み `kick.wav` はSynthDef再生成前の描画で、1フレームだけ1 LSB異なる。
Noiseを使うスネアとハイハットは再生成ごとに波形・測定値が変化する（10回でPeak最大1.6 dB、RMS 0.5 dB未満）。
NRTのブロック境界によりファイル長は指定より最大数ms長くなる。

## 検証

`mise run check-samples`（`core/tests/drum_asset_tests.rs`）は形式、指定時間との誤差、非無音、ピーク余裕、
先頭10msの発音、末尾10msの静けさを確認する。厳密なファイルハッシュには依存しない。
`mise run audio-check` はレシピを描画し、キックがコミット済み素材と1 LSB以内、
スネアとハイハットが同じ長さでRMS ±1 dB以内であることも確認する。
実音を聴いて音色が用途に合うかは別途確認する。
