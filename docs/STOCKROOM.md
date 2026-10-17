# stockroom へ送る

**状態**：計画（未実装）。ロードマップのフェーズ 5。通信の決定は [ADR-0004](../decisions/adr-0004-sending-to-stockroom.md)（提案）。
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
| つながらない | stockroom を起動し、設定 › 連携 で受付をオンにしてください |
| 401 | stockroom との接続が切れました。設定 › stockroom から接続し直してください |
| 503（ライブラリが開かれていない） | stockroom でライブラリを開いてください |
| 400（フォルダが無い） | 送り先のフォルダが見つかりません。設定 › stockroom で選び直してください |
| そのほか | stockroom の応答をそのまま |

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
- HTML から Markdown への変換は crate を 1 つ足す。候補は `htmd`（Apache-2.0）。`html2md` は GPL なので使わない。Recommendations の健全性チェックにかけ、決めたものを PR に書く

## あとで

- 記事の画像も画像として送る（`page` は記事の URL）。画像はすでに Rust が取っている（`image_data_url`）
- stockroom に URL（ブックマーク）の種類ができたら、「リンクだけ送る」を選べるようにする
- 送り先のフォルダを送るたびに選ぶ
- ブラウザ拡張（フェーズ 7）・MCP（フェーズ 8）を作るとき、stockroom の受付の作りを写す
