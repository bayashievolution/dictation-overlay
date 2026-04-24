# dictation-overlay — 引継ぎドキュメント

> このドキュメントは、ばっさんディクテーション（Chrome拡張 `dictation-beta`）と連携してPC画面に透過オーバーレイで字幕を表示する**ネイティブヘルパーアプリ**の新規プロジェクトに関する引継ぎ。新しく立ち上げる Claude（カルディ2 新セッション）がこれ1枚読めば全体像が掴めるように書いてある。

---

## 1. プロジェクト概要

**プロジェクト名**：`dictation-overlay`
**場所**：`~/dictation-overlay`
**目的**：Windows / macOS / Linux 上で、他のアプリ（Zoom・プレゼン資料・動画など）の**どんな画面の上にも透明に浮く字幕**を表示する。
**連携先**：`~/dictation-beta`（Chrome拡張機能「ばっさんディクテーション (β)」）

### 使用シーン

- 授業・講義・会議で発表者の音声を dictation-beta が文字起こし・AI整形
- 会場の PC 画面（プロジェクタ／大型モニタ）に字幕を**画面全体の最前面に透明オーバーレイ**
- プレゼン資料・Zoom 会議・動画再生など、**どんなコンテンツの上にも**字幕が乗る
- マウス操作は下のアプリに通る（クリックスルー）
- 聴覚障害のある方を含む誰もがアクセスできる合理的配慮の技術実装

---

## 2. なぜネイティブアプリが必要か

dictation-beta の現状（v0.12.4 時点）でできていること：
- Chrome サイドパネルで録音・文字起こし・AI 整形
- 別ウィンドウ「字幕（ライブキャプション）」を開ける
- OSDモード：親ウィンドウ内で UI を全部隠して字幕だけ大きく表示
- AI で TV 字幕風に整形（文節区切り、`→` 継続マーカー）
- トランジション（フェード / スライド / スクロール）

Web 技術の限界で**できないこと**：
- **OS レベルの透過**（Chrome ウィンドウには必ず枠・タイトルバーが残る）
- **クリックスルー**（字幕の裏側をクリックできない）
- **他のアプリの上に常時浮く**（Document PiP は Chrome のタイトルバーが付く、キャプション専用ではない）

→ **これらを実現するには OS の API を直接叩くネイティブアプリが必要**。

---

## 3. アーキテクチャ

```
┌────────────────────────────────┐
│  Chrome (dictation-beta 拡張)  │  ← 音声認識＋Gemini整形
│                                │
│  chrome.runtime                │
│  .connectNative()              │  ← Chrome Native Messaging
└──────────────┬─────────────────┘
               │ stdin/stdout (JSON, length-prefixed)
┌──────────────▼─────────────────┐
│  Native Host (dictation-overlay)│
│  - JSON パース                 │  ← Rust+Tauri 推奨
│  - 字幕テキスト受信             │
│  - OS API で透過窓描画          │
└──────────────┬─────────────────┘
               │ OS API
┌──────────────▼─────────────────┐
│  OSレベル透明窓                 │
│  ・常に最前面                   │
│  ・クリックスルー                │
│  ・画面全域                     │
└────────────────────────────────┘
```

---

## 4. 技術スタック選定

### 推奨：Rust + Tauri 2.0

| 理由 | 詳細 |
|---|---|
| **バイナリサイズ** | 〜10MB（Electron の 1/6） |
| **起動速度** | 瞬時（Electron: 1〜2秒） |
| **メモリ消費** | 50MB 前後（Electron: 200MB+） |
| **クロスプラットフォーム** | Windows / macOS / Linux を1コードベースで |
| **Native Messaging 実装** | Rust の `std::io` で stdio を直接扱うだけ |
| **ウィンドウ描画** | Tauri WebView（HTML+CSS で字幕 UI）or 直接 OS API |
| **クロマキー・透過** | Tauri の `transparent: true` + `decorations: false` で基盤対応、細かい制御は OS API 直叩き |

### 代替案

- **Electron**：実装最速だが配布サイズ大、メモリ重い。プロトタイプなら可
- **C++ + Qt**：最軽量・最速、開発時間長い。熟練者向け
- **Go + Wails / Lorca**：中間、Windows/Mac で挙動差あり

### 最終判断材料

- プロトタイプを 1〜2 週間で動かしたい → **Rust+Tauri**
- とにかく早くデモを見せたい → **Electron**（後で Rust に移行）
- 本番配布を前提・長期運用 → **Rust+Tauri**（推奨）

