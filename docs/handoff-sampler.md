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

### 2026-11-01 追確認

- `emit_samples.rb` のベースを fixture と設計記録どおり 0.5/1.5/2.5/3.5 拍へ戻した。実 Spinel で全構成の fixture 比較が成功し、Rust テストも Git LFS の WAV 実体を取得した状態で全件成功した。
- ホスト権限で `analog_bass.scd` と `sampler.scd` の SynthDef を再生成。UDP 時刻テスト2件が成功した。
- 独立した `scsynth` は `SuperCollider 3 server ready.` を出力。4小節の CLI 再生は終了コード0で `scsynth accepted 56 note commands` を出力し、サーバーログにエラーは出なかった。再生後にサーバーは停止した。
- その後、利用者が起動した `scsynth -u 57110` への再生は受理されたが無音だった。PipeWireで `SuperCollider:out_1`/`out_2` が未接続と確認し、既定の `omarchy_host_output` の左右へ接続した。同じ4小節を再生し、利用者が聞こえたことを確認した。
- その後のテクノ試作では、キックとベースの現在のバランスを利用者がOKと確認した。サンプラー例の個別のバランス評価と、保存編集が次小節で反映するかは未確認。

## ホストで残る確認

サンプラー経由のOSC送信と新SynthDef生成は2026-11-01にホスト確認済み。`cd core && cargo test --offline --test timing_tests -- --ignored` もホスト権限で2件成功済み。
サンプラー例の実音は利用者確認済み。テクノ試作のキックとベースは現時点でOK。サンプラー例そのもののバランス評価と、保存編集の実機確認は別に残る。後者は `mise run audio` と `mise run watch-samples` を別ターミナルで起動し、`composer/bin/emit_samples.rb` を編集する。再び無音なら `development-omarchy.md` のPipeWire接続手順を確認する。

## 制限

- WAVはPCM16、1–2ch、8–192kHz、30秒/32MiB以下。宣言WAV合計とキャッシュの入力バイト上限は64MiB。バッファ数は1024以下。
- サーバーは専用・プレイヤー1個を前提とする。他クライアントとのバッファID共有は未対応。
- サンプル音量はvelocityをそのまま掛ける。既存シンセのvelocity×0.2とは別。正規化・レート変更・ループ・ピッチ変更は未対応。
- WAVだけの変更ではwatch更新を検知しない。作曲ソースの変更時にWAVを再読込する。
- 新規バッファのサーバーロードは再生スレッド上で行うため、遅いロードは200msの先行余裕を超える可能性がある。遅延時刻は警告し、時計はずらさない。
- 強制終了時の解放は保証しない。必要なら専用サーバーを再起動する。
