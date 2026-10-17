# 外観（ライト / ダーク）

困っていること：ダークだけだと、明るい部屋や昼間に読みにくい。OS をライトにしている人には、アプリだけ暗いのは目が疲れる。

## 決定

- 設定（⌘,）の「外観」で **OS に合わせる / ライト / ダーク** を選ぶ。既定は **OS に合わせる**。OS を切り替えると、アプリも動いている間に追従する
- desktop-platform の UI Standard §1 は「ダークを既定にする」。既定を OS に合わせるのはそこからの逸脱なので [ADR-0003](../decisions/adr-0003-theme-follows-os.md) に残す
- 色はトークンの差し替えだけで変える（UI Standard §1「ライトテーマは必要になったらトークンの差し替えで足す」）。コンポーネントは今までどおりトークンだけを使う

## 実装

| どこ | 何をする |
|---|---|
| `src/index.css` | `@theme` の値がダーク。`:root[data-theme="light"]` で同じトークンにライトの値を入れる。`--color-hover`（ポインタの下の色）と `--color-scrollbar` も足した。白の半透明（`hover:bg-white/10`）はライトで見えないので、`hover:bg-hover` に置き換えた |
| `src/lib/theme.ts` | 設定から `data-theme` を決める。OS に合わせるときは `prefers-color-scheme` を見て、変わったら追従する。決めた値を `localStorage`（`feedability.theme`）にも置き、次の起動で設定を読む前から同じ色で描く |
| `src-tauri/src/commands.rs` `apply_theme` | ウィンドウのテーマ（タイトルバー・スクロールバー・OS の部品）と背景色を合わせる。起動時（`lib.rs` `main_window`）と設定を変えたとき。Rust から行うので、ページにウィンドウの権限（capability）を足していない |
| `tauri.conf.json` | ウィンドウの `theme: "Dark"` と `backgroundColor` を外し、`create: false` にして Rust で作る |

本文（`.article`）のスタイルもトークンだけで書いてあるので、そのまま両方の外観で読める。

## ライトの色

| トークン | ライト | ダーク（既定） |
|---|---|---|
| `--color-bg` | `#f7f7f8` | `#1b1c1f` |
| `--color-panel` | `#eeeef0` | `#232428` |
| `--color-raised` | `#ffffff` | `#2c2d32` |
| `--color-line` | `#dcdce1` | `#34353b` |
| `--color-fg` | `#1d1d20` | `#e6e6e8` |
| `--color-dim` | `#6b6c74` | `#9a9ba3` |
| `--color-accent` | `#2b6de8` | `#4c8dff` |
| `--color-danger` | `#d63c3c` | `#ef5d5d` |
| `--color-success` | `#1d9a5a` | `#3ec27f` |
| `--color-star` | `#d99a00` | `#f5c451` |
