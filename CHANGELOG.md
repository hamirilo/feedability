# Changelog

## 0.2.0

### 変更点

- 外観設定：設定（⌘,）から「OS に合わせる / ライト / ダーク」を選択できるように対応
- あとで読む：記事を「あとで読む」に追加（ショートカット `L`）し、サイドバーから一覧表示可能に

### ダウンロード

- macOS（Apple Silicon）：`Feedability_0.2.0_aarch64.dmg`
- Windows：`Feedability_0.2.0_x64-setup.exe`

## 0.1.0

### 変更点

- 最初の版。フィードの購読（サイトの URL からフィードを探す）、フォルダ、未読・スター・検索、本文の表示、自動更新、OPML の読み込みと書き出し、アプリ内アップデート

### ダウンロード

- macOS（Apple Silicon）：`Feedability_0.1.0_aarch64.dmg`
- Windows：`Feedability_0.1.0_x64-setup.exe`

### 注意

- 署名なしのビルドなので、初回だけ警告が出る
  - macOS：`.dmg` を開いて「アプリケーション」へコピー → 起動すると「開発元を検証できない」と出るので「完了」→ **システム設定 › プライバシーとセキュリティ** の下にある「このまま開く」。ターミナルで `xattr -dr com.apple.quarantine "/Applications/Feedability.app"` でもよい
  - Windows：`-setup.exe` を実行 → SmartScreen が出たら「詳細情報」→「実行」
