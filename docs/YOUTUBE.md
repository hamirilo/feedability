# YouTube のチャンネル

困っていること：YouTube のチャンネルを購読すると、記事（動画）の本文と一覧の冒頭が空になる。チャンネルの URL を入れても、ページによってはフィードが見つからない。

調べたことは [research/feed-readers.md](research/feed-readers.md) の「YouTube のチャンネル」。

## 決定

- **本文**：本文（Atom の `content` / `summary`、RSS の `description`）が空で、Media RSS（`<media:group>`）があるときは、その縮小画像と `<media:description>` から本文を作る。YouTube の動画はこれにあたる
  - 先頭に縮小画像。押すと記事の URL（動画のページ）をブラウザで開く。アプリの中では再生しない（WebView から外へ出さない。CSP は変えない）
  - 説明文がテキスト（`type="plain"`、既定）なら、空行で段落、改行で `<br>`、`http(s)://` の URL をリンクにする。`type="html"` ならそのまま。どちらも表示の前に今までどおり ammonia で安全化する
  - 一覧の冒頭（`summary`）もこの本文から作る
  - YouTube に限らず、`<media:group>` にしか中身の無いフィードすべてに効く。本文のあるフィードは今までどおり
  - 更新のたびに既存の記事の本文も書き直すので、すでに購読しているチャンネルも次の更新で本文が入る
- **購読（URL から）**：フィードを追加・URL の付け替え・ブラウザ拡張からの購読で、YouTube の URL を次のように扱う
  - URL だけでフィードが決まるものは、ページを取らずにフィードの URL にする：`/channel/UC…` → `videos.xml?channel_id=`、`/user/名前` → `videos.xml?user=`、`/playlist?list=…` → `videos.xml?playlist_id=`。EU などで同意のページへ飛ばされても購読できる
  - それ以外（`/@名前`、`/c/…`、動画のページ `watch?v=`、`youtu.be/…`）はページを取り、まず `<link rel="alternate">` を探す。無ければページの中のチャンネル ID（`"externalId"`、無ければ最初の `"channelId"`）からフィードの URL を作る。動画のページなら、その動画のチャンネルを購読する
  - 見つからないとき、YouTube のページなら「チャンネルの URL（`/channel/UC…`）でも追加できます」と添える

## 決めていないこと

- ショート動画を除いて購読する（再生リスト `UULF…` を使う方法。公式でない）
- YouTube がときどき 404 / 500 を返すときの、間隔を空けた取り直し（ROADMAP の後回しの「失敗したフィードの間隔を空けた再試行」）
- フィードのアイコン：チャンネルのページの `<link rel="icon">`（YouTube のアイコン）になる。チャンネルの顔写真にはしていない

## 実装

- `feed.rs`：`article` で本文が空なら `media_body`（縮小画像 + `text_html` / `linkify`）。`youtube_feed`（URL から）、`youtube_feed_in_page`（ページの中のチャンネル ID から）
- `commands.rs`：`fetch_feed_or_page` の最初に `youtube_feed`、`discover` で見つからなければ `youtube_feed_in_page`
- 画面・API・モックは変わらない。外へ取りに行くのは購読したフィードとそのページだけで、ADR-0001 の範囲のまま
