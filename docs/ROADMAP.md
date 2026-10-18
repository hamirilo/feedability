# ロードマップ

v0.1.0 のあとの機能追加の計画。優先するのは **読む体験** と **取り込み・連携**。整理・検索と運用・品質は後回しにする。

変えない方針：

- データはこの PC の中だけ。アカウントもサーバーも持たない（[ADR-0002](../decisions/adr-0002-data-in-app-data-dir.md)）
- 外への通信は Rust の `fetch.rs` だけ。WebView から外へ出さない（[ADR-0001](../decisions/adr-0001-fetching-feeds.md)）
- 表示を切り替えても、主要な操作の位置を変えない（desktop-platform の UI Standard §10）

規模の目安：S = 1〜2 日、M = 3〜5 日、L = 1〜2 週間以上。

対応済み（計画の外から入ったもの）：**外観（ライト / ダーク、既定は OS）**（[THEME.md](THEME.md)）、**あとで読む**（[LATER.md](LATER.md)）。フェーズ 7 の「拡張の中であとで読む」は、この「あとで読む」に入れる形にする。

## 一覧（優先順）

| フェーズ | 項目 | 区分 | 規模 | 人の判断が要るもの |
|---|---|---|---|---|
| 0 | v0.1.0 のリリースと実機での確認 | — | S | — |
| 0.5 | 基本機能（B1〜B5。下の表。対応済み） | 読む | M | —（B1 の取得先は ADR-0001 に足した） |
| 1 | 本文の表示の調整（文字の大きさ・行間・本文の幅）（対応済み、[READING.md](READING.md)） | 読む | S | — |
| 2 | 既読にするタイミングの設定（対応済み、[READING.md](READING.md)） | 読む | S | — |
| 3 | 一覧のレイアウト（標準・コンパクト・画像付き）（対応済み、[LIST_LAYOUT.md](LIST_LAYOUT.md)） | 読む | M | — |
| 4 | 全文取得（要約だけのフィードの本文をページから取る）（対応済み、[FULL_TEXT.md](FULL_TEXT.md)） | 読む | L | —（決定済み：ADR-0001 を更新、`dom_smoothie`） |
| 5 | 記事を stockroom へ送る（対応済み、[STOCKROOM.md](STOCKROOM.md)） | 連携 | S + M | — （決定済み：[ADR-0004](../decisions/adr-0004-sending-to-stockroom.md)、`htmd`、stockroom の受け口） |
| 6 | `feed:` リンクと .opml ファイルから購読（対応済み、[OPEN_WITH.md](OPEN_WITH.md)。共有メニューは見送り） | 連携 | M | —（決定済み：`tauri-plugin-deep-link`。capabilities は結局足していない） |
| 7 | ブラウザ拡張から購読・記事を送る（対応済み、[EXTENSION.md](EXTENSION.md)） | 連携 | L | —（決定済み：127.0.0.1:41630、[ADR-0005](../decisions/adr-0005-browser-extension-receiver.md)） |
| 8 | Claude（MCP）から読む・整理する | 連携 | L | AI に公開する操作の範囲（ポートは 41631 を予定） |

後回し（順番は決めない）：

- **整理・探す**：フィードのドラッグでの並べ替えとフォルダ移動（pointer イベント。UI Standard §6）、タグ、保存した検索、全文検索（SQLite FTS5。1 万件を超えたら LIKE では遅くなる）
- **運用・品質**：macOS の公証（Distribution Standard §3 からの逸脱になる。ADR が要る）、Windows のコード署名、更新できないフィードの一覧（URL の付け替えはフェーズ 0.5 の B5）、失敗したフィードの間隔を空けた再試行、新着の OS 通知

---

## フェーズ 0：v0.1.0 のリリースと実機での確認（S）

- `v0.1.0` のタグを push し、draft のリリースを確認して publish する
- macOS（Apple Silicon）と Windows で、インストール・購読・更新・本文の画像・リンク・OPML・アプリ内アップデートを一度ずつ試す
- 実在のフィードで確かめる：日本語のブログ（はてな、note、Zenn）、Atom（GitHub のリリース）、Shift_JIS の古いサイト、要約だけのフィード
- 見つかった不具合は v0.1.x で直す

## フェーズ 0.5：基本機能（M）