---

## 5. 通信プロトコル（Chrome Native Messaging）

### 基本仕様

- stdio + JSON
- 各メッセージは `[4-byte little-endian length][JSON payload]`
- 1メッセージ最大 1MB（拡張→ネイティブ）、64MB（ネイティブ→拡張）
- 1対1接続、拡張が `connectNative` した時だけネイティブアプリが起動

### 拡張 → ネイティブアプリ

```json
// 字幕表示
{
  "type": "show_caption",
  "text": "整形後の字幕テキスト\n継続→",
  "settings": {
    "fontSize": 64,
    "fontFamily": "'Noto Sans JP', sans-serif",
    "fontWeight": 600,
    "color": "#ffffff",
    "bgColor": "#000000",
    "bgAlpha": 70,
    "shadowOn": true,
    "shadowColor": "#000000",
    "shadowBlur": 6,
    "strokeOn": false,
    "position": "bottom-center"
  }
}

// 字幕を隠す
{ "type": "hide_caption" }

// スタイルだけ更新
{ "type": "update_style", "settings": { ... } }

// 位置を直接指定
{ "type": "set_position", "x": 100, "y": 200, "width": 1200, "height": 160 }

// 終了
{ "type": "exit" }
```

### ネイティブアプリ → 拡張

```json
// 起動完了・能力宣言
{
  "type": "ready",
  "version": "0.1.0",
  "platform": "windows",
  "capabilities": ["transparency", "click-through", "multi-monitor"]
}

// エラー
{ "type": "error", "code": "init_failed", "message": "..." }

// ユーザーが手動で位置移動した
{ "type": "position_changed", "x": 100, "y": 200 }
```

---

## 6. OS 別実装ポイント

### Windows

- **Win32 API** で透過窓を作る
- 必要な拡張スタイル：
  - `WS_EX_LAYERED`（per-pixel alpha 対応）
  - `WS_EX_TRANSPARENT`（マウスクリックスルー）
  - `WS_EX_TOPMOST`（常に最前面）
  - `WS_EX_TOOLWINDOW`（タスクバーに出さない）
- DWM composition 前提（Windows 10/11 ではデフォルトで有効）
- `UpdateLayeredWindow` で per-pixel alpha の描画
- `SetWindowPos(HWND_TOPMOST, ...)` で最前面維持
- Rust クレート：`windows` または `winapi`

### macOS

- **NSWindow** のレベル：`.floating` or `.statusBar`
- `ignoresMouseEvents = true` でクリックスルー
- `collectionBehavior`: `.canJoinAllSpaces | .fullScreenAuxiliary`
- Layer backing で透過：`backgroundColor = .clear`, `isOpaque = false`
- Rust クレート：`cocoa` + `objc`
- 注意：macOS Sequoia 以降でのスクリーン共有制約に対応必要

### Linux (X11 / Wayland)

- **X11**：
  - `XShapeCombineMask` でクリックスルー領域設定
  - `_NET_WM_STATE_ABOVE` で最前面
  - Composite Manager（picom 等）必須
- **Wayland**：
  - Compositor 依存、`wlr-layer-shell` プロトコルなど
  - GNOME Shell はレイヤーシェル未サポート（要拡張）
- Rust クレート：`x11rb` / `smithay-client-toolkit`
- Phase 3 以降で対応、まず Win/Mac 優先

---

## 7. インストーラ配布

### Windows

- **MSI**（WiX Toolset）or **Inno Setup**
- インストール先：`%LOCALAPPDATA%\Dictation\overlay.exe`
- **Native Messaging manifest を Registry に登録**：
  ```
  HKCU\Software\Google\Chrome\NativeMessagingHosts\com.bayashi.dictation_overlay
  (既定値) = C:\path\to\com.bayashi.dictation_overlay.json
  ```
- manifest JSON の例：
  ```json
  {
    "name": "com.bayashi.dictation_overlay",
    "description": "dictation overlay native host",
    "path": "C:\\Users\\xxx\\AppData\\Local\\Dictation\\overlay.exe",
    "type": "stdio",
    "allowed_origins": ["chrome-extension://<YOUR_EXT_ID>/"]
  }
  ```

### macOS

- **.dmg**（ドラッグインストール）or **pkg**
- Native Messaging manifest：
  ```
  ~/Library/Application Support/Google/Chrome/NativeMessagingHosts/com.bayashi.dictation_overlay.json
  ```
