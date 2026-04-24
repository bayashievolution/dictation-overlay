# カルディ2→カルディ 依頼メモ

やっさん、カルディに以下のお願いを転送してもらえればOK：

---

## カルディへ：新規 Notion 掲示板3件を作成してほしい

以下3プロジェクトの Notion 掲示板を「claude掲示板」配下に作ってください：

### 1. dictation（本番・安定版）
- **GitHub**: https://github.com/bayashievolution/dictation
- **ディレクトリ**: `~/dictation`
- **状態**: v0.11.0（モニター配布中）
- **概要**: Chrome 拡張「ばっさんディクテーション」の本番。リアルタイム音声認識＋Gemini 整形。2026-04 から数十人のユーザーで運用中。

### 2. dictation-beta（β開発ライン）
- **GitHub**: https://github.com/bayashievolution/dictation-beta
- **ディレクトリ**: `~/dictation-beta`
- **状態**: v0.12.4（実験機能の開発場所）
- **概要**: dictation 本番の先を行く実験場。OSD字幕、AI TV字幕整形、Document PiP、バックグラウンド録音などがここで先に入る。動作安定したら本番に統合。

### 3. dictation-overlay（ネイティブOSDドライバ）
- **GitHub**: https://github.com/bayashievolution/dictation-overlay
- **ディレクトリ**: `~/dictation-overlay`
- **状態**: 企画段階（引継ぎドキュメントのみ）
- **概要**: OSレベル透過オーバーレイのネイティブヘルパーアプリ。Rust+Tauri 予定。Chrome 拡張（dictation-beta）と Native Messaging で連携。聴覚障害のある方向け合理的配慮を Zoom/プレゼンの上に字幕オーバーレイするのが目的。HANDOFF.md を読めば別セッションで実装開始可能。

---

### カルディへ追加依頼
- 3つとも作成後、**共通ルールページ**
  https://www.notion.so/341980e4ee1381fda220f5161d1c1a01
  のプロジェクト一覧にも追記してください。
- 作成したら `~/.claude/CLAUDE.md` の【Notionプロジェクト一覧】にも Notion ページ URL を貼ってもらえると嬉しい（いまは「*Notion未作成*」と書いてある箇所）。

---

*生成: カルディ2 (Claude Opus 4.7) 2026-04-25*
