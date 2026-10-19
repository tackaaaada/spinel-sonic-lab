# spinel-sonic-lab：Omarchy開発引き継ぎ

作成日：2026-10-19（日本時間）

## 次のCodexへの依頼

作業場所は `/home/tkd/Projects/spinel-sonic-lab`。
`$tdd-master-coach` と `$design-principles-coach` を使い、まずこの資料と `AGENTS.md`、関係するコード・テストを読む。
現在の最優先課題は、4音サンプルで最初のド→ミだけ発音間隔が短く聴こえる問題の調査と修正。
これは原因確定・修正済みではない。失敗するテストの追加もまだ行っていない。

利用者はOmarchyを今後の開発拠点とし、必要に応じて自分でもターミナルでコマンドを実行しながら進めたい。日本語で目的と結果を短く説明し、小さな単位で確認する。音の最終判定は利用者に聴いてもらう。

## 現象：ユーザーによる聴感確認

- USB接続機器から実際に音が出ることは、利用者が確認済み。
- サンプルはド・ミ・ソ・高いドの4音。利用者の期待は8分音符で均等な間隔と音量。
- 「音が潰れる」ではなく「最初のドの発音が遅れ、ド→ミの間隔が短くなりリズムが崩れる」が正確な説明。
- 利用者の表現では約32分音符ぶんのずれ。これは聴感による概算で、録音・オンセット測定はまだしていない。
- scsynthを起動したまま `mise run play` を繰り返しても同じように聴こえる。
- 初回だけの音色読み込み遅延という仮説は裏付けられていない。

## 確認済みの実行結果

`mise run doctor`：必要ツール確認が成功。出力先は `omarchy_host_output_a931d4132781f73e`（Mac System Default）。VM空き約30GB。

`mise run test`：Rust 27件（lib 2、core 18、live 7）が成功。実際のSpinelコンパイルと各fixtureの出力照合も成功。
これらは今回の実発音タイミングの正常性を保証していない。

`mise run check-score`：

```text
0.000s freq=65.406Hz dur=0.250s amp=0.160 gain=2.800 output_amp=0.448
0.250s freq=82.407Hz dur=0.250s amp=0.160 gain=2.800 output_amp=0.448
0.500s freq=97.999Hz dur=0.250s amp=0.160 gain=2.800 output_amp=0.448
0.750s freq=130.813Hz dur=0.250s amp=0.160 gain=2.800 output_amp=0.448
```

楽譜・Rustスケジュール上は250ms間隔で音量指定も同じ。

音声サーバーは別ターミナルで利用者が `mise run audio` を実行している。作業開始時に現在の起動状態を確認する。二重起動や無断停止をしない。

```text
scsynth -u 57110 -B 127.0.0.1 -i 0 -o 2
SC_AudioDriver: sample rate = 48000.000000, driver's block size = 4096
JackDriver: connected SuperCollider:out_1 to Mac System Default:playback_FL
JackDriver: connected SuperCollider:out_2 to Mac System Default:playback_FR
SuperCollider 3 server ready.
```

4096 / 48000 ≈ 85.33ms。大きいバッファが関係する可能性はあるが、単独で今回の原因とは断定していない。Macの既定出力はUSB機器を使用している。

## コードから確認した事実

以下の行番号はユーザーが提示した時点のもの。実ファイルで再確認する。

- `core/src/main.rs:114–117`：OSC接続→`load_synthdef`完了待ち→`Instant::now()`→再生。
  音色ロード待ちを再生開始時刻に含めている構造ではない。
- `main.rs:196–204` の `play_scheduled_notes`：

```rust
for (at, group) in group_notes_by_time(notes) {
    sleep_until(start, at)?;
    osc.play_notes_bundle(OSC_IMMEDIATE_TIMETAG, group)?;
}
```

- 最初の音はoffset 0なので、準備の先行時間を取らず即時指定で送信する。
- 後続もRust側で予定時刻まで待ち、即時指定で送る。将来時刻を指定するlookahead送信ではない。
- `core/src/osc.rs`：`OSC_IMMEDIATE_TIMETAG = 1`。
- `OscClient::play_notes_bundle` と `encode_bundle` は明示的なtimetagを受け取れる。
- `system_time_to_ntp(SystemTime)` の変換関数も存在。
- `load_synthdef` は `/d_load` 送信後 `/done` を待つ。
- `verify_commands` は `/sync` を送信して `/synced` または `/fail` を確認する。
  通信の成功を、人が音を聴いたことや正確な実発音時刻の証明にしない。
- `run_watch` も同じ再生関数を使用。小節境界まで待って次のbarを選ぶ実装。

## 既存テストと未実施の次の一歩

`core/tests/core_tests.rs:504` に
`osc_bundles_group_simultaneous_notes_and_encode_ntp_timetags` がある。
`encode_bundle(OSC_IMMEDIATE_TIMETAG, ...)` のパケットを確認しているが、CLIが未来の発音時刻を指定して先行送信する振る舞いは検証していない。

次に読む予定だった内容：

```bash
sed -n '1,90p' core/tests/live_tests.rs
sed -n '490,550p' core/tests/core_tests.rs
```

これらの結果はこのチャットではまだ受け取っていない。次のCodexが直接読めばよい。

## 調査・修正の候補（決定済み設計ではない）

即時送信とサーバーの処理タイミングが揺らぎを生む可能性を調べる。候補は、十分先の共通再生開始時刻を設け、各ノートを未来のNTP時刻付きbundleとして先行送信すること。
単に最初にsleepを足すだけでは、全体が遅れるだけで即時送信の揺らぎが残る可能性がある。

