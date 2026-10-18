# Feedability

RSS / Atom のフィードを購読して読む、ローカルのフィードリーダー（Tauri 2 / macOS・Windows）。
アカウントもサーバーも要らない。購読と記事はこの PC の中にだけ置く。

## 機能

- **購読**：フィードの URL か、サイトの URL を入れるだけ（ページからフィードを探す）。RSS 0.9x / 1.0 / 2.0、Atom、JSON Feed。Shift_JIS や EUC-JP のフィードもそのまま読める
- **整理**：フォルダに分ける。名前の変更、フォルダへの移動、購読をやめる（右クリック、またはメニューの「フィード」）
- **フィードの情報**：⌘I。URL・サイト・最後に更新した日時・記事の数・更新できなかった理由の全文。URL の付け替えもここ（[docs/FEED_INFO.md](docs/FEED_INFO.md)）
- **アイコン**：サイドバーと一覧にフィードのアイコン（[docs/FAVICON.md](docs/FAVICON.md)）
- **ペインの幅**：サイドバーと一覧の右端をドラッグして変える。ダブルクリックで既定に戻る（[docs/LAYOUT.md](docs/LAYOUT.md)）
- **読む**：3 列（フィード / 記事の一覧 / 本文）。開いた記事は既読になる。未読のみ・すべての切り替え、スター、検索（タイトル・要約・フィード名、空白区切りですべてを含むもの）
- **あとで読む**：L で入れておき、入れた順に読んでいく。読み終えて次へ移ると自動で外れる（[docs/LATER.md](docs/LATER.md)）。スターは自分で外すまで残る保存
- **外観**：ライト / ダーク。既定は OS に合わせる（設定 › 外観。[docs/THEME.md](docs/THEME.md)）
- **一覧をすべて既読にする**：表示中の一覧を既読にする。トーストの「元に戻す」で戻せる
- **更新**：起動の少しあとと、設定した間隔（既定 30 分）で全フィードを更新する。⌘R で手動。更新できなかったフィードはサイドバーに赤い印が付き、印にマウスを載せると理由が出る
- **本文**：フィードの HTML から、スクリプト・スタイル・フォームなどを取り除いて表示する。リンクは既定のブラウザで開く。画像はアプリが取得して表示する（下の「ネットワーク」）
- **stockroom へ送る**：取っておきたい記事を、本文ごと Markdown にして stockroom（同じ作りの保管庫アプリ）へ送る（Shift+S）。記事の右クリックから Markdown でコピーもできる（[docs/STOCKROOM.md](docs/STOCKROOM.md)）
- **ブラウザ拡張**：Chrome / Edge から、見ているページのフィードを購読したり、ページをあとで読むに入れたりする（設定 › ブラウザ拡張。既定はオフ。[docs/EXTENSION.md](docs/EXTENSION.md)）
- **全文取得**：要約だけを配るフィードは、フィードごとに「全文を取得する」をオンにすると、記事のページから本文を抜き出して表示する（既定はオフ。[docs/FULL_TEXT.md](docs/FULL_TEXT.md)）
- **OPML**：ほかのリーダーとの購読の受け渡し（ファイル › OPML を読み込む / 書き出す）。`.opml` ファイルを開いても読み込める（確認のあと）
- **`feed:` リンク**：ブラウザなどで `feed:` のリンクを開くと、「フィードを追加」がその URL 入りで開く（[docs/OPEN_WITH.md](docs/OPEN_WITH.md)。インストールしたアプリでだけ効く）
- **古い記事の整理**：既読でスターの無い記事は、フィードから消えて設定した日数（既定 30 日）が過ぎたら削除する

## キーボード

