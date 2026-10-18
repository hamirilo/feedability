# 本文を抜き出す crate（調べたもの）

フェーズ 4（全文取得、[FULL_TEXT.md](../FULL_TEXT.md)）のための候補の比較。決定ではない。
desktop-platform の Recommendations §3（依存の健全性チェック）の項目に沿って比べた。

## 調べ方

- 調べた日：2026-10-18
- crates.io の検索「readability」と、そこから辿れる crate。版・ライセンス・依存は crates.io の API、リポジトリの動きは GitHub の API で見た
- 脆弱性：`rustsec/advisory-db` に該当する crate の項目が無いことを、GitHub のコード検索で確かめた（`cargo audit` はこの Mac に入っていない。実装の PR で回す）
- 抜き出しの出来：上位 4 つを小さな試験用バイナリ（scratchpad。リポジトリには入れていない）に入れ、実在の記事ページ 4 本で試した。ITmedia NEWS、INTERNET Watch、GIGAZINE、BBC News。どれも 2026-10-18 に取得したページ
- 大きさ：試験用バイナリを Feedability と同じ release の設定（thin LTO、`opt-level = 3`、strip）でビルドし、crate を入れない場合と比べた

## 候補

| crate | 版 | ライセンス | 作り | 最後の版 | 判断 |
|---|---|---|---|---|---|
| **`dom_smoothie`** | 0.18.2 | MIT | readability.js の移植。`dom_query` の上 | 2026-09-21 | **第一候補** |
| `legible` | 0.5.1 | Apache-2.0 | readability.js の移植。html5ever の上 | 2026-08-08 | 次点 |
| `libreadability` | 0.3.0 | MIT | readability.js の移植。`scraper` の上 | 2026-10-15 | 外す |
| `readabilityrs` | 0.1.4 | Apache-2.0 | 独自の実装。`scraper` の上 | 2026-08-10 | 外す |
| `readability` | 0.3.0 | MIT | 古い移植。html5ever 0.26 | 2023-12-20 | 外す（止まっている。html5ever が古い） |
| `article_scraper` | 3.0.2 | GPL-3.0 | NewsFlash（GNOME のフィードリーダー）の抜き出し。サイトごとの規則と `dom_smoothie` | 2026-10-13 | 外す（GPL。reqwest と tokio を持ち込む） |
| `readability-js` | 0.1.6 | UPL-1.0 | readability.js を QuickJS（C）で動かす | 2026-09-17 | 外す（C の JS エンジンを同梱する） |
| `llm_readability` | 0.0.17 | MIT | spider の一部。html5ever をフォークしたもの | 2026-04-30 | 外す（0.0.x。フォークした html5ever を足す） |
| `readability-rust` | 0.1.0 | Apache-2.0 | 版が 1 つだけ。clap も依存に入っている | 2025-07-22 | 外す |

## 抜き出しの出来（実在のページ 4 本）

本文の文字数・`<p>` の数・`<img>` の数を比べた。

| ページ | dom_smoothie | legible | libreadability | readabilityrs |
|---|---|---|---|---|
| ITmedia NEWS（マンガの記事） | 1737 字・画像 7 | 同じ | 1805 字・画像 7（末尾に「関連する情報」が残る） | 2439 字・**画像 2**（「あなたにおすすめの記事」まで入る） |
| INTERNET Watch（ニュース） | 692 字・画像 3 | 同じ | 同じ | 1856 字・**画像 0**（関連記事の一覧が入る） |
| GIGAZINE | 2198 字・画像 12 | 同じ | 同じ | 2164 字・画像 3 |
| BBC News | 5064 字・画像 4 | 同じ | 同じ | 5158 字・画像 4 |
| 1 ページの時間 | 1〜4 ms | 3〜5 ms | 3〜6 ms | 17〜29 ms |

