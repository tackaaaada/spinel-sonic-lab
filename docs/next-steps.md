# 次回作業メモ（2026-11-01更新）

サンプラーのコードと SynthDef 生成、ホスト側UDP時刻テスト、4小節のOSC再生・実音は確認済み。詳細は `handoff-sampler.md`。

4小節のRuby風Grid DSLの実装・未確認事項・8小節版の候補は `handoff-grid-dsl.md` に記録した。Gridデモの実音はまだ利用者未確認。

2026-11-07: Gridの `hits` に `velocity:` とシンセ用 `duration_beats:` を追加し、実SpinelとRustの時刻・MIDI・同時発音テストで確認した。卓の H/A → Dynamics → EQ → Aux → Pan → Fader とGridを組み合わせる目標・実装順は `console-grid-dsl.md`。

同日更新: H/A・Pan・FaderはDSL、NDJSON、Rust検証・スケジュール、OSC、SynthDefまで実装。`emit_console_grid.rb` を実Spinelでコンパイルし、Rust全テストとNRT左右・レベル測定が通過。ホストの専用scsynth 57112で4小節12発音命令を受理し、追加エラーなし。利用者の聴感確認は未実施。Dynamics、EQ、Aux SendとReturnの構文とNDJSON検証も追加したが、共有トラックバスと処理グラフは未実装。現在はこれらの設定を持つスコアの再生を明示的に拒否する。

同日更新2: 共有トラックバスと常駐チャンネルストリップ、コンプレッサー、ベルEQ、Aux Send/Return（リバーブ）を実装し、拒否を解除した。`mise run play-console-strip` で全モジュールとGridの組み合わせを試聴できる。NRT測定とホストscsynthでの再生・ライブ変更は通過。利用者の聴感確認が次の作業。

## 1. 残る確認（最優先）

テクノ試作のキックとベースは2026-11-01の試聴で現時点OK。別の音源・編曲の聴感はその都度判断する。

1. `mise run audio` と `mise run watch-samples` を別ターミナルで起動し、`composer/bin/emit_samples.rb` の有効な保存編集が次小節に反映し、無効な編集でも演奏が続くことを確認する。

## 2. 確認後に選ぶ候補

- B: サンプラーの制限解消（ピッチ変更、ループ、音量正規化、WAVだけの変更をwatchで検知）。
- C: v1.0の完成条件の決定（任意スコアのWAV書き出しCLIなど）。

## 3. 完了時の更新

確認が済んだら `roadmap.md` のv0.6の残作業と `handoff-sampler.md` の「ホストで残る確認」を更新する。