TDDでは以下を考慮する：

1. 最初のノートにも送信から発音までの余裕があること。
2. 4音の指定時刻の差が250msずつになること。同時発音は同一bundleを維持。
3. 純粋な時刻計算は注入した固定時刻で検証し、OSのsleepが厳密に何msだったかを単体テストにしない。
4. 必要ならローカルUDPの偽サーバーでCLIが実際に送るbundleを観測する。受信は上限付きタイムアウトにする。
5. `--watch` の小節境界での更新、無効な更新時に前の楽譜を維持、テンポ変更、`--bars` 終了時の最後の音の待機を壊さない。
6. 先行時間を加える場合、再生終了待機にも整合性が必要。
7. InstantとSystemTimeの基準を揃え、経過時間ごとに別々の現在時刻から発音予定を作らない。遅れた送信の扱いも具体的に検討する。
8. 長い楽譜を一括で無制限に送る等の変更は避け、最小の必要範囲にする。

まず失敗を確認し、最小実装→関連テスト→既存の全検証へ進む。コード上の改善と聴感の改善を区別して報告する。必要に応じて同じ条件で録音・オンセット間隔を測り、最終的にはユーザーに再生を聴いてもらう。

## 守ること

`AGENTS.md`を優先して全文を読む。
composition / runtime scheduler / OSC transport / audio engineの責務を分離する。時刻送信の都合を音楽イベントやNDJSONへ混ぜない。
今回、音量・gain・音色・ADSR・楽譜の音間隔は勝手に変更しない。
実装が変わったらAGENTS.md中の「現在は即時timetagで送信」という説明や関連ドキュメントも事実に合わせる。
音源変更時のSynthDef再生成、gain変更時のpeak/RMS測定規約も守る。

## 開発コマンド

```bash
cd ~/Projects/spinel-sonic-lab
mise tasks
mise run doctor
mise run test
mise run check-score
# 別ターミナル：
mise run audio
# 再生側：
mise run play
# ライブ編集確認は必要に応じて：
mise run watch
```

Ruby 4.0.1・Rust 1.99.1はプロジェクトのmise管理。Spinelは`~/Projects/.tools/spinel/bin/spinel`を使用。
Spinel互換性の検証をCRuby実行で代用しない。

## 配置とスキル

Mac上の元コードを残し、Omarchyの`~/Projects/`直下へ各プロジェクトを整理済み。
spinel-sonic-labは移行時点でGitリポジトリではなかった。作業開始時に現状を確認し、無断でgit initやGitHub公開をしない。
GitHub CLI自体はtackaaaadaで認証済み。他の既存リポジトリで一時ブランチへのPush・Pull・削除を確認済み。

スキルの配置先：
- `/home/tkd/.agents/skills/design-principles-coach/SKILL.md`
- `/home/tkd/.agents/skills/tdd-master-coach/SKILL.md`

両方ともMacのスキルを参照・テンプレート・agentsメタデータごとコピーする。内容の改変はしない。
Codexの次の入力で利用できる。見えない場合はセッションを開き直し、`/skills`で確認する。
公式説明：https://learn.chatgpt.com/docs/build-skills

この引き継ぎ作成では再生実装・テストを変更していない。修正はここから。

## 2026-10-19 調査・修正追記（上記は修正前の記録）

- 原因は引き続き未確定。即時bundleにより受信・処理タイミングが発音予定へ反映される経路を修正した。聴感改善は未確認。
- `main.rs` の `PlaybackClock` がSynthDefロード後のInstant/SystemTimeを基準に200ms先の開始時刻を作る。送信は楽譜のoffsetに沿って刻み、発音は共通基準の未来NTP時刻で指定する。最終音の終了待ちにも200msを含める。音色、音量、ADSR、楽譜、プロトコルは変更していない。
- watchは発音小節境界の200ms前に検証済み更新を確定し、音は境界で切り替わる。受付締切後の更新はさらに次の小節へ回る。遅れた送信は予定時刻を保持して警告する。大きなOS停止や時計の補正に対する正確な発音は保証しない。
- TDD: 時刻計算テストを先に追加し、未実装PlaybackClockによるコンパイル失敗を確認。その後実装しGreen。先頭200ms、250ms間隔、小節offset、範囲外時刻を固定した入力で検証する。
- Rust 28件成功。CLIのUDPテストはソケット作成が `Operation not permitted` となる環境制限のためopt-inにした。実送信のRed/Greenは未確認。ホスト上で `cd core && cargo test --offline --test timing_tests -- --ignored` を実行する。通常再生とwatch 2小節のtimetag・終了待ちを観測する。
- 起動中のscsynthがないことを確認後、standalone起動を試したが `could not initialize audio` で終了。この実行環境での音声統合と録音測定は未実施。
- 利用者の環境で `mise run audio`、別ターミナルで `mise run play` を数回実行し、最初のド→ミが後続と同じ間隔に聴こえるか確認する。改善しない場合は同条件の録音でonset間隔を測定し、音声バッファ経路と実際の発音を引き続き調べる。
- `SPINEL_BIN=/home/tkd/Projects/.tools/spinel/bin/spinel sh tests/check_spinel.sh` は全構成で成功。`cargo fmt --check` も成功。

## 利用者による修正後の聴感確認

200ms先行送信への変更後、利用者から「同じ間隔で聞こえます！」「これでOK」と確認を得た。最初のド→ミを含む4音の間隔の問題は対応完了。即時送信が聴感上のずれを生んだ詳細な経路は録音で特定していない。
