# 次回作業メモ（2026-10-19作成）

サンプラーのコードは実装済み。ホスト側の音声統合確認から再開する。詳細は `handoff-sampler.md`。

## 1. ホスト確認（最優先）

1. `sclang synthdefs/sampler.scd` で `sample_mono.scsyndef` と `sample_stereo.scsyndef` を再生成する。前回は `_LocalIPs` の権限エラーで失敗した。
2. `mise run audio` で scsynth を起動する。
3. 別ターミナルで `mise run play-samples` を実行し、キック・スネア・ハイハットとベースが同時に正しい間隔で鳴ることをCLIログとscsynthログで確認する。
4. `cd core && cargo test --offline --test timing_tests -- --ignored` を実行する。
5. 聴感を確認する。OSC送信の成功だけでは、人が聴いた証拠にならない。
6. `mise run watch-samples` でライブ編集を確認する。

## 2. 確認後に選ぶ候補

- B: サンプラーの制限解消（ピッチ変更、ループ、音量正規化、WAVだけの変更をwatchで検知）。
- C: v1.0の完成条件の決定（任意スコアのWAV書き出しCLIなど）。

## 3. 完了時の更新

確認が済んだら `roadmap.md` のv0.6の残作業と `handoff-sampler.md` の「ホストで残る確認」を更新する。
