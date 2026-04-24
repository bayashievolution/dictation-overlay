# dictation-overlay — Native Messaging 仕様書 (v0.1.0)

> このドキュメントは **dictation-beta（Chrome 拡張）** 側で実装する Native Messaging 連携のための仕様書です。dictation-overlay 側（ネイティブヘルパー）がこの仕様を満たします。
>
> 対応バージョン：dictation-overlay v0.1.0（Phase 1 実装）
>
> 最終更新：2026-04-25

---

## 1. 概要

- Chrome 拡張 `dictation-beta` が、ユーザーの PC にインストールされたネイティブアプリ `dictation-overlay` を起動し、stdio（JSON, length-prefixed）で字幕テキストとスタイルを渡す。
- ネイティブアプリは OS レベルの透過ウィンドウに字幕を描画する。
- 1 つの拡張につき 1 プロセス。`connectNative()` を呼ぶたびに新しいネイティブプロセスが起動する。

---

## 2. ホスト名

```
com.bayashi.dictation_overlay
```

Chrome の命名規則：小文字の ASCII 英数字・ドット・アンダースコアのみ（ハイフン不可）。

---

## 3. セットアップ（ユーザー環境）

ユーザー PC 上で以下が揃っている必要がある：

### Windows

- `%LOCALAPPDATA%\Dictation\overlay\com.bayashi.dictation_overlay.json`（manifest）
- Registry: `HKCU\Software\Google\Chrome\NativeMessagingHosts\com.bayashi.dictation_overlay` の `(default)` 値に manifest のフルパス
- manifest 内の `allowed_origins` に dictation-beta 拡張 ID が含まれていること
- `overlay.exe` 本体がインストール済みであること

### macOS（将来対応）

- `~/Library/Application Support/Google/Chrome/NativeMessagingHosts/com.bayashi.dictation_overlay.json`

### Linux（将来対応）

- `~/.config/google-chrome/NativeMessagingHosts/com.bayashi.dictation_overlay.json`

インストーラがこれらを一括で配置する設計。dictation-beta 側はインストーラの DL URL を提示するだけでよい。

---

## 4. 通信プロトコル

### 4.1 基本

- **トランスポート**：stdio（ネイティブ側から見ると stdin/stdout）
- **フレーム**：`[4-byte little-endian length][UTF-8 JSON]`
- **方向**：双方向。拡張→ネイティブは 1 MiB/msg、ネイティブ→拡張は 64 MiB/msg 上限（Chrome 仕様）
- **接続単位**：`chrome.runtime.connectNative('com.bayashi.dictation_overlay')` で port を張る。port が open している間、プロセスが生きる。

Chrome 拡張側は `port.postMessage(obj)` / `port.onMessage.addListener(fn)` で扱うため、length prefix は Chrome が自動で付与・解除する。拡張開発者は JSON オブジェクトだけを意識すればよい。

---

### 4.2 拡張 → ネイティブ

#### `show_caption`

字幕を表示する（非表示なら表示して更新、表示中なら更新のみ）。

```json
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
    "strokeColor": "#000000",
    "strokeWidth": 2,
    "lineHeightTenth": 13
  }
}
```

- `text`：必須。改行 `\n`、継続マーカー `→` などは文字列として埋めて送る。
- `settings`：任意。指定フィールドだけ更新する（ネイティブ側は直前の値を保持）。
- **推奨デバウンス**：拡張側で 50〜100ms。連続フレームを間引いてネイティブへの負荷を減らす。

#### `hide_caption`

ウィンドウを非表示にする（プロセスは維持）。

```json
{ "type": "hide_caption" }
```

#### `update_style`

テキストは変えずにスタイルだけ更新。

```json
{ "type": "update_style", "settings": { "fontSize": 72 } }
```

#### `set_position`

ウィンドウ位置・サイズを直接指定（px、物理ピクセル）。

```json
{ "type": "set_position", "x": 100, "y": 900, "width": 1600, "height": 200 }
```

- 右クリックメニューからの手動移動との排他性は、拡張側で「自動配置モード／手動モード」フラグを持って制御する想定。

#### `ping`

ヘルスチェック。ネイティブは `pong` を返す。

```json
{ "type": "ping" }
```

#### `exit`

ネイティブプロセスを終了させる。通常は拡張が `port.disconnect()` するだけで十分だが、明示的な終了が必要な時（デバッグ、手動再接続など）に使う。

```json
{ "type": "exit" }
```

---

### 4.3 ネイティブ → 拡張

#### `ready`

プロセス起動直後に **1 回だけ**自動送信される。

```json
{
  "type": "ready",
  "version": "0.1.0",
  "platform": "windows",
  "capabilities": ["transparency", "always-on-top"]
}
```

- `platform`：`windows` / `macos` / `linux`
- `capabilities`：ネイティブが実装している機能。拡張側で機能有無の判定に使う。
  - Phase 1：`["transparency", "always-on-top"]`
  - Phase 2 以降：`click-through` / `multi-monitor` / `chroma-key` などが追加される予定

#### `pong`

`ping` への応答。

```json
{ "type": "pong" }
```

#### `error`

パース失敗、内部エラーなど。

```json
{ "type": "error", "code": "parse", "message": "..." }
```

