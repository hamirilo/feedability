# `feed:` リンクと .opml ファイルから開く

**状態**：対応済み。ロードマップのフェーズ 6。`tauri-plugin-deep-link` を足すことは 2026-10-18 に人が判断した（健全性チェックは PR に）。

困っていること：ブラウザで見つけたフィードを購読するには、URL をコピーして「フィードを追加」に貼る。ほかのリーダーから書き出した OPML も、メニューから選び直さないと読めない。

## 決定

| 論点 | 決定 |
|---|---|
| 受け取るもの | `feed:` のリンク（URL スキーム）と、拡張子 `.opml` のファイル |
| `feed:` の読み方 | `feed:https://…` / `feed:http://…` は続く URL。`feed://host/path` は `http://host/path`。`feed://https://…` は中の URL。どれも `fetch::normalize_url` を通し、http / https 以外は捨てる |
| リンクを開いたとき | 「フィードを追加」をその URL を入れた状態で開く。**自動では購読しない**（リンクを書くのは Web ページなので） |
| .opml を開いたとき | 「『<ファイル名>』の購読を読み込みますか？」と聞き、「読み込む」なら メニューの OPML を読み込む と同じ処理（購読していないフィードを足して更新） |
| 受け取る場所 | Rust だけ（`opened.rs`、`commands::hand_over`）。窓には `open-requested` のイベントで知らせ、窓が `take_open_requests` で受け取る。起動のときに窓がまだ読み込まれていなくても失わない |
| capabilities | **足していない**。deep-link の JS の API を使わず、Rust で受け取るので、窓に権限は要らない |
| プラグインの版 | `=2.6.1` に固定。2.6.2 は Linux で rust-ini 0.21.3 と組み合わせるとビルドできない（tauri-apps/plugins-workspace#3710。直ったが未リリース）。CI の Linux のテストで落ちた。2.6.3 が出たら `"2"` に戻す |
| 二重起動 | single-instance のまま。Windows で起動中に開くと、2 回目の起動の引数が今の窓に渡る |

## OS ごとの動き

- **macOS**：`feed:` は `Info.plist` の URL スキーム（`plugins > deep-link > desktop > schemes`）、`.opml` は書類の種類（`bundle > fileAssociations`）で登録される。どちらも起動時・起動中に `RunEvent::Opened` の URL として届く（deep-link プラグインの `on_open_url`）。登録はインストールしたアプリ（`.app`）でだけ効く。`npm run tauri dev` では効かない
- **Windows**：インストーラ（NSIS）がスキームと拡張子を登録する。届くのは起動の引数（初回の起動は `std::env::args`、起動中は single-instance の引数）。deep-link プラグインの Windows 向けの引数の処理は使わず、`opened::from_args` で自分で読む（ファイルの関連付けはプラグインが扱わないため。tauri-apps/plugins-workspace#1990）
- ほかのアプリがすでに `feed:` や `.opml` の既定になっているときは、OS の「このアプリで開く」の設定で選ぶ

## 守ること

- リンクの URL もファイルも外から来る。URL は欄に入れるだけ、ファイルは確かめてから読むだけで、どちらもユーザーの操作なしには何もしない
- `.opml` 以外の拡張子、無いファイル、`feed:` 以外のスキームは無視する（ログにも残さない）
- 受け取ったものはログに出す（`opened: …`）

## あとで

- macOS の共有メニュー（Share Extension）から購読する。アプリ拡張の追加になり、Tauri だけでは作れないので見送った
- ブラウザ拡張から購読する（フェーズ 7）