RSS リーダーとして毎日使うときの土台。読む体験（フェーズ 1 以降）より先に揃える。Reeder 5 と比べて足りないものから選んだ（[research/reeder.md](research/reeder.md)）。フェーズ 1（本文の表示の調整）とフェーズ 2（既読にするタイミング）も基本機能に入るが、内容は各フェーズのとおり。

| # | 項目 | 規模 | 中身 |
|---|---|---|---|
| B1 | ファビコン | M | サイドバーと一覧の行にフィードのアイコン。取得は `fetch.rs` 経由、保存して使い回す。サイトの favicon を取りに行くのが ADR-0001 の「フィードとそのページ」に入るかを先に決める（対応済み、[FAVICON.md](FAVICON.md)） |
| B2 | 記事の並び順 | S | 新しい順 / 古い順。表示の好み。あとで読むは入れた順のまま（対応済み、[LIST.md](LIST.md)） |
| B3 | 次の未読へ・次のフィードへ | S | N で次の未読の記事（一覧に無ければ未読のある次のフィードへ）。Shift+J / Shift+K で次 / 前のフィード（対応済み、[LIST.md](LIST.md)） |
| B4 | ここより上 / 下を既読にする | S | 記事の右クリックとメニュー。すべて既読にしたら次のフィードへ移る設定（既定はオフ）（対応済み、[LIST.md](LIST.md)） |
| B5 | フィードの情報と URL の変更 | S〜M | URL・サイト・最終更新・エラーの中身を見る。URL を付け替える（後回しの「URL の付け替え」をここへ）（対応済み、[FEED_INFO.md](FEED_INFO.md)） |

順番：B2・B3・B4（小さく、Rust の変更が少ない）→ B5 → B1。

ペインの幅の変更（[LAYOUT.md](LAYOUT.md)）も基本機能として対応済み。

## フェーズ 1：本文の表示の調整（S）

- 本文の文字の大きさ（3〜5 段階）、行間、本文の最大幅を変えられるようにする
- 置き場所：本文の上のボタン（既読・スター・ブラウザ）と同じ帯に「Aa」のボタンを 1 つ足し、押すと小さなパネルを開く。帯の高さと既存のボタンの位置は変えない
- キー：⌘+ / ⌘- で文字の大きさ（コマンド表に足す）
- 保存：表示の好みとして `localStorage`（`feedability.prefs`）

## フェーズ 2：既読にするタイミング（S）

- 設定（⌘,）に「記事を既読にする」：開いたとき（今の動き）/ 開いて 2 秒後 / 自分で既読にする（M のみ）
- 「開いて 2 秒後」は J / K で通り過ぎた記事を未読のまま残すため。タイマーは別の記事を開いたら取り消す
- 保存：`settings.json`（`markReadOn`）。docs/SETTINGS.md に足す

## フェーズ 3：一覧のレイアウト（M）

対応済み。決めたことは [LIST_LAYOUT.md](LIST_LAYOUT.md)（切り替えは一覧の上の 1 行目の右端、`user_version` は 4）。以下は計画のときのもの。

- 標準（今）/ コンパクト（タイトルだけ 1 行）/ 画像付き（左に記事の最初の画像の縮小版）
- 切り替えは一覧の上の行のセグメントに 1 つ足す。レイアウトによって行の高さは変わるが、上の操作の位置は変えない
- 画像付きの縮小版：記事を保存するときに本文の最初の `<img>` の URL を `articles.thumb_url` に入れておく（スキーマは `user_version` 2 で列を足す）。画像は本文と同じく Rust が取り、小さくしてから渡す（image crate。Recommendations の既定）
- 1 万件で固まらないこと（`examples/seed.rs` で確かめる）。縮小版は見えている行の分だけ取る

## フェーズ 4：全文取得（L）

対応済み。決めたことは [FULL_TEXT.md](FULL_TEXT.md)（抜き出しは `dom_smoothie`、`user_version` は 5、手元のネットワークの IP は取らない）。以下は計画のときのもの。

要約だけを配るフィード（ニュースサイトに多い）を、アプリの中で全文で読めるようにする。