- `code`：`parse` / `init_failed` / `internal` など
- `message`：人間向け説明（ログ用）

#### `position_changed`（将来）

ユーザーが右クリックメニューやドラッグで手動移動・リサイズした時。拡張側は UI に反映する。

```json
{ "type": "position_changed", "x": 100, "y": 900, "width": 1600, "height": 200 }
```

Phase 1 の実装では未送信。Phase 3 で追加予定。

---

## 5. 拡張側の実装指針

### 5.1 接続

```js
let port = null;

function connectNativeOverlay() {
  if (port) return;
  try {
    port = chrome.runtime.connectNative('com.bayashi.dictation_overlay');
    port.onMessage.addListener(handleNative);
    port.onDisconnect.addListener(handleDisconnect);
  } catch (e) {
    // manifest 未登録 / インストーラ未実行 など
    notifyUser('ネイティブオーバーレイが見つかりません', e);
  }
}

function handleNative(msg) {
  switch (msg.type) {
    case 'ready':
      overlayState.version = msg.version;
      overlayState.platform = msg.platform;
      overlayState.capabilities = msg.capabilities || [];
      onOverlayConnected();
      break;
    case 'pong':
      /* heartbeat */
      break;
    case 'error':
      console.warn('[overlay]', msg.code, msg.message);
      break;
    case 'position_changed':
      onOverlayRepositioned(msg);
      break;
  }
}

function handleDisconnect() {
  const err = chrome.runtime.lastError;
  port = null;
  onOverlayDisconnected(err && err.message);
}
```

### 5.2 字幕送信

```js
function sendOverlayCaption(text, settings) {
  if (!port) return;
  try {
    port.postMessage({ type: 'show_caption', text, settings });
  } catch (e) {
    console.warn('send failed', e);
  }
}
```

`renderLatest()` から呼び出す。50〜100ms のデバウンス推奨。

### 5.3 切断

```js
function disconnectOverlay() {
  if (!port) return;
  try { port.disconnect(); } catch (_) {}
  port = null;
}
```

ページ離脱やトグル OFF 時に呼ぶ。

---

## 6. manifest.json の変更点

dictation-beta の `manifest.json` に `"nativeMessaging"` パーミッションを追加：

```json
{
  "permissions": [
    "sidePanel",
    "storage",
    "nativeMessaging"
  ]
}
```

追加後は拡張の再読み込みが必要。

---

## 7. UI 推奨（dictation-beta 側）

### 字幕ウィンドウのツールバー

- 「🪟 デスクトップオーバーレイ」ボタン
  - 未接続：押下 → `connectNativeOverlay()`
  - 接続中：押下 → `disconnectOverlay()`（確認ダイアログ任意）
  - 接続不能：エラー文言＋インストーラ DL リンク

### 設定パネル

- セクション：「ネイティブオーバーレイ連携」
  - 接続状態（接続中 vx.x.x / 未接続）
  - 「テスト字幕を送信」ボタン
  - 「インストーラをダウンロード」リンク（初期リリースでは GitHub Releases）

既存の OSDモード（拡張内ウィンドウ）と**並行使用可能**。オーバーレイ接続中も既存のウィンドウは独立して動く。

---

## 8. バージョニングと互換性

- `ready.version` でネイティブ側バージョンを通知。拡張側で対応表を持ち、大きなギャップがあれば警告。
- 新しい `type` を追加する時は：
  - ネイティブ側は未知の type を受け取ったら `error` を返す（silent drop しない）
  - 拡張側は `capabilities` に該当機能が載っているか確認してから送る

---

## 9. セキュリティ

- manifest の `allowed_origins` に列挙された拡張 ID からしか `connectNative` 不可（Chrome が強制）。
- 字幕テキストは PC ローカルで完結。ネイティブは外部通信しない。
- インストーラは `HKCU`（現在のユーザーのみ）を書き換え、管理者権限不要。

---

## 10. Phase 1 実装時の制限

| 機能 | v0.1.0 対応 | 備考 |
|---|---|---|
| `show_caption` / `hide_caption` | ✅ | |
| `update_style` | ✅ | |
| `set_position` | ✅ | |
| `ping` / `pong` | ✅ | |
| `exit` | ✅ | |
| 透明背景 | ✅ | Tauri 2.0 の `transparent: true` |
| 最前面 | ✅ | `alwaysOnTop: true` |
| クリックスルー | ❌（Phase 2） | Win32 `WS_EX_TRANSPARENT` で実装予定 |
| マルチモニタ選択 | ❌（Phase 2） | |
| 右クリックメニュー | ❌（Phase 3） | |
| `position_changed` 通知 | ❌（Phase 3） | |
| インストーラ（MSI） | ❌（Phase 3） | 現状は PowerShell スクリプト |
| macOS / Linux | ❌（Phase 3+） | Windows 優先 |

---

## 11. 参考

- 本プロジェクトの [HANDOFF.md](./HANDOFF.md)
- 本プロジェクトの [PROJECT_DESIGN.md](./PROJECT_DESIGN.md)
- [Chrome Native Messaging 公式](https://developer.chrome.com/docs/apps/nativeMessaging/)
- test harness: `test-extension/`（本リポジトリ内、Phase 1 検証用）
