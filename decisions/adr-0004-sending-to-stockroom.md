# ADR-0004: 記事を 127.0.0.1 の stockroom へ送る

**状態**：提案
**日付**：2026-10-17

## 状況

Feedability の記事は流れていくもので、古いものは消える（`db::purge`）。元のページも書き換わったり消えたりする。
取っておきたい記事は、同じ作りのローカルの保管庫アプリ stockroom に入れたい（[docs/STOCKROOM.md](../docs/STOCKROOM.md)）。

Security Standard §6 は「ツールが自分から出る通信は updater の feed と project-context に書いた連携先だけ」とする。
[ADR-0001](adr-0001-fetching-feeds.md) は外（インターネット）への通信を `fetch.rs` だけに絞り、OS のプロキシを通す。
stockroom へ送るのは、どちらにも書いていない新しい通信先になる。

stockroom には、ブラウザ拡張から画像やファイルを受け取る受付（127.0.0.1:41620、token 必須、`Host` と `Origin` の検査、接続の許可）がすでにある。

## 決定

記事を Markdown にし、**Rust から 127.0.0.1:41620 の stockroom の受付へ送る**。

- 送り先は `127.0.0.1:41620` に固定する。ユーザーが入れた URL や、ほかのアドレスには送らない
- 送るのは新しい `src-tauri/src/stockroom.rs` だけ。`fetch.rs`（外への通信）とは分ける。reqwest を使うが、プロキシは通さない（`no_proxy`。会社の PAC が 127.0.0.1 を外へ回すと届かないため）
- WebView からは送らない（CSP の `connect-src` に 127.0.0.1 を足さない）。capabilities も足さない
- 認証は stockroom の受付の token。初回は stockroom の接続の許可（`/pair`、4 桁の番号を両方に出す）でもらい、`settings.json` に置く
- stockroom のデータ（`library.db`、ライブラリのフォルダ）は読まない・書かない
- 送るのはユーザーが「stockroom に送る」を選んだときだけ。起動時や更新のついでに stockroom へ問い合わせない

## 理由

- **stockroom の DB やフォルダに直接書く案**：stockroom が開いている間の同時書き込みと画面の更新漏れが起き、stockroom のスキーマに縛られる。stockroom の MCP でも同じ理由で採らなかった
- **新しいポートで Feedability か stockroom が待ち受ける案**：待ち受けポートは両方の「変えないもの」で、増やすと人の判断が 2 つ要る。stockroom の受付はすでに安全の検査と許可の画面を持っている
- **URL スキーム（`stockroom://`）で渡す案**：stockroom が閉じていても起動できるのは利点だが、本文は URL に載せるには長く、両方に deep link の capabilities が要る
- **クリップボードだけにする案**：連携は要らないが、元の URL が stockroom の「取り込み元」に入らず、送り先のフォルダとタグも選べない。最初の段階（Markdown でコピー）としては採る
- **`fetch.rs` に入れる案**：`fetch.rs` は外への通信の入口で、OS のプロキシ・サイズの上限・形式の判定がある。127.0.0.1 への送信はどれも当てはまらないので、分けたほうが「外への通信は `fetch.rs` だけ」が崩れない

## 影響

- 採用したら、`decisions/project-context.md` の「外との接続」に「ユーザーが選んだ記事を 127.0.0.1:41620 の stockroom へ送る」を足し、README の「ネットワーク」にも書く
- ARCHITECTURE.md の「外への通信は `fetch.rs` だけ」の横に、127.0.0.1 の stockroom へは `stockroom.rs` だけと書く
- stockroom 側で、拡張以外のアプリも接続の許可を求められるようにする変更が要る（stockroom の `docs/FROM_APPS.md`）。それまでは段階 1（Markdown でコピー）だけを出す
- stockroom の受付のポートや API が変わったら、こちらも合わせる。`GET /info` の `app` が `stockroom` であることを確かめてから送る

## 見直す条件

- stockroom 以外のアプリにも送りたくなったとき（送り先を選べるようにするなら、送り先ごとの許可を考え直す）
- ブラウザ拡張（ロードマップのフェーズ 7）や MCP（フェーズ 8）で Feedability 自身が待ち受けるようになったとき
