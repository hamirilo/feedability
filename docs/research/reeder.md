# Reeder 5 の機能（調べたもの）

Feedability の機能を決めるときの参考。決定ではない。

## 調べ方

- 対象：`/Applications/Reeder.app`（`com.reederapp.5.macOS`、5.6、ビルド 5060003、App Store 版、macOS 12 以上、x86_64 + arm64）
- 調べた日：2026-10-17
- 読んだもの：`Info.plist`、拡張（`PlugIns/*.appex`）の宣言、記事表示の `article.html` / `article.js` / `article.css`、Safari 拡張の `script.js`、実行ファイルから取った文字列（設定キー・クラス名・画面の文言）
- アプリは動かしていない。動きは確かめていない。名前から推し量っただけのものには「（推測）」と付けた

## 1. 同期先とデータ

- 同期サービス：この Mac だけ（On My Mac）、iCloud、Feedly、Inoreader、Feedbin、NewsBlur、The Old Reader、BazQux、FreshRSS、FeedHQ、Feed Wrangler、Fever、Google Reader 互換 API（`ServiceConnectorReaderAPI`）
- フィードとして読めるもの：RSS / Atom / JSON Feed のほか、Reddit（Popular / All / Rising など）、Mastodon、Bluesky、YouTube のチャンネル、Twitter（`TwitterAPI`）
- あとで読む：アプリに組み込み。タグが付けられ、入れるときにタグを選ぶ設定もある（`read-later.choose-tag-on-saving`）。外の送り先として Instapaper、Pocket、Safari のリーディングリスト
- OPML：読み込み・書き出し。読み込みでは入れるフィードを選べる（`ImportSelectController`）。全部は読み込めなかったときの知らせがある
- 同期の設定：自動更新の間隔（15 分 / 30 分 など、`syncing.interval`）、アーカイブを残す数（`syncing.max-archive`）、次の更新で全件を取り直す（`syncing.force-full`）、スリープから戻ったら同期、サーバー側の更新を省く（`syncing.skip-server-refresh`）
- 更新できないフィードの一覧（`FailedStreamsViewController`）
- データは Realm。スター付き・未読の記事の画像を前もって取っておく設定（`image-caching.starred` / `image-caching.unread`）

## 2. 記事の一覧

- 並び：新しい順 / 古い順（`app.item-order`）、フィードごとにまとめる（`app.item-group-by-feed`）
- 行の見た目を要素ごとに切り替える：縮小画像（`app.item-thumbnails`）、本文の冒頭（`app.item-hide-text-previews`）、ファビコン（`app.item-hide-favicons`）、ファビコンを白黒に（`app.grayscale-favicons`）。レイアウトをまるごと切り替える作りではない
- アーカイブ済みの記事を薄く表示（`app.item-dim-archived`）
- 絞り込み：未読だけ / スター付きだけ / すべて（`StateFilterBarItem`）
- まとめて既読：全部 / この記事より上 / この記事より下。全部のときに確認を出すか（`app.item-confirm-read-all`）、ボタンの位置（`app.item-read-all-button-position`）、全部既読にしたら次のフィードへ（`app.item-next-feed-after-read-all`）
- スクロールで通り過ぎたら既読（`reading.on-scroll`）
- スワイプ：行の左右に操作を割り当てる（`items.leading-swipe` / `items.trailing-swipe`。既読・スターの切り替えなど）
- 一覧の下端を引っ張ると次のフィードへ（`items.pull-bottom`）。上端を引っ張る更新は立方体が回るアニメーション（`CubeRefresher`）
- 検索と保存した検索（`SavedSearch`）。スター付きの中にタグを出す（`list.tags-in-starred`）
- 未読数のバッジ（Dock、`app.icon-badge`）

## 3. 記事の表示

- WKWebView に HTML のテンプレートを差し込む。JS から Swift へは `didRender` / `layout` / `imgTap` / `imgTapAndHold` / `keyDown` / `contextImageRef` で知らせる
- 文字：入っているフォントを何でも選べる（`article.font`）、大きさ（`article.font-size`、OS の既定に従う設定も）、行間（`article.line-height`）、本文の最大幅（`article.max.width`）
- タイトル：大きさの倍率、太さ、揃え、大文字にする、システムフォントを使う（`article.title.*`）
- コントラストを上げる（`article.increase-contrast`）、フィードの色を使う（`article.uses-color`、推測）
- リーダー表示（ページから全文を取る）：抜き出しを Mozilla Readability と Postlight Parser から選ぶ（`article.reader-view-library`）。記事ごとに切り替えるのが基本で、最初からリーダー表示で開く設定もある
- Bionic Reading：強調の度合い・間隔・濃さ（`bionic.fixation` / `bionic.saccade` / `bionic.opacity`。フレームワークを同梱）
- 埋め込みの整形：YouTube（サムネイルを出して押すと再生）、Reddit の投稿者・クロスポスト、Mastodon / Bluesky の投稿、ツイート、動画、画像ギャラリー、画像だけのリンク。本文に画像が無いときは縮小画像を先頭に差し込む
- 画像ビューア（`CoreImageViewer`）、画像を保存、動画のアドレスをコピー
- 本文の端を引っ張ると前後の記事へ（`article.pull-to-next-prev`）、ツールバーを自動で隠す（`article.auto-hide-toolbars`）
- 翻訳（`Translate`、推測）