| キー | 操作 |
|---|---|
| J / ↓ | 次の記事 |
| K / ↑ | 前の記事 |
| N | 次の未読の記事（一覧に無ければ未読のある次のフィード） |
| Shift+J / Shift+K | 次 / 前のフィード |
| Space | 本文をスクロール（最後まで読んだら次の記事） |
| M | 既読 / 未読の切り替え |
| S | スターを付ける / 外す |
| L | あとで読むに入れる / 外す |
| V / Return | ブラウザで開く |
| Shift+S | stockroom に送る |
| ⌘R | すべて更新 |
| ⌘⇧R | 開いているフィードを更新 |
| ⌘I | 開いているフィードの情報（URL の変更もここ） |
| ⌘⇧A | 一覧をすべて既読にする |
| ⌘U | 未読のみ表示の切り替え |
| ⌘F | 検索 |
| ⌘1 / ⌘2 / ⌘3 | すべての記事 / あとで読む / スター付き |
| ⌘N | フィードを追加 |
| ⌘⇧N | フォルダを作成 |
| ⌘+ / ⌘− / ⌘0 | 本文の文字を大きく / 小さく / 標準に |
| ⌘, | 設定 |
| ? | ショートカット一覧 |

Windows は ⌘ → Ctrl、⌥ → Alt。1 文字のキーは検索欄などの入力欄の外で効く。

## データ

`feedability.db`（SQLite）1 ファイルに、購読・フォルダ・記事・既読・スター・あとで読むを入れている。

| OS | 場所 |
|---|---|
| macOS | `~/Library/Application Support/com.local.feedability/feedability.db` |
| Windows | `%APPDATA%\com.local.feedability\feedability.db` |

設定（`settings.json`）も同じフォルダにある。

**バックアップ・別の PC へ**：購読の一覧は **ファイル › OPML に書き出す** で持ち運べる（記事はフィードから取り直せる）。既読やスターも残したいときは、アプリを終了してから `feedability.db` をコピーする（終了時に 1 ファイルへ書き戻している）。1 つの `feedability.db` を同期フォルダに置いて複数の PC から開く使い方は想定していない（[ADR-0002](decisions/adr-0002-data-in-app-data-dir.md)）。

## ネットワーク

アプリが通信するのは次だけ。テレメトリは送らない（[ADR-0001](decisions/adr-0001-fetching-feeds.md)）。

- 購読したフィード（追加するときは、入力したサイトのページも。フィードを探すため）
- 記事の本文の画像と、一覧の「画像付き」の縮小版（見えている行の分だけ。アプリが小さくしてから渡す）。ウィンドウの中から外へは通信せず、アプリが取得して渡す（JPEG・PNG・GIF・WebP・AVIF・BMP・ICO。SVG は表示しない）
- 購読したフィードのサイトのアイコン（フィード自身のアイコン → ページの `<link rel="icon">` → `/favicon.ico` → フィードのロゴの順。30 日に 1 回まで、256 KB まで、SVG は使わない。[docs/FAVICON.md](docs/FAVICON.md)）
- 全文取得をオンにしたフィードの記事のページ（既定はオフ。フィードごとに選ぶ。新しい記事を 1 回の更新で 20 件まで、古い記事は開いたときに 1 回だけ。5 MB まで。手元のネットワークの IP を直接書いた URL は取らない。[docs/FULL_TEXT.md](docs/FULL_TEXT.md)）
- 記事を送ると選んだときだけ、この PC の stockroom の受付（`127.0.0.1:41620`。プロキシを通さない。[ADR-0004](decisions/adr-0004-sending-to-stockroom.md)）
- アップデートの確認（GitHub Releases）

プロキシは OS の設定（自動構成スクリプト（PAC）を含む）をそのまま使う。

**待ち受け**：設定でブラウザ拡張の受付をオンにしている間だけ、`127.0.0.1:41630` で待ち受ける（このパソコンの中の、接続を許可した拡張機能だけ。Web ページからの接続は拒否。[ADR-0005](decisions/adr-0005-browser-extension-receiver.md)）。`41631` はフェーズ 8 の MCP 用に空けてある。

## インストール