- フィードごとのオン / オフ（右クリック「全文を取得する」）。既定はオフ
- オンのフィードは、新しい記事を保存するときに記事のページを取得し、本文を抜き出して `articles.full_content` に入れる。表示は今と同じく ammonia で安全化してから
- 抜き出しは Rust の readability 系の crate を使う予定（候補を Recommendations の健全性チェックにかけ、1 つに決めて PR に書く）
- 守ること：ページの取得は `fetch.rs` 経由、上限 5 MB、並行数はフィードの更新と同じ枠の中。失敗したら要約のまま見せる
- **ADR-0001 の見直し**：今は「購読したフィードとそのページ、記事の画像」だけを取りに行く。記事のページを取りに行くのは範囲の拡大なので、ADR を更新してから実装する

## フェーズ 5：記事を stockroom へ送る（S + M）

対応済み。決めたことは [STOCKROOM.md](STOCKROOM.md)（キーは Shift+S、stockroom 側は hamirilo/stockroom の `Peer`）。以下は計画のときのもの。

Feedability はフロー、stockroom はストック。取っておきたい記事だけを、本文ごと Markdown にして stockroom へ送る。詳しくは [STOCKROOM.md](STOCKROOM.md)。

- 段階 1（S）：記事を **Markdown でコピー**。stockroom に貼り付けて使い方を確かめる。stockroom 側の変更は要らない
- 段階 2（M）：**stockroom に送る**（右クリック・メニュー・本文の帯）。stockroom のブラウザ拡張の受付（127.0.0.1:41620 の `POST /import`）に、Rust の `stockroom.rs` から送る。送り先のフォルダとタグは設定で選ぶ
- stockroom 側の受け口：拡張以外のアプリも接続の許可（`/pair`）を求められるようにする。stockroom の `docs/FROM_APPS.md`。URL の種類は待たない
- **決定済み**（2026-10-17）：127.0.0.1 の stockroom への通信（[ADR-0004](../decisions/adr-0004-sending-to-stockroom.md) を採用）、HTML から Markdown への変換は `htmd`、stockroom の受付を拡張以外のアプリにも開く（トークンは拡張と共通）
- stockroom が動いていないときは「stockroom を起動してください」とだけ言う。Feedability は stockroom のデータを直接触らない

## フェーズ 6：`feed:` リンクと共有メニューから購読（M）

対応済み。決めたことは [OPEN_WITH.md](OPEN_WITH.md)（Rust だけで受け取るので capabilities は足していない。共有メニューは見送り）。以下は計画のときのもの。

- `feed:https://…` のリンクや `.opml` ファイルを開くと、Feedability の「フィードを追加」が URL 入りで開く
- macOS は URL スキームの登録、Windows はプロトコルの登録。`tauri-plugin-deep-link` を使う（Recommendations に無いので健全性チェックをする）
- **人の判断が要る**：capabilities に deep link の権限を足す（CLAUDE.md「変えないもの」）。受け取った URL は `fetch::normalize_url` と同じ検証を通す
- 二重起動の防止（single-instance）と組み合わせ、2 回目の起動の引数を今のウィンドウに渡す

## フェーズ 7：ブラウザ拡張から購読・記事を送る（L）

対応済み。決めたことは [EXTENSION.md](EXTENSION.md)（フィードに属さない記事は「保存したページ」のフィードの行、`user_version` は 6、ページは拡張が HTML で送る）。以下は計画のときのもの。

- 見ているページのフィードをワンクリックで購読する（ページからの探索は今の `feed::discover` を使う）
- 拡張の中で、フィードに無いページを「あとで読む」に入れる（フィードに属さない記事。`feeds` に「保存した記事」の行を 1 つ作るか、`articles.feed_id` を NULL 可にするかは実装時に決める）
- desktop-platform の Optional Standard「ローカル受付」に従う：127.0.0.1 だけ、token 必須、`Host` と `Origin` の検査、ペアリングの確認、既定はオフ
- **人の判断が要る**：待ち受けポート。stockroom の拡張と同じ作りなら、拡張の土台を stockroom から写す

## フェーズ 8：Claude（MCP）から読む・整理する（L）

- まずは読むだけ：未読の一覧、記事の本文、フィードの一覧、検索。「今日の未読を要約して」に答えられるようにする
- 次に、元に戻せる変更だけ：既読にする、スターを付ける、フォルダへ移す。変更は記録し、あとでまとめて戻せるようにする（Architecture Standard §7）。購読の削除は公開しない
- desktop-platform の Optional Standard「ローカル受付」と、stockroom の `mcp/`（tiny_http の上に JSON-RPC）の形に揃える
- **人の判断が要る**：待ち受けポート、Claude に公開する操作の範囲