- **Gatekeeper 対応**必須：codesign + notarization（Apple Developer ID）
- これやらないと「開発元を確認できない」警告でユーザーが萎える

### Linux

- `.deb` / `.rpm` / AppImage
- Native Messaging manifest：
  ```
  ~/.config/google-chrome/NativeMessagingHosts/com.bayashi.dictation_overlay.json
  ```

---

## 8. dictation-beta 側の必要な変更

### 変更1: manifest.json に nativeMessaging 権限

```json
{
  "permissions": [
    "sidePanel",
    "storage",
    "nativeMessaging"  // ← 追加
  ]
}
```

### 変更2: captions.js にネイティブ接続コード

```js
let _nativePort = null;

function connectNativeOverlay() {
  try {
    _nativePort = chrome.runtime.connectNative('com.bayashi.dictation_overlay');
    _nativePort.onMessage.addListener((msg) => {
      if (msg.type === 'ready') {
        diagLog.info(`ネイティブオーバーレイ接続成功 v${msg.version}`);
      } else if (msg.type === 'error') {
        console.warn('[native]', msg.message);
      }
    });
    _nativePort.onDisconnect.addListener(() => {
      const err = chrome.runtime.lastError?.message || '未知のエラー';
      _nativePort = null;
      alert('ネイティブオーバーレイが切断されました:\n' + err
          + '\n\nインストーラを実行して再度試してください。');
    });
  } catch (e) {
    console.warn('native connect failed', e);
    alert('ネイティブオーバーレイが見つかりません。\n' + (e.message || e));
  }
}

function sendOverlayCaption(text, settings) {
  if (!_nativePort) return;
  try {
    _nativePort.postMessage({ type: 'show_caption', text, settings });
  } catch (e) {
    console.warn('send failed', e);
  }
}
```

### 変更3: UI に「🪟 デスクトップオーバーレイ」ボタン追加

- 字幕ウィンドウのツールバーに追加
- クリック → `connectNativeOverlay()` → renderLatest のたびに `sendOverlayCaption`
- 既存の OSDモード（親ウィンドウ内）と**並行使用可能**
- 切断時は自動でボタンを「未接続」表示に

### 変更4: 設定パネルに「ネイティブオーバーレイ連携」セクション

- 接続状態（接続中 / 未接続 / バージョン）
- 「テスト字幕を送信」ボタン
- 「インストーラをダウンロード」リンク（将来：ネイティブアプリの GitHub Releases）

---

## 9. 開発ステップ（推奨ロードマップ）

### Phase 1: MVP（1〜2週間）
1. Rust + Tauri プロジェクト骨組み（`cargo tauri init`）
2. Native Messaging の stdio 通信（JSON を echo して動作確認）
3. Chrome 拡張（実験用でOK）から `connectNative` → "hello" 送信 → ネイティブ側で console log
4. Tauri で透明ウィンドウ作成（`decorations: false`, `transparent: true`, `alwaysOnTop: true`）
5. 字幕テキスト表示（Tauri WebView に HTML+CSS でシンプル表示）
6. **マイルストーン**：Chrome から送った文字列が透明フローティング窓に出る

### Phase 2: OS別最適化（2週間）
7. **Windows**：`WS_EX_TRANSPARENT` でクリックスルー（Tauri デフォルトでは layered 窓にならないので Win32 API で `SetWindowLong` 直叩きが必要）
8. **macOS**：`NSWindow.level = .floating`、`ignoresMouseEvents = true`
9. マルチモニター対応（どのディスプレイに出すか選択 UI）
10. **マイルストーン**：Zoom 会議の上に字幕が浮いて、かつ Zoom のボタンがクリックできる

### Phase 3: 設定とインストーラ（2週間）
11. 字幕スタイル（フォント/色/位置/透明度）を拡張から送って反映
12. 右クリックメニュー（位置固定、透明度変更、終了 etc）
13. Windows MSI インストーラ（Inno Setup）+ NativeMessaging manifest 自動配置
14. macOS .dmg + 公証対応

### Phase 4: 本番統合（1週間）
15. dictation-beta に連携ボタン追加、正式リリース
16. ユーザー向けセットアップガイド（README / Notion）
17. **マイルストーン**：モニターさんに配布してフィードバックもらえる状態

---

## 10. 設計上の制約・注意点

