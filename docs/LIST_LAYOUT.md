# 一覧のレイアウト（標準・コンパクト・画像付き）

困っていること：一覧の 1 行が 3 段（フィード・題名・要約）で固定。題名だけ素早く流し読みしたいときは行が高すぎ、写真の多いフィード（ニュース・写真ブログ）では中身が想像しにくい。

## 決定

- **3 つのレイアウト**：表示の好みとして `localStorage` の `feedability.prefs`（`listLayout`：`standard` / `compact` / `thumbs`）に置く。範囲外の値は標準に戻す。すべての表示（すべて・フィード・フォルダ・スター付き・あとで読む）に同じものが効く
  - **標準**（既定）：今まで通り。フィード・題名・要約 2 行。行の高さ 92px
  - **コンパクト**：1 行。未読の印・フィードのアイコン・題名・あとで読む / スター・日付。フィード名はアイコンとツールチップだけ。行の高さ 34px
  - **画像付き**：標準の左に 64px 四方の縮小版。画像の無い記事は標準と同じ（縮小版の場所を空けない）。行の高さは標準と同じ 92px
- **切り替え**：一覧の上の 1 行目（題名と件数の行）の右端に、3 つのアイコンのセグメント。2 行目（検索・並び順・すべて既読）と 3 行目（未読 / すべてのタブ）の位置は変えない（UI Standard §10）。メニュー 表示 › 一覧のレイアウト にも同じ 3 つ（チェック付き）。キーは付けない
  - 切り替えても一覧は問い合わせ直さない。選んでいる記事が見える位置に戻す
- **記事の画像を決める順**（`feed.rs` の `thumbnail`、記事を保存するとき）：
  1. フィードが付けた縮小版（`<media:thumbnail>`）。feed-rs は `<media:content>` と一緒のときだけ読む（単独の `<media:thumbnail>` は落ちる。困るフィードが出たら考える）
  2. フィードが添付した画像（`<media:content>` / `<enclosure>` の `image/*`）
  3. 本文の最初の `<img>`。幅か高さが 2px 以下（アクセス解析の 1px 画像）は飛ばす
  - 相対 URL は記事の URL で解決する（本文の表示と同じ）。http(s) だけ。SVG は使わない
- **保存**：`articles.thumb_url`（`user_version` 4 で列を足した）。更新のたびに記事を上書きするので、列を足す前の記事も、フィードにまだ載っていれば次の更新で入る
- **縮小版の取得**：見えている行の分だけ、Rust の `thumbnail_data_url` で取る。本文の画像と同じく `fetch.rs` 経由（ADR-0001 の「記事の画像」の範囲）で、WebView は外へ出ない
  - Rust が 128px 四方（表示の 2 倍）に中央で切り抜いて縮め、JPEG（透明のある画像は PNG）の `data:` URL で渡す（`thumb.rs`、`image` crate）。小さい画像は拡大しない
  - `image` crate が Rust だけでは読めない形式（AVIF）は、256 KB までならそのまま渡す（表示で縮める）。それより大きければ縮小版なし
  - 同時に 4 つまで。順番待ちの間に画面の外へ出た行は、待ちから外す（速くスクロールしても待ちが溜まらない）。取れたものは 400 件まで覚える。取れなかったものも覚えて、アプリを開き直すまで取り直さない。取れるまで・取れなかったときは灰色の四角

## 実装

- Rust：`feed.rs` の `thumbnail`（と `<img>` を拾うための `tags`。`link_tags` も使う）、`db.rs` の `NewArticle.thumb_url` / `ArticleRow.thumb_url` と移行 4、`thumb.rs`、`commands.rs` の `thumbnail_data_url`（`image_data_url` と取得を共有する `fetch_image`）
- フロント：`lib/listLayout.ts`（3 つの定義と行の高さ、検証）、`lib/thumbs.ts`（順番待ちと覚えておく分）、`store.ts` の `listLayout` / `setListLayout`、`ArticleList.tsx` の `LayoutSegment` / `RowBody` / `Thumb`
- コマンド表：`list.layout.standard` / `list.layout.compact` / `list.layout.thumbs`。メニューは `menuBar.ts` の 表示 › 一覧のレイアウト
- 1 万件：`examples/seed.rs` は 3 件に 2 件の記事に画像の URL を入れる。一覧の問い合わせは 1 万件で 34ms ほど（release）