[Releases](https://github.com/hamirilo/feedability/releases) から入手する。署名なしのビルドなので、**初回インストール時だけ**警告が出る。

- **macOS（Apple Silicon 専用）**：`.dmg` を開いて「アプリケーション」へコピー → 起動すると「開発元を検証できない」と出るので「完了」→ **システム設定 › プライバシーとセキュリティ** の下にある「このまま開く」。
  ターミナルで `xattr -dr com.apple.quarantine "/Applications/Feedability.app"` でもよい
- **Windows**：`-setup.exe` を実行 → SmartScreen が出たら「詳細情報」→「実行」

## アップデート

2 回目以降はアプリ内で更新する。起動から数秒後に確認し、新しい版があればダイアログで知らせる（設定でオフにできる）。手動では設定（⌘,）の「今すぐ確認」か、メニューの「アップデートを確認…」。

## ログ

ヘルプ › ログを表示 で、ログのファイル（`feedability.log`）を Finder / エクスプローラで開く。

| OS | 場所 |
|---|---|
| macOS | `~/Library/Logs/com.local.feedability/` |
| Windows | `%LOCALAPPDATA%\com.local.feedability\logs\` |

## 開発

前提：Node.js 22+、Rust（stable）、[Tauri の前提環境](https://tauri.app/start/prerequisites/)

```bash
npm install
npm run tauri dev      # アプリを起動
npm run dev            # ブラウザで UI だけ（モック。src/dev/mockBackend.ts）
npm test               # Rust のユニットテスト + 型チェック
npm run lint           # ESLint
cargo run --manifest-path src-tauri/Cargo.toml --example fetch -- <URL>   # アプリなしでフィードを取得・解析
```

開発の基準は [desktop-platform](https://github.com/jazmf-dx/desktop-platform) の `ai/ONBOARDING.md`。前提は [decisions/project-context.md](decisions/project-context.md)。

## 構成

```text
src/                      React UI
  App.tsx                 3 列のレイアウト、キー処理、自動更新のタイマー
  store.ts                zustand ストア（表示・一覧・開いている記事・トースト）
  lib/api.ts              Rust コマンドの型付きラッパ
  lib/commands.ts         コマンド表（メニュー・右クリック・キー・ショートカット一覧の元）
  lib/menuBar.ts          メニューバー
  lib/actions.ts          ダイアログを伴う操作（追加・削除・OPML）
  lib/update.ts           アプリ内アップデート
  components/             サイドバー、記事の一覧、本文、ダイアログ、トースト
  dev/mockBackend.ts      ブラウザ開発用のモック
src-tauri/
  src/lib.rs              plugin の配線、setup、invoke_handler
  src/commands.rs         フロントから呼ぶコマンド
  src/db.rs               SQLite（スキーマ、問い合わせ）
  src/fetch.rs            HTTP（外への通信の唯一の入口）
  src/feed.rs             フィードの解析、ページからのフィード探索
  src/sanitize.rs         本文の HTML を安全にする
  src/opml.rs             OPML の読み書き
  src/proxy.rs            OS のプロキシ（PAC）の解決
  examples/fetch.rs       アプリなしでフィードを取得・解析
docs/                     ARCHITECTURE.md、SETTINGS.md、THEME.md、LATER.md、LIST.md、LAYOUT.md、FEED_INFO.md、FAVICON.md、STOCKROOM.md、ROADMAP.md
decisions/                project-context.md、ADR
```

## リリース

1. `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` のバージョンを揃えて上げる（`npm version X.Y.Z --no-git-tag-version`、`cargo update -p feedability --manifest-path src-tauri/Cargo.toml`）
2. `CHANGELOG.md` に `## X.Y.Z` の節を書く（変更点 / ダウンロード / 注意）
3. `chore: vX.Y.Z` で main に入れ、`vX.Y.Z` の tag を push する
4. CI がバージョンと CHANGELOG と署名鍵を確かめてから、macOS と Windows のインストーラを作り、**draft** のリリースを作る（本文は CHANGELOG の節）
5. draft を確認して publish する。publish されるまでアプリ内アップデートには届かない

署名鍵（`TAURI_SIGNING_PRIVATE_KEY`、`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`）は GitHub Secrets に置く。owner（hamirilo）の鍵を使う（鍵 ID `22866C4837734BD8`。desktop-platform の playbook/updater-key.md）。秘密鍵のファイルとパスワードはパスワードマネージャーにもバックアップしておく。鍵が無いと tag のビルドは失敗し、それ以外のビルドはインストーラだけを作る。
