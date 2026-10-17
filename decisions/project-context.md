# Project Context

AI と人が実装前に確認する **現在の前提**。機能要件、ロードマップ、画面仕様は `docs/` に置き、ここには書かない。
変更したら日付を更新する。変更の理由を将来説明する価値がある場合だけ、別の ADR を残す。

**更新日**：2026-10-17（updater の鍵を owner 専用に）

| 項目 | 現在の前提 |
|---|---|
| ツールの種類 | ウィンドウアプリ |
| UI の実装 | React + zustand + Tailwind 4 |
| データの置き場 | `app_data_dir()` の SQLite 1 ファイル（`feedability.db`）。設定は `app_config_dir()/settings.json`。構造は README「データ」。Standard の 2 択から外れる理由は [ADR-0002](adr-0002-data-in-app-data-dir.md) |
| 外との接続 | 購読したフィード（とそのページ、記事の画像）を HTTP で取りに行く。ブラウザ拡張・MCP・ローカルの待ち受けは無い。[ADR-0001](adr-0001-fetching-feeds.md) |
| 配布の形 | 単独リポジトリ（tag `v*`、feed `releases/latest`） |
| 識別子 | `com.local.feedability`（一度配布したら変えない）。`productName` は `Feedability` |
| updater の鍵 | owner（hamirilo）の鍵。stockroom（owner 2204601）とは別（playbook/updater-key.md「owner が違うツールは、その owner で 1 回だけ鍵を作る」）。鍵 ID `22866C4837734BD8`、秘密鍵は `~/.tauri/hamirilo.key` と GitHub Secrets の `TAURI_SIGNING_PRIVATE_KEY`（と `_PASSWORD`）。hamirilo のほかのツールもこの鍵を使う |
| desktop-base | 未採用（stockroom の f7e823a から写した：`proxy.rs`、`log.ts`、`titleBar.ts`、`ime.ts`、`update.ts`、`Toasts.tsx`、`vite.config.ts`、`tsconfig*.json`、`index.css` の `@theme`） |

## 該当しない Standard

- Optional「ローカル受付」：待ち受けない
- Optional「複数ツール同居リポジトリ」：単独リポジトリ
- Optional「トレイ常駐」：ウィンドウアプリ
- Security §3 asset protocol：ローカルのファイルを表示しないので使わない（`assetProtocol` を設定しない）
- UI §6 ドラッグ&ドロップ：ファイルを扱わない
- UI §5 OS の通知：10 秒を超える処理は更新だけで、結果はサイドバーに出る。通知は入れていない

## 逸脱

- [ADR-0001](adr-0001-fetching-feeds.md) フィードを取りに行く（Security §6「アプリは URL を取りに行かない」から）
- [ADR-0002](adr-0002-data-in-app-data-dir.md) データを `app_data_dir()` の SQLite に置く（Architecture §6「フォルダ 1 つで完結」と Project Context の 2 択から）
