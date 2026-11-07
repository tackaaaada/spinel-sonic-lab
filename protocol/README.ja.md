# NDJSONスコアプロトコル(日本語概要)

[English](README.md) — 仕様の正本は英語版です。ここでは要点のみ記します。

UTF-8テキストで1行1JSONオブジェクト。空行は無視します。先頭は必ず `tempo` で、続いて `tone` / `sample` / `channel` / `aux_return` の宣言、その後に1つ以上の再生イベント(`note` または `sample_note`)を置きます。再生前にスコア全体を検証し、未知のフィールドやイベント種別はエラーです。

```json
{"type":"tempo","bpm":120}
{"type":"tone","instrument":"analog_bass","oscillator":"saw","cutoff_hz":1800.0,"attack_seconds":0.005,"release_seconds":0.060,"gain":2.600}
{"type":"note","instrument":"analog_bass","note":36,"beat":0.0,"duration_beats":0.5,"velocity":0.8}
```

| イベント | 要点 |
|---|---|
| `tempo` | `bpm` は正の有限数。任意の `bars`(1〜1024)でループ長を小節数で指定(無音小節も含む)。 |
| `note` | `note` はMIDI 0〜127(C2=36、C3=48)、`beat` は0以上、`duration_beats` は正、`velocity` は0〜1。1拍=四分音符。同じ拍は入力順を保持。 |
| `tone` | 再生イベントより前。`oscillator`(`saw`/`sine`/`pulse`/`noise`、省略時 `saw`)、`cutoff_hz` 40〜18000、`attack`/`decay`/`release` 各0〜5秒、`sustain_level` 0〜1、`lfo_rate_hz` 0.1〜20 と `lfo_depth` 0〜1(対で指定)、`gain` 0〜4(省略時1.0)。`tone` が無ければv0.1相当の音(gain 2.8)。 |
| `track` | `note`/`tone`/`sample_note` の任意フィールド。1〜32文字のASCII英数字・`_`・`-`。省略時は `main`。`--mute-track NAME` はスコア全体の検証後にそのトラックを除外。OSCには送らない。 |
| `channel` | トラックごとに最大1つ。`ha_gain_db`・`fader_db`(−96〜+24 dB)、`pan`(−1〜+1)は必須。任意で `compressor`、`eq_bell`、`aux_send`。 |
| `aux_return` | 一意な `name`、リバーブ `room`/`damp`/`wet`(0〜1)、`fader_db`。 |
| `sample` | 一意な名前とWAVパスの宣言。再生イベントより前。相対パスは元の作曲ファイルのディレクトリ基準。 |
| `sample_note` | 宣言済みの `sample`、`beat`、`velocity`(0〜1)。WAV全体を原音程・通常速度で一度だけ再生。 |

信号経路(`channel` あり): ソース → H/A → コンプ → ベルEQ → Aux取り出し → パン → フェーダー。

対応WAVはPCM16、1〜2ch、8000〜192000 Hz、1素材30秒・32MiB以内、スコア全体で64MiB以内。すべての宣言を再生前に検証します。

プロトコルは音楽と楽器の設定のみを持ち、OSCパケットやSuperColliderのノードID、バッファIDは含みません。それらはRustのトランスポート層が扱います。