1. **Native Messaging の拡張→ネイティブは 1MB / メッセージ上限**。字幕テキストは小さいので問題なし。
2. **stdin/stdout がストリーム** なので複数プロセス起動は不可。`connectNative` は 1 接続 / 1 拡張。
3. **セキュリティ**：manifest に書いた `allowed_origins` の拡張 ID からしか接続できない。
4. **ユーザー体験**：初回接続時にネイティブアプリが見つからなかった場合のガイド（「インストーラをダウンロード」URL へ誘導）を用意。
5. **アップデート**：ネイティブと拡張のバージョン互換性を `capabilities` / `version` でネゴシエーション。大きな破壊的変更時は明示的に警告。
6. **プライバシー**：字幕テキストは PC ローカルのみで完結（ネットに送らない）。Gemini API 呼び出しは拡張側で完結、ネイティブには整形済みの文字列だけ渡す。
7. **クリーンアップ**：アンインストール時に Registry / plist の manifest 登録も確実に削除する。

---

## 11. 現状の dictation-beta 字幕機能まとめ（参考）

### ファイル構成

```
~/dictation-beta/
├── manifest.json      ← Chrome 拡張 manifest (MV3)
├── index.html         ← サイドパネル UI
├── app.js             ← 録音・整形・UI のメインロジック
├── gemini.js          ← Gemini API クライアント
│                        formatForOSDWithGemini() が TV 字幕整形
├── captions.html      ← 字幕ウィンドウの DOM
├── captions.css       ← 字幕スタイル（フォント/色/トランジション）
├── captions.js        ← 字幕ロジック（localStorage 購読、AI整形、OSDモード）
└── style.css / icons.js / background.js / ...
```

### 送信すべきデータソース

- `captions.js` の `renderLatest()` が整形後テキストを算出している
- それを `sendOverlayCaption(text, settings)` で Native Messaging に流す
- デバウンス推奨（過度な更新を抑える）

### 設定で送る値

localStorage `dictation:captionsSettings` に以下あり：

```js
{
  fontSize, fontFamily, fontWeight, color,
  bgColor, bgAlpha,
  strokeOn, strokeColor, strokeWidth,
  shadowOn, shadowColor, shadowBlur,
  lineHeightTenth, paraCount, followLive,
  osdAi, transition,
  broadcastMode, keyColor,
}
```

ネイティブオーバーレイも同じ値を使って見た目を合わせれば、Chrome ウィンドウ内とオーバーレイで一貫した字幕表示になる。

---

## 12. 経緯（時系列サマリ）

- 2026-04-22 頃：dictation-beta に字幕ウィンドウ機能を追加（別タブで字幕表示）
- 2026-04-23 頃：Document Picture-in-Picture API で PiP 化試みる → Chrome のタイトルバーが必ず付く制約を認識
- 2026-04-24：OSDモード（親ウィンドウ内で UI 非表示）に転換、AI 字幕整形・トランジション追加（v0.12.4）
- 2026-04-25（本日）：やっさんから「OS レベルのオーバーレイは native driver で別プロジェクト化」の判断 → 本プロジェクト発足

---

## 13. 参考資料

- [Chrome Native Messaging 公式](https://developer.chrome.com/docs/apps/nativeMessaging/)
- [Tauri 2.0 Documentation](https://tauri.app/)
- [Windows Layered Windows (MSDN)](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features#layered-windows)
- [NSWindow - Apple Developer](https://developer.apple.com/documentation/appkit/nswindow)
- [wlr-layer-shell protocol](https://gitlab.freedesktop.org/wlroots/wlr-protocols/-/blob/master/unstable/wlr-layer-shell-unstable-v1.xml)
- [Inno Setup（Windows インストーラ）](https://jrsoftware.org/isinfo.php)
- [WiX Toolset（Windows MSI）](https://wixtoolset.org/)

---

## 14. 次セッション初手で確認したいこと

新しい Claude セッションを立ち上げたら、以下を最初に確認してスタート：

1. **このドキュメント（HANDOFF.md）を全部読む**
2. **OS 決め打ち**：まず Windows から着手（やっさんの環境が Windows）
3. **技術スタック**：Rust+Tauri で進めていいか最終確認
4. **プロトタイプの目標**：Phase 1 完了まで（Chrome 拡張から送った文字列が透明フローティング窓に出る）
5. **既存の dictation-beta には最後まで触らない**（Phase 4 で統合する時のみ変更）

---

*This handoff was written by カルディ2（Claude Opus 4.7, 1M context）on 2026-04-25.*
*Next カルディ2 session picks up from here.*
