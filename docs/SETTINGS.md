# 設定

## 置き場所

desktop-platform の Architecture §5 の 3 層のうち、「アプリ全体」と「表示の好み」の 2 つを使う。データごとの設定は無い（データは 1 つだけ）。

| 層 | 保存先 | 中身 |
|---|---|---|
| アプリ全体 | `app_config_dir()/settings.json` | 下の設定画面の項目 |
| 表示の好み | `localStorage` の `feedability.prefs` | 未読のみ表示、古い順、閉じているフォルダ、ペインの幅、本文の文字の大きさ・行間・幅（[READING.md](READING.md)） |

`settings.json` は一時ファイルに書いてから置き換える。読めないときは `settings.json.bad-<時刻>` に退避して既定値で始める。

## 設定画面（⌘,）

| 項目 | キー | 既定 | 選択肢 |
|---|---|---|---|
| 外観 | `theme` | `system` | OS に合わせる（`system`）/ ライト / ダーク（[THEME.md](THEME.md)） |
| フィードの更新 | `refreshMinutes` | 30 | 自動では更新しない（0）/ 15 / 30 / 60 / 180 分 |
| 既読の記事を残す期間 | `keepDays` | 30 | 7 / 30 / 90 / 365 日 / 削除しない（0） |
| 記事を既読にする | `markReadOn` | `open` | 開いたとき（`open`）/ 開いて 2 秒後（`delay`）/ 自分で既読にする（`manual`、M のみ）（[READING.md](READING.md)） |
| すべて既読にしたら次のフィードへ移る | `nextFeedAfterReadAll` | オフ | 一覧の「すべて既読」のあと、未読のある次のフィードを開く（[LIST.md](LIST.md)） |
| ブラウザ拡張 | `extension.enabled` | オフ | 受け付ける / 受け付けない。「拡張機能のフォルダを用意して表示」「すべての拡張機能との接続を解除」（[EXTENSION.md](EXTENSION.md)）。鍵 `extension.token` は Rust だけが読み書きする |
| stockroom | `stockroom` | 未接続 | 接続（する / し直す / 解除）、送り先のフォルダ（既定は未分類）、付けるタグ（「,」区切り、既定は無し）（[STOCKROOM.md](STOCKROOM.md)） |
| アップデートを起動時に確認する | `autoUpdate` | オン | |

変えたらすぐ保存する（保存ボタンは無い）。`settings.json` の `stockroom`（接続の鍵 `token`・`folder`・`folderName`・`tags`）は Rust だけが読み書きし、画面には鍵を渡さない（`stockroom_status` は接続済みかどうかだけ）。更新の間隔は次のタイマーから効く。
