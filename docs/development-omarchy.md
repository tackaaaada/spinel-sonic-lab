# OmarchyでのSpinel Sonic Lab開発

## 現在の構成

- プロジェクト: `~/Projects/spinel-sonic-lab`
- Bashを使用。プロジェクトの`mise.toml`でRuby 4.0.1とRust 1.99.1を選択。
- Spinel: `~/Projects/.tools/spinel/bin/spinel`。コミット`25351692ba8c610b3af1a8e8fe1f8f098102a2c6`からLinux向けにビルド。
- `SPINEL_BIN`はmiseで自動設定。CRubyを作曲ファイルの実行に代用しない。
- SuperCollider 3.14.1。音声はPipeWireからTry OmarchyのMac音声ブリッジを経由する。音声起動タスクがSC_JACK_DEFAULT_OUTPUTS=systemを指定し、自動接続する。
- Mac側の既定出力はAKGのUSB機器。Macで出力先を変えるとOmarchyのMac System Defaultにも反映される。

## 初回の音声接続

Omarchyのターミナルで管理者認証を行い、JACK互換ライブラリを追加する。

```sh
sudo pacman -S --needed pipewire-jack
```

`jack2`の置き換え確認は`y`。依存関係エラーの場合は強制削除せず表示を確認する。

## まず状態確認とテスト

```sh
cd ~/Projects/spinel-sonic-lab
mise run doctor
mise run test
mise run check-score
```

`doctor`は必要なツール、PipeWire-JACK、音声サービスと空きを確認。
`test`はRustのテストと実際のSpinelによる作曲例検証。
`check-score`は音を鳴らさず、4音のサンプルをコンパイルして予定を表示。

## 演奏

ターミナル1:

```sh
cd ~/Projects/spinel-sonic-lab
mise run audio
```

`SuperCollider 3 server ready`を確認する。サーバーは127.0.0.1のUDP 57110だけで待ち受け、入力チャンネル0・ステレオ出力2で起動する。

ターミナル2:

```sh
cd ~/Projects/spinel-sonic-lab
mise run play
```

最初は機器の音量を小さくする。CLIの`scsynth accepted ... note commands`は受理の確認であり、人に音が聞こえたこととは別。

## ライブ編集

音声サーバーを起動したまま、別のターミナルで:

```sh
cd ~/Projects/spinel-sonic-lab
mise run watch
```

さらに別のペインで:

```sh
cd ~/Projects/spinel-sonic-lab
nvim composer/bin/live_session.rb
```

有効な変更は次の4拍の小節境界で反映される。無効な変更は直前の有効なスコアの再生を継続する。

再生と音声サーバーの停止は、それぞれのターミナルでCtrl+C。
Herdrから画面を抜けるCtrl+B→Dは作業を保持する操作で、停止ではない。

## エディターとシェル

- `v`でNeovim、`ll`で詳細一覧、`liz`でZellij。
- `.scd`はscnvimで認識。Space→s→sがsclang起動、Space→s→bがサーバー起動。
- 既に独立したscsynthを起動しているときは、別のサーバーを重ねて起動しない。
- 日本語切替はCtrl+Space。Herdr開始キーはCtrl+B。

## 検証状況

- Rust全27テストとSpinel作曲例検証はLinux上で成功済み。
- `mise run check-score`は4音の予定を表示して成功。
- `mise run synthdef`で音源定義の生成成功。
- NeovimのSuperColliderファイル認識、SCNvimStart登録、sclangパスを確認。
- PipeWire-JACK導入済み。48000Hzでサーバー起動、左右出力からMac System Defaultへの接続、4音の演奏コマンド受理まで確認済み。USB機器から実際に聞こえたかはユーザー確認待ち。

音色・ゲイン・OSC契約・演奏ロジックは変更していない。

## SynthDef生成時のQtテーマ対策

`synthdef`タスクは `QT_QPA_PLATFORM=offscreen` と
`QT_QPA_PLATFORMTHEME=generic` を指定する。デスクトップ側の `gtk3`
テーマ設定を引き継いでGTKがディスプレイ接続を試みることを避ける。
2026-10-19のsclangのSIGABRT記録では `gtk_init` → `libqgtk3` →
`QPlatformThemeFactory` の呼び出しを確認した。
`play`・`watch`・`samples`はこのタスクに依存するため同じ設定を使う。

`Couldn't set realtime scheduling priority` は優先度の権限に関する警告。
この警告だけでは生成失敗とは判断せず、生成コマンドの終了とSynthDefの更新を確認する。
Codexの制限環境ではQt初期化を通過しても `_LocalIPs` の権限エラーで
sclangの初期化が失敗するため、完全な再生成確認はホスト側で行う。
