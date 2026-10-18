# ブラウザ拡張

**状態**：対応済み。ロードマップのフェーズ 7。待ち受けの決定は [ADR-0005](../decisions/adr-0005-browser-extension-receiver.md)（127.0.0.1:41630、既定はオフ。2026-10-18 に人が決めた）。
拡張の土台は stockroom の `extension/` から写した（接続の確認、`shared.js`、`style.css`）。

困っていること：ブラウザで見つけたフィードを購読するには、ページの URL をコピーしてアプリに貼る。フィードに無いページは、あとで読むに入れられない。

## 使い方

1. 設定（⌘,）›「ブラウザ拡張」で「ブラウザ拡張からの購読・あとで読むを受け付ける」をオンにする
2. 「拡張機能のフォルダを用意して表示」→ Chrome / Edge の `chrome://extensions` で「デベロッパー モード」をオン →「パッケージ化されていない拡張機能を読み込む」でそのフォルダを選ぶ（このフォルダの拡張は `config.json` の鍵でそのまま接続される）
3. 拡張のアイコンを押すと、上にアプリとの接続の状態（接続中・未接続・応答なし）、その下に見ているページと次の 2 つが出る：
   - **フィードを購読**：ページが `<link rel="alternate">` で案内しているフィード（RSS・Atom・JSON Feed）が形式の札つきで並ぶ。「購読」で Feedability が購読する（「フィードを追加」と同じ処理）。案内が無ければ「このページの URL で探して購読」
   - **あとで読む**：「このページをあとで読むに入れる」で、ブラウザに見えているページの HTML を送る。Feedability が本文を抜き出し（全文取得と同じ `dom_smoothie`）、あとで読むの最後に入れる
4. いちばん下のスイッチ（拡張の `chrome.storage.local` に残る）：
   - **追加したら Feedability を表示**（既定はオン）：購読・あとで読むのあと、アプリの窓が前に出て、購読したフィード / あとで読むのその記事を開く
   - **購読するとき全文取得もオンにする**（既定はオフ）：購読したフィードの全文取得（[FULL_TEXT.md](FULL_TEXT.md)）をオンにする

アプリが応答しない（起動していない、受け付けがオフ）とき、ポップアップは「Feedability が応答しません」と出し、次の 2 つを出す：

- **Feedability を起動**：`feed:open` を開く。OS がアプリを起動するか前に出す（`feed:` のスキームは [OPEN_WITH.md](OPEN_WITH.md)。`open` は URL ではないので、アプリは何もせずに起動するだけ）
- フィードの「購読」は **「アプリで開く」** に変わり、`feed:<フィードの URL>` を開く。アプリが起動して「フィードを追加」がその URL 入りで開く（受け付けがオフでも購読できる）
- あとで読むは、ページの HTML を受付へ送るので、アプリが応答するまで押せない

`feed:` をほかのリーダーが受け持っているパソコンでは、そちらが開く（OS の設定で Feedability を選ぶ）。

別の場所から入れた拡張（開発中の `extension/` を直接読み込んだものなど）は、拡張のアイコン →「アプリと接続」。Feedability に 4 桁の番号の確認が出るので、同じ番号なら「許可する」。

## 決めたこと

| 論点 | 決定 |
|---|---|
| 受付 | `extension.rs`。`127.0.0.1:41630`、設定でオンの間だけ。`Host` の検査、拡張以外の `Origin` は拒否、Bearer の鍵（ADR-0005） |
| 受け付けるもの | `POST /subscribe?url=`（購読）、`POST /later?url=&title=`（本文はページの HTML、5 MB まで）、`GET /info`、`POST /pair`。`/subscribe` と `/later` はポップアップのスイッチを `show=1`（窓を前に出してそれを開く）・`fullText=1`（`/subscribe` だけ。全文取得をオン）で受け取る（`extension::Options`） |
| ページの取得 | **アプリはページを取りに行かない**。拡張がブラウザの Cookie のまま表示しているページの HTML を送る（ログインが要るページもそのまま入る） |
| フィードに属さない記事 | 「保存したページ」というフィードの行（`feeds.saved = 1`、スキーマ v6）に入れる。サイドバー・更新・アイコン・OPML の書き出しには出さない。一覧ではフィード名が「保存したページ」 |
| 保存したページの状態 | 既読（ニュースではないため）・あとで読むの最後。同じ URL をもう一度送ると、本文を新しくし、あとで読むの場所はそのまま |
| 抜き出せなかったとき | タイトルとリンクだけの記事として入れる（JavaScript で描くページなど） |
| 鍵 | `settings.json` の `extension.token`。窓には渡さない。「すべての拡張機能との接続を解除」で作り直す（書き出したコピーの `config.json` も新しくする） |
| 拡張の権限 | `storage`・`scripting`・`activeTab` と、`http://127.0.0.1:41630/*` だけ。ページを読むのはポップアップを開いたときだけ（`activeTab`）。stockroom の拡張と違い、すべてのページで動くスクリプトは無い |
| 知らせ | 受け取ったら窓に「『…』を購読しました（ブラウザから）」「『…』をあとで読むに入れました」と出し、サイドバーと一覧を読み直す。`show=1` なら Rust が窓を前に出し（`show_main_window`）、窓は購読したフィードか、あとで読むのその記事を開く（`extension-changed` の `feedId` / `laterId`） |
| アプリが応答しないとき | `feed:` のリンクで起動する（ネイティブ メッセージングは使わない。拡張の権限と、OS ごとのホストの登録が要るため）。購読は `feed:<URL>` で「フィードを追加」へ。あとで読むは起動を待つ |

## 守ること

- 送られてきた HTML は、ほかの記事と同じく表示のたびに `sanitize::article_html` を通す
- URL は http / https だけ。それ以外は 400
- 購読の URL を取りに行くのは ADR-0001 の範囲（フィードとそのページ）のまま

## 実装

- Rust：`loopback.rs`（stockroom から写した待ち受け）、`extension.rs`（受付。`Host` の trait で Tauri なしにテスト）、`commands.rs` の `ExtensionHost`・`extension_status` / `set_extension_receiver` / `reset_extension_token` / `install_extension` / `answer_extension_pair`、`db::save_page`、`feed::page_article`
- 拡張：`extension/`（`manifest.json`・`popup.*`・`connect.*`・`shared.js`・`style.css`・`icons/`）。アプリに同梱（`bundle > resources`）
- フロント：`src/components/ExtensionSettings.tsx`（設定の節と接続の確認）、`App.tsx` の `extension-pair` / `extension-pair-end` / `extension-changed`（`show=1` のときはそのフィード / 記事へ移る）
- モック：`window.__MOCK_EXTENSION__("pair")`、`("later")`（「表示」がオンのときと同じく、あとで読むのその記事へ移る）

## あとで

- 右クリックのメニュー（リンクをあとで読むに入れる）
- Chrome ウェブストアでの配布
- 購読するときにフォルダを選ぶ
