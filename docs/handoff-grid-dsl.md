# Grid DSL 引き継ぎ（2026-11-01）

## 到達点

- `composer/lib/track_score.rb` に `score.track :kick do ... loop bars: 4 do ... end` を追加。`bar { hits 1, 2, 3, 3.75 }` は従来どおり1始まりの拍位置として解釈する。
- `bar grid: 4/8/16/32/64` は各小節の分割数を指定する。Grid付きの `hits` は1始まりの整数ステップ番号。Gridは小節ごとに変更できる。
- `use_sample` のトラックではサンプルを鳴らす。`use_synth :analog_bass` のトラックでは `hits 1, 5, note: "C3"` のように音名を指定し、音の長さは現在1 Gridステップ。複数の `hits` 呼び出しで1小節のメロディーを作れる。
- 新しい4小節ループは `tempo` の任意の `bars` フィールドで長さを明示し、無音の終端小節も維持する。旧スコアでは最後のイベント位置から一巡長を推定する。`beat` と `duration_beats` の出力は64分音符のため小数4桁になり、関連fixtureを再生成した。
- 例は `composer/bin/emit_four_bar_kick.rb` と `composer/bin/emit_grid_demo.rb`。`bd_303` は現行 `samples/kick.wav` の別名で、実機の音色再現ではない。

## 検証したこと

- `cargo test --offline --manifest-path core/Cargo.toml` と `SPINEL_BIN=/home/tkd/Projects/.tools/spinel/bin/spinel sh tests/check_spinel.sh` が通過。Rustでは64分音符の最終ステップ15.9375拍でサンプルと音名付きノートが同時になることを時計待ちなしで確認した。
- `--dry-run --watch --bars 4 composer/bin/emit_grid_demo.rb` と、`--mute-track kick` / `--mute-track bass` の各経路が通過。
- ホストで `scsynth -u 57111` にGridデモを送り、CLIは `scsynth accepted 35 note commands` と報告。サーバーログに追加エラーは出なかった。ユーザーによるGridデモの聴感確認は未実施。検証後、57111番のサーバーは停止した。
- 57110番はこの実行時に使用中だったため57111番で確認した。明日のポート状態は再確認する。
- テクノ例のBPMとハイハット位置について、コード（133 BPM、7.75拍）に合わせてfixture、テスト、READMEを修正済み。READMEの小数3桁記述も現行の4桁出力に合わせた。

## 次の作業候補

1. `mise run audio` と `mise run play-grid-demo` で利用者がGridデモを試聴し、リズムと短いメロディー音の聴感を確認する。サーバーの出力接続とポートを先に確認する。
2. 4小節版が意図どおりなら、`loop bars: 8` の意味と、トラックごとに一巡長が違う場合の規則を決めてから8小節版をRed-Green-Refactorで実装する。現在の `TrackPattern` は4小節のみ受け付ける。
3. 当時の音名付き `hits` は1ステップ長・固定ベロシティだった。2026-11-07に `duration_beats:` と `velocity:` を追加済み。現在の仕様は `console-grid-dsl.md` を参照する。
4. v0.6の保存編集の実機確認は `docs/next-steps.md` のまま残っている。

## 2026-11-07 卓チャンネルの音声グラフ

- DSLは上から `use_sample`/`use_synth` → `ha` → `dynamics` → `eq` → `aux` → `pan` → `fader` → `loop` の順で書ける（Codexで構文とNDJSON検証まで実装済み）。今回、Dynamics・EQ・Auxを含むトラックが実際に再生されるようにした。
- `core/src/mixer.rs` を追加。トラックごとの専用ステレオバスと常駐 `channel_strip` ノード、使用中Returnの `aux_return` ノード、ノードID・バス番号、小節境界での `/n_set`、音の末尾（Returnは+3秒）後の解放を担当する。音楽イベントにはバス番号を入れず、`PlaybackEvent.route`（トラック名）をOSC符号化時にバスへ解決する。
- H/A・Pan・Faderだけのチャンネルは従来どおりイベント単位の倍率とpan。
- 例: `composer/bin/emit_console_strip.rb`（fixture `composer/examples/console_strip.ndjson`）、`mise run play-console-strip` / `watch-console-strip`。`mise run synthdef` が `channel.scd` も生成する。
- 検証: Rust全テスト、実Spinel `check_spinel.sh`、NRT `check_channel_audio.py`（測定値は `audio-levels.md`）、ホスト `scsynth -u 57111` での有限再生・ライブFader変更・不正リロード無視。利用者の聴感確認は未実施。
- 未実施: RuboCopと `codemetrics` が未インストールのため、Ruby静的検査と関数長監査は走らせていない。（→ 下記で解消）

## 2026-11-07 関数長・複雑度のゲート化

- 全関数が10行以内・循環的複雑度10以下・認知的複雑度4以下になり、`static-check`（コミット前フック・CI・Codexフック）が関数長と複雑度も失敗扱いにする。基準と仕組みは `docs/quality-policy.md`。
- `mixer.rs` は再読み込み1回分を `Build`（成功時に一括コミット、失敗時に新規ノードを解放）にまとめ、`GraphBackend::add_node` は `NodeSpec` を受け取る形にした。`osc.rs` の再生バンドルは `Lookups`（サンプルバッファIDとストリップバスの解決）を受け取る。
- `main.rs` の `Engine` がイベント末尾までの予約（`end`）とサンプル用SynthDefの遅延読み込み（`SamplePlayer`）を持つ。
- 長いテストは振る舞いごとに分割し、共通処理は `core/tests/common/mod.rs` に置いた。

## 作業ツリー

未コミットの変更が多数ある。既存の変更と今回のDSL変更が混在しているため、次回は `git status --short` と実ファイルを確認してから編集する。`AGENTS.md` には追記していない。
