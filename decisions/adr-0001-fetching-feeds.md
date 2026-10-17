# ADR-0001: フィードと記事の画像をアプリが取りに行く

**状態**：採用
**日付**：2026-10-17
**更新**：2026-10-17（サイトのアイコンを足した）

## 状況

Security Standard §6 は「ツールが自分から出る通信は updater の feed と project-context に書いた連携先だけ」「Web からのダウンロードが必要ならブラウザ拡張に取らせ、アプリは URL を取りに行かない」とし、Recommendations も `tauri-plugin-http` を「使わない」としている。

RSS リーダーは、購読したフィードを定期的に取りに行くこと自体が機能の中心で、ブラウザ拡張に任せる形では成り立たない（アプリを開いている間、ブラウザに関係なく更新したい）。記事の本文に入っている画像も、取りに行かなければ表示できない。

## 決定

アプリ（Rust 側、`src-tauri/src/fetch.rs`）が次の URL だけを HTTP(S) で取りに行く。

- ユーザーが購読したフィードの URL（と、サイトの URL を入れたときのそのページ。ページの `<link rel="alternate">` からフィードを探すため）
- 記事の本文に入っている画像（`<img src>`）
- 購読したフィードのサイトのアイコン。次の順で、最初に画像が得られたものを使う：(1) フィード自身が宣言したアイコン（Atom の `<icon>`）、(2) サイトのページの `<link rel="icon">` / `shortcut icon` / `apple-touch-icon`、(3) サイトの `/favicon.ico`、(4) フィードのロゴ（RSS の `<image>`、Atom の `<logo>`）。サイトはフィードの `site_url`、無ければフィードの URL のオリジン。1 つのフィードにつき 30 日に 1 回まで。上限 256 KB、SVG は使わない

守ること：

- 取りに行くのは Rust 側だけ。WebView からは外へ出ない（CSP の `connect-src` と `img-src` に外の出どころを足さない）。画像は Rust が取って `data:` URL で渡す
- プロキシは OS の設定（PAC を含む）をそのまま使う（`proxy.rs`、updater と同じ）。独自のプロキシ設定画面は作らない
- 受け取る大きさに上限を設ける（フィード 10 MB、画像 15 MB）。画像は中身で形式を判定し、SVG は渡さない
- http / https 以外の URL は取りに行かない
- テレメトリは送らない。User-Agent はアプリ名とバージョンとリポジトリの URL だけ
- HTTP クライアントは `tauri-plugin-http` でなく reqwest（updater が既に依存しているもの）を Rust から直接使う。フロントに HTTP の権限（capability）を渡さない

## 理由

- ブラウザ拡張に取らせる案：アプリを開いているだけで更新されるという RSS リーダーの基本が成り立たない。
- `tauri-plugin-http` を使う案：WebView に外への通信の権限を渡すことになり、capability と CSP を広げる必要がある。Rust 側に閉じたほうが信頼境界が狭い。
- 画像を CSP の `img-src https:` で直接読む案：記事を開くたびに WebView が外のサイトへ直接通信し、追跡用の画像も読まれる。Rust 経由にすると、通る形式とサイズを Rust で決められる。

## 影響

- `src-tauri/src/fetch.rs` が外への通信の唯一の入口。ほかのモジュールから reqwest を直接使わない。
- `tauri.conf.json` の CSP は Standard の既定より狭い（`asset:` と `frame-src` を外した）。外の出どころは足していない。
- README の「ネットワーク」に、どこへ通信するかを書く。

## 見直す条件

- フィード以外のもの（記事の全文取得、ポッドキャストの音声など）を取りに行きたくなったとき。
- 2 つ目のツールが同じようにアプリから URL を取りに行く必要が出たら、Security Standard §6 を直す PR を desktop-platform に出す。
