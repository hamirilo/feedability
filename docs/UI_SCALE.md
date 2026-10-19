# 表示の大きさ

困っていること：サイドバー・一覧・設定などの字が小さくて読みにくい。本文の文字は「Aa」（[READING.md](READING.md)）で変えられるが、それ以外の画面は変えられない。

## 決定

- 設定（⌘,）の「一般 › 外観」に **表示の大きさ** を置く。80 / 90 / 100（標準）/ 110 / 125 / 150 %。既定は 100 %（今までと同じ）
- 画面全体を同じ割合で大きくする（ブラウザのズームと同じ）。文字だけでなく余白・ボタン・一覧の行の高さも一緒に変わるので、詰まったり重なったりしない。主要な操作の位置関係も変わらない（UI Standard §10）
- 本文の文字の大きさ（Aa、⌘+ / ⌘−）とは別。本文は「表示の大きさ × 本文の文字の大きさ」になる。⌘+ / ⌘− は今までどおり本文の文字だけを変える
- アプリ全体の設定なので `settings.json` の `uiScale`（百分率の数）に置く。手で書き換えた値は 80〜150 に収めて使う

## 実装

| どこ | 何をする |
|---|---|
| `src-tauri/src/commands.rs` `apply_ui_scale` | WebView のズーム（`WebviewWindow::set_zoom`）を設定の値にする。起動時（`lib.rs` `main_window`）と設定を変えたとき。外観と同じく Rust から行うので、ページに WebView の権限（capability）を足していない |
| `src/lib/theme.ts` `applyUiScale` | `--ui-zoom` を `<html>` に置く。値は `localStorage`（`feedability.uiScale`）にも置き、次の起動で設定を読む前から使う。ブラウザ（`npm run dev`）では Rust が無いので CSS の `zoom` で代わりにする |
| `src/index.css` | macOS のウィンドウのボタン（信号）はズームで大きくならないので、その分の余白 `--traffic-lights` を `--ui-zoom` で割る |
| `SettingsDialog.tsx` | 「外観」の節の行（セレクト） |
