# Feedability

RSS / Atom のフィードを購読して読む、ローカルのフィードリーダー（Tauri 2 / macOS・Windows）。

## 開発の基準

開発の Standard・Recommendations・Playbook は **`../desktop-platform/ai/ONBOARDING.md`** を最初に参照すること。Web アプリ向けの ai-dev-platform は参照しない。

実装へ大きく影響する前提は `decisions/project-context.md` にある。先に読む。

モデルは desktop-platform の Governance §3.1 のティアで使い分ける。設計・難しい調査・重要なレビューは本体のモデル、通常の実装と文書は Sonnet、テスト実行やスクリーンショットなどの確認は Haiku のサブエージェントへ委譲する。

## このツール固有

- 設計の境界：`docs/ARCHITECTURE.md`
- 機能ごとの決定：`docs/<topic>.md`
- 操作はコマンド表 `src/lib/commands.ts` にまず足す
- Rust コマンドは `src/lib/api.ts` のラッパ経由で呼ぶ。モック `src/dev/mockBackend.ts` も同じ PR で更新する
- 外への通信は `src-tauri/src/fetch.rs` だけ（ADR-0001）。WebView から外へ出さない（CSP に外の出どころを足さない）
- 画面を変えるときは、表示を切り替えても主要な操作の位置を変えない（desktop-platform の UI Standard §10）

## コマンド

```bash
npm install
npm run tauri dev      # アプリを起動
npm run dev            # ブラウザで UI だけ（モック）
npm test               # cargo test + tsc --noEmit
npm run lint
npm run tauri build    # 配布用ビルド（署名鍵が要る。playbook/updater-key.md）
```

## 変えないもの

`identifier`、`productName`、updater の `pubkey` と `endpoints`、CSP の緩和、capabilities の追加は人の判断なしに変えない。