- readability.js の移植の 3 つは、4 本とも結果がほぼ同じ。ITmedia の末尾に「関連記事」の見出しが残るのも 3 つとも同じ（readability.js の動きのまま）
- `readabilityrs` は本文のまわりの一覧まで拾い、画像を落とす。比べる中で出来がいちばん悪い
- どれも JavaScript は動かさない。本文を JavaScript で描くページ（SPA）からは抜き出せない。その場合はフィードの要約のまま見せる（FULL_TEXT.md）

## 依存とビルド

| crate | Feedability の `Cargo.lock` に無い crate | 試験用バイナリの増え方 | ビルドの時間（依存込み、何も無い状態から） |
|---|---|---|---|
| `dom_smoothie` | `dom_smoothie`・`flagset`・`gjson`・`html-escape`・`nom`（5 つ、どれも小さい） | +1.3 MB | 5 秒 |
| `legible` | `legible` だけ | +2.3 MB | 6 秒 |
| `libreadability` | `scraper`・`ego-tree`・`dateparser`・`getopts` など 8 つ | +2.5 MB | 6 秒 |
| `readabilityrs` | `scraper`・`ego-tree` など | +2.8 MB | — |

- `dom_smoothie` の土台の `dom_query 0.28` と `html5ever 0.39` は、Tauri（`tauri-utils` の `html-manipulation`）が既に `Cargo.lock` に入れている。ただしそれはビルドのとき（proc-macro）だけで、アプリの実行ファイルに入るのは今回が初めて。上の「増え方」はそれを含む
- 実行ファイルには、ammonia の `html5ever 0.40` と並んで `0.39` が入る。STOCKROOM.md の `htmd`（0.38）も入れると、html5ever の版は 3 つになる
- 大きさは圧縮していない数字。配布物（dmg / NSIS）では小さくなる

## 健全性（Recommendations §3）

| 項目 | dom_smoothie | legible |
|---|---|---|
| リポジトリが動いているか | 動いている。最後の push は 2026-10-14、PR #226 まで。Issue に応答している | 動いている。最後の push は 2026-09-30、PR #284 まで。ほぼ作者 1 人で、Issue は 0 件 |
| アーカイブ | されていない | されていない |
| ライセンス | MIT | Apache-2.0 |
| 既知の脆弱性 | RustSec に項目なし | RustSec に項目なし |
| 使われ方 | 依存する crate が 40。`article_scraper`（NewsFlash）も使っている。GitHub のスター 226 | 作られたのが 2026-01。スター 3 |
| 剥がすコスト | 局所的。`fulltext.rs`（新しく作る）の 1 か所だけが使う | 同じ |
| 大きさとビルド時間 | 上の表のとおり。いちばん小さい | +2.3 MB |
| macOS と Windows | 純 Rust。OS に依存しない | 同じ |
| 同じ用途のもの | 無い | 無い |
| 入力の上限 | `max_elements_to_parse`（要素の数） | `ParseBudget`（バイト数・ノード数・深さなど）。こちらのほうが細かい |

## 推す理由

**`dom_smoothie`** を推す。

- 抜き出しの出来は readability.js の移植の 3 つで差が無い。そうなると、使われ方・大きさ・新しく入る依存で決まる
- 使われ方がいちばん広く、同じフィードリーダーの NewsFlash が実際に使っている
- 試験用バイナリの増え方がいちばん小さく（+1.3 MB）、土台の `dom_query` は Tauri と同じ版
- 次点の `legible` は入力の上限の付け方が細かく、新しく入る crate も 1 つだけ。ただ作られて 9 か月で、使っている人がほとんどいない。`dom_smoothie` が止まったときに乗り換える先として覚えておく

実装で気をつけること：

- `char_threshold`（既定 500 字）より短いと、抜き出しに失敗したことにする。日本語は 1 文字あたりの情報が多いので、INTERNET Watch（692 字）のような短いニュースが際どい。実装の PR で、日本語のページで値を確かめる
- `max_elements_to_parse` は必ず設定する（取得の上限 5 MB と合わせて、重いページで固まらないように）
