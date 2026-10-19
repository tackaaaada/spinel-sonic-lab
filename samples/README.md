# SuperCollider製ドラム素材

既存の `synthdefs/analog_bass.scd` のドラム合成をSuperCollider自身で描画したWAV。
外部からダウンロードした音源は使用していない。
キックはSineとピッチ下降、スネアは音程成分とNoise、ハイハットはハイパスを通したNoise。
現在の合成ドラムの代わりに、サンプラーで原音程・通常速度で再生するための素材。
音声サーバー起動後に `mise run play-samples` でRustのサンプラー経由で演奏できる。

| 素材 | 形式 | 実際の長さ | Peak | RMS |
|---|---|---:|---:|---:|
| [kick.wav](kick.wav) | 48kHz / stereo / PCM16 | 0.3507秒 | −7.66 dBFS | −16.64 dBFS |
| [snare.wav](snare.wav) | 同上 | 0.3013秒 | −8.80 dBFS | −24.25 dBFS |
| [hat.wav](hat.wav) | 同上 | 0.1813秒 | −10.53 dBFS | −26.86 dBFS |

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
python3 scripts/render_drum_samples.py
python3 tests/check_samples.py
```

コンパイル済みSynthDefが最新なら、後半の2コマンドだけでも生成・検証できる。
`scsynth -N` による非リアルタイム描画なので音声デバイス・起動中のライブサーバー・UDP接続は不要。
PythonはOSCスコアの作成とscsynth起動のみを担当し、波形合成はしない。
SynthDefロードの完了メッセージで論理時刻0に音を開始し、ロード用無音を素材の先頭へ入れない。
一時OSCスコアは一時ディレクトリ内に作成して自動削除する。

レシピは `scripts/render_drum_samples.py` の `RECIPES` に固定している。
現行プリセットを基に、velocity 0.6相当（amp 0.12）、BPM 120時のドラム長を用いた。
既存の作曲プリセットを後から編集しても、この素材のレシピは自動では変わらない。
Noiseを使うため再生成ごとの波形・測定値は多少変化し、バイト単位の一致は保証しない。
NRTのブロック境界によりファイル長は指定より最大数ms長くなる。

## 検証

`python3 tests/check_samples.py` は形式、指定時間との誤差、非無音、ピーク余裕、
先頭10msの発音、末尾10msの静けさを確認する。厳密なファイルハッシュには依存しない。
実音を聴いて音色が用途に合うかは別途確認する。
