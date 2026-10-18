# stockroom へ送る

**状態**：対応済み（段階 1・2）。ロードマップのフェーズ 5。通信の決定は [ADR-0004](../decisions/adr-0004-sending-to-stockroom.md)（採用）。
受け取る側の計画は stockroom の `docs/FROM_APPS.md`。

困っていること：フィードの記事は流れていくもの。Feedability は古い記事を消す（`db::purge`）し、元のページも書き換わったり消えたりする。
スターは自分で外すまで残るが、残るのは Feedability の中だけで、ほかの資料と一緒に探したり整理したりできない。

Feedability はフロー、stockroom はストックと役割を分け、**取っておきたい記事だけを stockroom へ送る**。

## 決定

| 論点 | 決定 |
|---|---|
| 送るもの | **記事を Markdown 1 件**（タイトル・元の URL・フィード名・日付・本文）。リンクだけでなく本文ごと残す。元の記事が消えても読めるように |
| 受け口 | stockroom のブラウザ拡張の受付（127.0.0.1:41620 の `POST /import`）。stockroom 側に新しいポートは作らない |
| 送るのは | Rust（新しい `stockroom.rs`）。WebView からは通信しない（CSP は変えない） |
| stockroom のデータ | 直接触らない。`library.db` もライブラリのフォルダも読まない。書き込むのは stockroom だけ |
| stockroom が動いていない | 「stockroom を起動してください」とだけ出す。起動はしない |
| 段階 | まず **Markdown でコピー**（連携なし）で使い方を確かめ、次に **stockroom に送る** |

## 段階 1：Markdown でコピー（S）

連携のコードを書く前に、記事を stockroom に取っておく使い方がどれくらいあるかを確かめる。

- 記事の右クリックとメニューの 記事 › **Markdown でコピー**。中身は下の「Markdown の形」と同じ
- コピーは WebView の `navigator.clipboard.writeText`（クリック・キーの操作から呼ぶ）。動かなければ `tauri-plugin-clipboard-manager` にするが、capabilities の追加になるので人の判断を挟む
- stockroom の一覧に貼り付けると `.txt` のテキストになる（stockroom の docs/TEXT.md）。元の URL は本文に入っているが、詳細パネルの「取り込み元」には入らない。そこは段階 2 で埋める
- 本文の HTML を Markdown にするのは Rust（`sanitize::article_html` で安全化したあとの HTML から）。コマンド `article_markdown(id)` をコマンド表と `src/lib/api.ts` に足し、モック（`src/dev/mockBackend.ts`）も同じ PR で

## 段階 2：stockroom に送る（M）

- 記事の右クリック・メニューの 記事 › **stockroom に送る**・本文の上の帯のボタン。コマンド表に `article.stockroom` を足す（キーは実装時に決める。今の 1 文字のキーとぶつけない）
  - 本文の帯のボタンは既読・スター・あとで読む・ブラウザと同じ並びの最後に足し、既存のボタンの位置は変えない（UI Standard §10）
  - 送ったらトースト「stockroom に保存しました」。重複なら「stockroom にもうあります」
- 通信：`POST http://127.0.0.1:41620/import?name=<タイトル>.md&page=<記事の URL>&folder=<設定のフォルダ>&tag=<設定のタグ>`、本文は Markdown のバイト列。`Authorization: Bearer <トークン>`
- 接続：初めて送るときに `POST /pair?code=<4 桁>` に `X-Stockroom-Client: Feedability` を付けて送る。Feedability の画面にも同じ番号を出し、stockroom で許可してもらう。もらったトークンは `settings.json` の `stockroom.token` に置く
- 設定（⌘,）に「stockroom」の節：接続の状態（接続済み / 未接続・接続し直す）、送り先のフォルダ（`GET /folders`。既定は未分類）、付けるタグ（`GET /tags` から選ぶか新しく入れる。既定は無し）。docs/SETTINGS.md に足す
- 送り先の受付の状態ごとの言い方：

