# 読む（文字の表示・既読にするタイミング）

困っていること：本文の文字が小さい / 大きい、行が詰まっている / 広すぎる、と感じても変えられない。また、J / K で記事を送っていくと、読んでいない記事まで開いただけで既読になってしまう。

## 決定

- **本文の表示**：文字の大きさ・行間・本文の幅の 3 つ。表示の好みなので `localStorage` の `feedability.prefs`（`textSize` / `lineHeight` / `textWidth`）に、段階の番号で保存する。読み込むとき、範囲外の値は既定に戻す
  - 文字の大きさ：5 段階（13 / 14 / 15 / 17 / 19 px）。既定は 15 px（今までと同じ）。題名は同じ割合で大きくなる（15 px のとき 22 px）。日付などの行は変えない
  - 行間：狭い 1.6 / 標準 1.8 / 広い 2.0。既定は標準
  - 本文の幅：狭い 600 / 標準 720 / 広い 880 px。既定は標準
- **「Aa」のボタン**：本文の上の帯の、既存の 4 つのボタン（既読・スター・あとで読む・ブラウザ）の**左**。記事が無くても押せる。既存のボタンの位置は変わらない。押すと小さなパネルが下に開く。外をクリックするか Esc で閉じる。ダイアログではないので、パネルを開いたままでもキーは効く
  - パネル：文字の大きさ（− / ＋ と px）、行間と本文の幅（3 択のセグメント）、「標準に戻す」（3 つとも既定に）
- **キー**：⌘+ 文字を大きく、⌘− 小さく、⌘0 標準に（Windows は Ctrl）。コマンド表の `view.textBigger` / `view.textSmaller` / `view.textReset`、メニュー 表示に入れる。端まで来ると無効（⌘0 は既定のとき無効）。WebView 自身のズームは使わない（`zoomHotkeysEnabled` は既定でオフ）
  - 「＋」はキーボードによって Shift が要る（US は ⌘=、JIS は ⌘;）ので、Shift の有無は問わない
  - メニューのアクセラレータは `CmdOrCtrl+Equal` と `CmdOrCtrl+Minus`（muda が受け付ける名前）。メニューで拾えない配列のキーは、ページのキー処理が拾う（同じキーが両方に届いても 1 回しか動かない）
- **記事を既読にする**（設定、`settings.json` の `markReadOn`、既定は `open`）：
  - `open`（開いたとき）：今までどおり、開いたらすぐ既読
  - `delay`（開いて 2 秒後）：2 秒たっても同じ記事が開いていれば既読にする。別の記事を開く・表示を切り替える・記事を閉じる・M で自分で切り替える、のどれかでタイマーは取り消す。J / K で通り過ぎた記事は未読のまま残る
  - `manual`（自分で既読にする）：M、ボタン、メニュー、「ここより上 / 下を既読」「すべて既読」だけ
  - `delay` / `manual` では、開いている記事が未読のことがある（意図した動き）。次の未読（N）、ここより上 / 下を既読にする、未読のみ表示でも開いている記事を一覧に残す（`keep`）、Space で最後まで来たら次の記事、は今のまま働く

## 実装

- フロント：`lib/reading.ts`（段階の値と検証）、`store.ts` の `textSize` / `lineHeight` / `textWidth` と `setTextSize` ほか、`Reader.tsx` の `DisplayButton`。本文は `<article>` に `--article-size` / `--article-leading` を置き、`index.css` の `.article` がそれを読む
- コマンド表：`view.textBigger`（`Mod+Plus`）、`view.textSmaller`（`Mod+Minus`）、`view.textReset`（`Mod+0`）。`commands.ts` の `keyMatches` に `Plus` / `Minus`、`menuBar.ts` の `accelerator` に `Equal` / `Minus` への対応を足した
- 既読：`store.ts` の `select` が `markReadOn` を見る。タイマーはモジュールの `readTimer`（`finishOnLeave` と同じ作り）。発火したとき `selectedId` が同じでなければ何もしない
- Rust：`AppSettings.mark_read_on`（`MarkReadOn`、`#[serde(default)]` なので古い `settings.json` は `open`）。テストは `commands.rs` 末尾
