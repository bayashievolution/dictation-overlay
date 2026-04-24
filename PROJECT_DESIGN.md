# dictation-overlay — 設計ドキュメント

## 目的

dictation-beta（Chrome 拡張）の字幕機能を、**OS レベルの透過・クリックスルー・常に最前面**なウィンドウとして PC 画面に出せるようにするためのネイティブヘルパーアプリ。

## レイヤー構成

```
┌─────────────────────────────────────┐
│ OS compositor                       │
│  ┌─────────────────────────────┐    │
│  │ Overlay Window (our app)    │ ← 透過・最前面・クリックスルー
│  │  - 字幕テキスト             │
│  └─────────────────────────────┘    │
│  ┌─────────────────────────────┐    │
│  │ 他のアプリ（Zoom/資料/etc）  │ ← 下のアプリ、操作はここに通る
│  └─────────────────────────────┘    │
└─────────────────────────────────────┘
```

## 状態モデル

```
[idle]
  ↓ connect from Chrome ext
[connected, waiting]
  ↓ show_caption
[displaying]
  ↓ hide_caption
[connected, waiting]
  ↓ disconnect / exit
[idle]
```

**単一ウィンドウ、単一状態機械**。複雑な並行状態は持たない。

## 通信プロトコル

### 送信（拡張 → ネイティブ）

| type | payload | 効果 |
|---|---|---|
| `show_caption` | `{text, settings}` | 字幕表示（update も兼ねる） |
| `hide_caption` | なし | ウィンドウ非表示（プロセスは維持） |
| `update_style` | `{settings}` | スタイルのみ変更 |
| `set_position` | `{x, y, width, height}` | 位置・サイズ変更 |
| `ping` | なし | ヘルスチェック |
| `exit` | なし | プロセス終了 |

### 受信（ネイティブ → 拡張）

| type | payload | タイミング |
|---|---|---|
| `ready` | `{version, platform, capabilities}` | 起動直後（一度だけ） |
| `pong` | `{ts}` | ping に対して |
| `error` | `{code, message}` | エラー発生時 |
| `position_changed` | `{x, y, w, h}` | ユーザーが手動移動・リサイズした時 |

## スタイルのマッピング（拡張の設定 → ネイティブ描画）

| 拡張側設定 | ネイティブ側の適用先 |
|---|---|
| `fontSize` (px) | テキストのフォントサイズ |
| `fontFamily` | テキストのフォントファミリー（OS にインストール済みである前提） |
| `fontWeight` | テキストの太さ |
| `color` | テキスト色 |
| `bgColor` + `bgAlpha` | 字幕背景（rgba） |
| `shadowOn/Color/Blur` | テキストシャドウ（Win32: GDI+, macOS: NSShadow） |
| `strokeOn/Color/Width` | テキスト縁取り（描画 2 回：太い縁→中身） |
| `lineHeightTenth/10` | 行間 |

## ウィンドウの振る舞い

### 既定

- 画面最前面・常時表示
- 背景完全透過（字幕テキストのみ描画）
- クリックスルー（下のアプリにマウスイベントが通る）
- 位置：画面下部中央（16:9 の下 1/4 あたり）
- リサイズ：`set_position` 経由、または右クリックメニュー

### 右クリックメニュー（ウィンドウ上）

- **表示位置**：`↓下 / ↑上 / 自由移動`
- **透明度**：`スライダ`
- **このオーバーレイを終了**
- **dictation-beta の設定を開く**（chrome-extension://.../options.html を起動）

※ クリックスルーが ON だと右クリックも通ってしまうので、「Ctrl 押しながら右クリック」等の条件付き、あるいはシステムトレイアイコンからもアクセス可能にする。

## ログ・診断

- ログファイル：`%LOCALAPPDATA%\Dictation\overlay.log`（Win）、`~/Library/Logs/dictation-overlay/` (Mac)
- レベル：`info` 以上
- 拡張側の診断ログ（設定モーダルの診断ログ）にも `[native]` プレフィクスで送り返す（将来）

## パッケージングとインストーラ

### Windows

1. cargo bundle で `overlay.exe` を生成
2. Inno Setup の `.iss` ファイルで `.exe` インストーラを作る
3. インストーラ内で：
   - `%LOCALAPPDATA%\Dictation\` に `overlay.exe` 配置
   - `com.bayashi.dictation_overlay.json` を配置（Chrome ディレクトリまたは任意の場所）
   - Registry `HKCU\Software\Google\Chrome\NativeMessagingHosts\com.bayashi.dictation_overlay` に manifest パスを書き込み
4. アンインストーラも同様に逆操作

### macOS

1. `.app` バンドルを作る（Tauri bundle）
2. codesign + notarization
3. `.dmg` でドラッグインストール
4. 初回起動時に `~/Library/Application Support/Google/Chrome/NativeMessagingHosts/` に manifest を置く（アプリ内のコードで）

## セキュリティ

- Chrome 拡張の ID を manifest の `allowed_origins` に書く → 他拡張からは接続不可
- stdin/stdout なのでネットワーク経由では叩けない
- 字幕テキストは PC ローカルで完結、外部送信なし

## パフォーマンス目標

- **起動**：< 1秒（Chrome からの connectNative 時）
- **描画更新**：字幕テキスト変更から画面反映まで < 100ms
- **メモリ**：待機時 < 50MB、表示中 < 80MB
- **CPU**：待機時 0%、表示中 < 2%

## 試行錯誤ログ

（このセクションに実装中の判断・失敗・代替案を積んでいく）

---

## 試行錯誤ログ — 2026-04-25 時点（企画段階）

- **Document PiP で代替できるか検討** → Chrome のタイトルバー固定、透過不可、クリックスルー不可で却下
- **Electron vs Tauri 検討** → 配布サイズ・起動時間で Tauri を第一選択、Electron はプロトタイプ用 backup
- **OS 対応順** → Windows 最優先（やっさん環境）、macOS 次、Linux 最後