## 4. 操作

- キーボードショートカットを利用者が割り当て直せる（`PrefShortcutsViewController`、`CoreShortcut`）。既に使われているキーとぶつかったときの知らせがある
- トラックパッド：上下左右のスワイプとピンチに操作を割り当てる（`gesture.swipe-*`、`gesture.pinch-open` / `pinch-close`）。ページ間のスワイプ
- ブラウザで開く：既定のブラウザ、Safari Technology Preview / Chrome / Edge / Brave / Firefox を選べる（`app.default-browser`）。後ろで開く（`browser.open-links-in-background`）。アプリの中のブラウザもある
- 共有：ツールバーに出す共有サービスを選ぶ（`app.toolbar-sharing-services`）。リンクをすべて書き出す
- 設定の画面：一般 / 外観 / 読み方 / アニメーション / ジェスチャ / ショートカット / サービス / 共有 / Bionic Reading（`Pref*.tiff`）

## 5. OS との連携

- URL スキーム：`feed:`、`reeder:`、`reeder5:`
- Safari 拡張「Subscribe with Reeder」：ページの `<link rel="alternate">` で type に rss / atom / application/json を含むもの、`rss.xml` で終わるリンク、YouTube のチャンネル（`/c/` `/channel/` `/user/`）を見つけたらボタンを有効にする
- Safari 拡張「Read Later with Reeder」：見ているページをあとで読むに入れる（ポップオーバー）
- 共有メニュー（share extension）：URL 1 件をあとで読むに入れる
- ショートカット.app（Intents）：あとで読むに入れる、一覧・タグ・本文を取る、開く
- ウィジェット（WidgetKit）
- Handoff：あとで読むの記事を開く（`com.reederapp.OpenReadLaterItem`）

## ROADMAP との対応

| ROADMAP | Reeder にあるもの | Feedability で考えること |
|---|---|---|
| 1 本文の表示の調整 | 文字の大きさ・行間・最大幅に加え、フォント選択、タイトルの書式、コントラスト | 1 は計画どおり。フォント選択は後で足せるよう、`feedability.prefs` のキーを分けておく |
| 2 既読にするタイミング | スクロールで既読（`reading.on-scroll`） | 選択肢に「スクロールで通り過ぎたら」を足すか決める。J / K の操作とどちらを主にするか |
| 3 一覧のレイアウト | レイアウトの切り替えではなく、縮小画像・冒頭・ファビコンを要素ごとに表示する設定。フィードごとにまとめる | 3 つのレイアウトのままか、要素ごとの切り替えにするか。要素ごとなら行の高さの組み合わせが増える |
| 4 全文取得 | 記事ごとのリーダー表示。抜き出しは 2 種類から選ぶ。最初からリーダー表示で開く設定 | 計画はフィードごとのオン / オフ。記事ごとに一度だけ取る使い方も入れるか。抜き出しは 1 つに決める方針のままでよい |
| 5 stockroom へ送る | 送り先を選べる共有（Instapaper / Pocket / リーディングリスト / OS の共有） | 対応なし（stockroom は Feedability 固有） |
| 6 `feed:` リンクと共有メニュー | `feed:` スキーム、share extension | 計画と同じ |
| 7 ブラウザ拡張 | Safari 拡張 2 つ（購読 / あとで読む） | フィードの見つけ方は上の 5 の条件（`link rel=alternate`、`rss.xml`、YouTube のチャンネル）が参考になる。拡張の中での判定は `feed::discover` に任せ、ページ側では軽く見るだけにする |
| 8 Claude（MCP） | ショートカット.app から あとで読むを操作 | 公開する操作の範囲の参考：Reeder は読む・入れる・開くだけで、購読の削除は出していない |

後回しの項目との対応：

| ROADMAP の後回し | Reeder |
|---|---|
| タグ | あとで読むにタグ。サービスのタグも表示 |
| 保存した検索 | ある（`SavedSearch`） |
| 更新できないフィードの一覧 | ある（`FailedStreamsViewController`） |
| 新着の OS 通知 | 見つからなかった（文字列の範囲では） |

ROADMAP に無いもの：

| 機能 | 規模の目安 | メモ |
|---|---|---|
| この記事より上 / 下を既読にする | S | コマンド表と右クリックに足す |
| 全部既読にしたら次のフィードへ | S | 設定に 1 項目 |
| 全部既読の前に確認する | S | 元に戻せる作りなら確認は要らないかもしれない |
| ショートカットの割り当て直し | M | コマンド表（`commands.ts`）があるので土台はある |
| トラックパッドのスワイプへの割り当て | M | |
| Dock の未読数のバッジ | S | |
| ファビコンを白黒に、既読を薄く | S | 表示の好み |
| SNS の投稿・YouTube を読みやすく整える | M〜L | YouTube のチャンネル購読は ADR-0001 の範囲で足せそう。埋め込みの再生は WebView から外へ出るので CSP に触れる |
| 外部の同期サービス | — | 対象外（ADR-0002） |