| stockroom の応答 | 出すこと |
|---|---|
| つながらない | stockroom を起動し、設定（⌘,）›「ブラウザ拡張」で「ブラウザ拡張からの保存を受け付ける」をオンにしてください（stockroom 0.28 で設定の「連携」が「ブラウザ拡張」と「Claude」に分かれた） |
| 401 | stockroom との接続が切れました。設定 › stockroom から接続し直してください |
| 503（ライブラリが開かれていない） | stockroom でライブラリを開いてください |
| 400（フォルダが無い） | 送り先のフォルダが見つかりません。設定 › stockroom で選び直してください |
| そのほか | stockroom の応答をそのまま |

## 実装で決めたこと

- キーは **Shift+S**（S のスターの隣。今の 1 文字のキーとぶつからない）。本文の帯のボタンは「ブラウザで開く」の右（最後）
- 接続の確認：Feedability が 4 桁の番号を作って画面に出し、`stockroom_pair(code)` が stockroom の答えを待つ（2 分まで）。送ろうとしたときに未接続なら、この確認を出し、接続できたらそのまま送る。設定の「接続する / 接続し直す」からも出せる（閉じると設定に戻る）
- **401 は、上の表の言い方でなく、鍵を消して接続の確認を出し直す**（もう一度送ると確認が出る）。stockroom で鍵を作り直したときに、設定を開かずに済むため
- 送る前に毎回 `GET /info` で、相手が stockroom で、ライブラリが開かれていることを確かめる（ADR-0004）
- 送っている間は「stockroom に送っています…」のトースト。結果で書き換える
- 鍵は `settings.json` の `stockroom.token`。画面には渡さない（`set_app_settings` は `stockroom` の部分を触らない）
- 送り先のフォルダとタグは、設定を開いたときに `GET /folders` と `GET /tags` で読む。stockroom が閉じていても、保存したフォルダの名前は出る
- Markdown でコピーは、クリック・キーの中で `ClipboardItem` に Promise を渡す（WebKit は操作の中でしか書かせない）。ブラウザの UI だけのモック（`npm run dev`）では、ブラウザの欄がクリップボードを許さないことがある
- コード：`src-tauri/src/markdown.rs`（Markdown と ファイル名）、`src-tauri/src/stockroom.rs`（通信）、`commands.rs` の `article_markdown` / `stockroom_*` / `send_to_stockroom`、`src/components/StockroomDialogs.tsx`
- stockroom 側：hamirilo/stockroom の `webimport.rs`（`Peer`。アプリは `Origin` 無しで `X-Stockroom-Client: Feedability`）

## Markdown の形

```markdown
# 記事のタイトル

- 元のページ：https://example.com/post/1
- フィード：Example Blog
- 公開：2026-10-17
- 保存：2026-10-17（Feedability から）

---

本文
```

- 本文は `sanitize::article_html` を通した HTML から作る（スクリプトなどは既に落ちている。相対リンクも記事の URL で解決済み）
- 画像は Markdown の画像にせず `[画像](URL)` のリンクにする（stockroom の画面は外の画像を読まない）
- 本文が要約だけの記事は要約のまま。フェーズ 4（全文取得）が入ったら、全文があればそちらを使う
- ファイル名に使えない文字（`/ \ : * ? " < > |` と制御文字）は `_` にし、長さは 100 文字までにする。タイトルが無い記事は `記事.md`
- HTML から Markdown への変換は **`htmd`**（Apache-2.0）を足す。表やネストしたリストも崩さずに残せるため。画像は変換の規則を差し替えて `[画像](URL)` にする
  - html5ever の 3 つ目の版（0.38。ammonia は 0.40、Tauri の `dom_query` は 0.39）と `markup5ever_rcdom` が入り、ビルドが少し重くなる。それを承知で選んだ
  - Recommendations の健全性チェックにかけ、結果を実装の PR に書く
  - 比べて外したもの：`html2md`（GPL-3.0）、`fast_html2md`（0.0.x で、正規表現で変換するので崩れた HTML に弱い）、`html2text`（Markdown でなく整形したテキストになる）、`dom_query` の上に自前で書く（依存は増えないが、変換を自分で保守する）

## あとで

- 記事の画像も画像として送る（`page` は記事の URL）。画像はすでに Rust が取っている（`image_data_url`）
- stockroom に URL（ブックマーク）の種類ができたら、「リンクだけ送る」を選べるようにする
- 送り先のフォルダを送るたびに選ぶ
- ブラウザ拡張（フェーズ 7）・MCP（フェーズ 8）を作るとき、stockroom の受付の作りを写す
