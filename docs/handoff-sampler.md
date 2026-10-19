# WAVサンプラー引き継ぎ（2026-10-19）

## 実装した内容

- 音源宣言 `sample` と再生イベント `sample_note`。既存v0.1のnote/toneは互換維持。
- DSL: `sample_file`、`sample`、`track_sample`、16スロットの `sample_steps`。
- `composer/bin/emit_samples.rb` と `composer/examples/samples.ndjson` に、既存の生成済みドラムWAVとシンセベースの混在例。
- PCM16モノラル・ステレオWAVの全長・原音程再生。素材の聴感は利用者確認済み。
- 全WAVをミュート前に検証、検証済みバイト列のスナップショットをロード。相対パスは元の作曲ファイル基準。
- SynthDefの準備とバッファロードは先頭の共通時計基準を作る前に行う。200ms先行NTP送信を維持。
- 全再生イベントを時刻・入力順でマージし、同時発音を一つのbundleへ格納。
- バッファ再利用、更新失敗時の新規割当ロールバック、小節境界更新前のエンジン準備、予約発音終了までの保持。
- 有限再生は全小節を通じた最長の終了時刻まで待つ。通常の戻り値エラー時も既に送った発音が終わってから解放。
- `sample.rs`に純粋なWAV検証、`source.rs`に読込・一時ファイル、`sampler.rs`にエンジン資源の寿命、`osc.rs`にバッファコマンドと混在bundle。音楽イベントはバッファIDやOSCデータを保持しない。

## 検証

- TDDのRed/Green確認: 未対応sampleイベント、未実装API、ミュートで省かれていた時刻範囲検証、不正WAVチャンク順、未準備の純粋リロード。
- Rust 42件成功（既存28件＋サンプラー14件）。UDPが必要な2件はopt-inで未実行。
- 実Spinelによる全構成のコンパイル・fixture比較成功。サンプルパスのJSONエスケープ、不正パターンでの部分発音イベント非出力も検証。
- WAV素材チェック、cargo fmt --check成功。dry-runで混在14イベントとスネアのミュートを確認。
- `sclang synthdefs/sampler.scd`はoffscreen/generic設定で試したが、この実行環境の `_LocalIPs` 権限エラーにより初期化未完了、生成不可。sclangの失敗ログは `/tmp/spinel-sampler-sclang.log`。QtのGTKクラッシュではない。

## ホストで残る確認

音声サーバーが未起動なら `mise run audio` を起動する。別ターミナルで:

```sh
mise run play-samples
```

このタスクが `analog_bass.scd` と新しい `sampler.scd` をコンパイルしてから再生する。
キック・スネア・ハイハットとベースが同時に正しい間隔で聴こえること、CLIとサーバーのログに失敗がないことを確認する。
サンプラー経由の実音・新SynthDef生成はホスト確認待ちであり、完成確認済みとは扱わない。
追加検証は `cd core && cargo test --offline --test timing_tests -- --ignored`。
ライブ編集は `mise run watch-samples`、対象ファイルは `composer/bin/emit_samples.rb`。

## 制限

- WAVはPCM16、1–2ch、8–192kHz、30秒/32MiB以下。宣言WAV合計とキャッシュの入力バイト上限は64MiB。バッファ数は1024以下。
- サーバーは専用・プレイヤー1個を前提とする。他クライアントとのバッファID共有は未対応。
- サンプル音量はvelocityをそのまま掛ける。既存シンセのvelocity×0.2とは別。正規化・レート変更・ループ・ピッチ変更は未対応。
- WAVだけの変更ではwatch更新を検知しない。作曲ソースの変更時にWAVを再読込する。
- 新規バッファのサーバーロードは再生スレッド上で行うため、遅いロードは200msの先行余裕を超える可能性がある。遅延時刻は警告し、時計はずらさない。
- 強制終了時の解放は保証しない。必要なら専用サーバーを再起動する。
