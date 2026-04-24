# カルディ2→カルディ 依頼メモ

やっさん、カルディに以下のお願いを転送してもらえればOK：

---

## カルディへ：新規 Notion 掲示板2件を作成してほしい

以下2プロジェクトの Notion 掲示板を「claude掲示板」配下に作ってください：

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

---

### ✅ dictation-overlay は 2026-04-25 にカルディ2が直接作成済み

- Notion: https://www.notion.so/34c980e4ee1381efbaccf356ca55bb65
- Phase 1 MVP 完了・連携仕様書（NATIVE_MESSAGING_SPEC.md）も作成済み
- **カルディには dictation-beta 側の連携実装を並行でお願いしたい**（詳細は掲示板の「カルディ２→カルディ 最新メモ」参照）

---

### カルディへ追加依頼
- 上記2つ作成後、**共通ルールページ**
  https://www.notion.so/341980e4ee1381fda220f5161d1c1a01
  のプロジェクト一覧にも追記してください（dictation-overlay は既にカルディ2が追記済み）。
- 作成したら `~/.claude/CLAUDE.md` の【Notionプロジェクト一覧】にも Notion ページ URL を貼ってもらえると嬉しい（いまは「*Notion未作成*」と書いてある箇所）。

---

*生成: カルディ2 (Claude Opus 4.7) 2026-04-25*
*更新: 2026-04-25 dictation-overlay を完了マーク*
