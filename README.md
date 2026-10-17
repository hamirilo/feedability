# Feedability

RSS / Atom のフィードを購読して読む、ローカルのフィードリーダー（Tauri 2 / macOS・Windows）。
アカウントもサーバーも要らない。購読と記事はこの PC の中にだけ置く。

## 機能

- **購読**：フィードの URL か、サイトの URL を入れるだけ（ページからフィードを探す）。RSS 0.9x / 1.0 / 2.0、Atom、JSON Feed。Shift_JIS や EUC-JP のフィードもそのまま読める
- **整理**：フォルダに分ける。名前の変更、フォルダへの移動、購読をやめる（右クリック、またはメニューの「フィード」）
- **読む**：3 列（フィード / 記事の一覧 / 本文）。開いた記事は既読になる。未読のみ・すべての切り替え、スター、検索（タイトル・要約・フィード名、空白区切りですべてを含むもの）
- **一覧をすべて既読にする**：表示中の一覧を既読にする。トーストの「元に戻す」で戻せる
- **更新**：起動の少しあとと、設定した間隔（既定 30 分）で全フィードを更新する。⌘R で手動。更新できなかったフィードはサイドバーに赤い印が付き、印にマウスを載せると理由が出る
- **本文**：フィードの HTML から、スクリプト・スタイル・フォームなどを取り除いて表示する。リンクは既定のブラウザで開く。画像はアプリが取得して表示する（下の「ネットワーク」）
- **OPML**：ほかのリーダーとの購読の受け渡し（ファイル › OPML を読み込む / 書き出す）
- **古い記事の整理**：既読でスターの無い記事は、フィードから消えて設定した日数（既定 30 日）が過ぎたら削除する

## キーボード

| キー | 操作 |
|---|---|
| J / ↓ | 次の記事 |
| K / ↑ | 前の記事 |
| Space | 本文をスクロール（最後まで読んだら次の記事） |
| M | 既読 / 未読の切り替え |
| S | スターを付ける / 外す |
| V / Return | ブラウザで開く |
| ⌘R | すべて更新 |
| ⌘⇧R | 開いているフィードを更新 |
| ⌘⇧A | 一覧をすべて既読にする |
| ⌘U | 未読のみ表示の切り替え |
| ⌘F | 検索 |
| ⌘1 / ⌘2 | すべての記事 / スター付き |
| ⌘N | フィードを追加 |
| ⌘⇧N | フォルダを作成 |
| ⌘, | 設定 |
| ? | ショートカット一覧 |

Windows は ⌘ → Ctrl、⌥ → Alt。1 文字のキーは検索欄などの入力欄の外で効く。

## データ

`feedability.db`（SQLite）1 ファイルに、購読・フォルダ・記事・既読とスターを入れている。

| OS | 場所 |
|---|---|
| macOS | `~/Library/Application Support/com.local.feedability/feedability.db` |
| Windows | `%APPDATA%\com.local.feedability\feedability.db` |

設定（`settings.json`）も同じフォルダにある。

**バックアップ・別の PC へ**：購読の一覧は **ファイル › OPML に書き出す** で持ち運べる（記事はフィードから取り直せる）。既読やスターも残したいときは、アプリを終了してから `feedability.db` をコピーする（終了時に 1 ファイルへ書き戻している）。1 つの `feedability.db` を同期フォルダに置いて複数の PC から開く使い方は想定していない（[ADR-0002](decisions/adr-0002-data-in-app-data-dir.md)）。

## ネットワーク

アプリが通信するのは次だけ。テレメトリは送らない（[ADR-0001](decisions/adr-0001-fetching-feeds.md)）。

- 購読したフィード（追加するときは、入力したサイトのページも。フィードを探すため）
- 記事の本文の画像。ウィンドウの中から外へは通信せず、アプリが取得して渡す（JPEG・PNG・GIF・WebP・AVIF・BMP・ICO。SVG は表示しない）
- アップデートの確認（GitHub Releases）

プロキシは OS の設定（自動構成スクリプト（PAC）を含む）をそのまま使う。

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
docs/                     ARCHITECTURE.md、SETTINGS.md、ROADMAP.md
decisions/                project-context.md、ADR
```

## リリース

1. `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` のバージョンを揃えて上げる（`npm version X.Y.Z --no-git-tag-version`、`cargo update -p feedability --manifest-path src-tauri/Cargo.toml`）
2. `CHANGELOG.md` に `## X.Y.Z` の節を書く（変更点 / ダウンロード / 注意）
3. `chore: vX.Y.Z` で main に入れ、`vX.Y.Z` の tag を push する
4. CI がバージョンと CHANGELOG と署名鍵を確かめてから、macOS と Windows のインストーラを作り、**draft** のリリースを作る（本文は CHANGELOG の節）
5. draft を確認して publish する。publish されるまでアプリ内アップデートには届かない

署名鍵（`TAURI_SIGNING_PRIVATE_KEY`、`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`）は GitHub Secrets に置く。owner（hamirilo）の鍵を使う（鍵 ID `22866C4837734BD8`。desktop-platform の playbook/updater-key.md）。秘密鍵のファイルとパスワードはパスワードマネージャーにもバックアップしておく。鍵が無いと tag のビルドは失敗し、それ以外のビルドはインストーラだけを作る。
