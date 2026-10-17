# 構成の方針

desktop-platform の Architecture Standard の層（shell / core / features）に沿う。フィードリーダーには種類ごとの機能（features）が無いので、shell と core の 2 層で足りている。

## 境界

| 層 | 中身 |
|---|---|
| shell | `proxy.rs`、updater（`src/lib/update.ts`）、log の配線（`lib.rs`、`src/lib/log.ts`）、single-instance、window-state、`settings.json`、タイトルバー（`src/lib/titleBar.ts`）。stockroom から写したもので、desktop-base ができたら差し替える |
| core | それ以外。データ（`db.rs`）、取得（`fetch.rs`）、解析（`feed.rs`、`opml.rs`）、本文の安全化（`sanitize.rs`）、画面 |

## データの流れ

```text
fetch.rs ──bytes──▶ feed.rs ──NewArticle──▶ db.rs（articles）
                                               │
                         get_article ◀─────────┘
                              │ sanitize.rs（ammonia、相対リンクを記事の URL で解決）
                              ▼
                     Reader.tsx（DOMParser で不活性に解析 → img の src を data-src へ → ページへ）
                              │ image_data_url（Rust が取得、中身で形式を判定）
                              ▼
                         data: URL の画像
```

- **外への通信は `fetch.rs` だけ**。WebView は外へ出ない（CSP の `connect-src` と `img-src` に外の出どころが無い）。理由は [ADR-0001](../decisions/adr-0001-fetching-feeds.md)
- **本文の HTML は 2 重に守る**：Rust の ammonia でスクリプト・イベント属性・スタイル・フォーム・iframe を落とし、CSP の `script-src 'self'` でもインラインのスクリプトを止める
- 記事の `content` には **フィードの HTML をそのまま** 保存し、表示するたびに安全化する。安全化の規則を変えたとき、古い記事にも新しい規則が効く
- リンクは `open_url` で OS のブラウザへ。WebView の中では遷移しない（中クリックと ⌘ クリックも止める）

## 記事の同一性と削除

- 記事はフィードの中の guid（RSS の `<guid>`、Atom の `<id>`）で見分ける（`UNIQUE (feed_id, guid)`）。guid が無いものはリンクとタイトルから、それも無いものは本文と日付から作る（`feed.rs` の `missing_id`、`stable_hash`）。feed-rs の既定はここで乱数の UUID になり、更新のたびに同じ記事が新着に見える
- 同じ guid の記事が来たら、タイトル・本文・URL は新しいものにし、既読とスターは残す
- `seen_at` は、その記事がフィードに最後に載っていた更新の時刻。304（変更なし）のときも全記事の `seen_at` を進める
- 削除（`db::purge`）は「既読・スター無し・`seen_at` が保持期間より前」だけ。フィードにまだ載っている記事を消すと、次の更新で未読として戻ってくるため

## 更新

- `refresh_feeds` は 6 本ずつ並行に取得する（`futures_util::buffer_unordered`）。データベースの mutex は書き込みの間だけ持ち、通信の間は持たない
- ETag / Last-Modified を保存し、次は条件付きで取りに行く
- 同時に 1 つだけ（タイマーと ⌘R が重なったら後から来たほうは「更新中です」で返し、フロントは黙る）
- 進捗は `refresh-progress` イベント、中止は `cancel_refresh`（まだ取りに行っていないフィードを飛ばす）
- 自動更新（起動の 3 秒後と設定の間隔）は失敗をトーストで知らせない。サイドバーの赤い印で分かる。手動の更新は知らせる

## 一覧と状態

- 一覧は問い合わせの結果をそのままストアに置く。既読・スターの変更は、表示中の行とひらいている記事だけその場で書き換え、一覧は取り直さない。「未読のみ」で読んだ記事が手元から消えないように、問い合わせには開いている記事の id（`keep`）も渡す
- 行は高さが一定（92px）で、`@tanstack/react-virtual` で描く
- 未読のみ・フォルダの開閉は表示の好みとして `localStorage`（`feedability.prefs`）に置く

## 操作

- 操作は `src/lib/commands.ts` の表に 1 度だけ書き、メニューバー・右クリック・キー処理・ショートカット一覧（?）をそこから作る
- フィードとフォルダの操作は「開いているもの」に対して働く。右クリックはまずそのフィード / フォルダを開いてからメニューを出す（メニューバーと同じ意味になる）
- 画面の上の操作（サイドバーの ＋・フォルダ・更新、一覧の 未読/すべて・検索・すべて既読、本文の 既読・スター・ブラウザ）は、何を開いていても同じ場所にある。使えないときは無効表示にする
