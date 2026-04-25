# dictation-overlay

Chrome拡張 [dictation-beta](../dictation-beta) と連携する **OSレベル透過オーバーレイ**ネイティブヘルパーアプリ。

## これは何？

- 字幕を **画面全体の最前面に透明に浮かせる**
- Zoom / プレゼン資料 / 動画再生 / どんなアプリの上にも字幕が乗る
- 字幕の裏はクリックスルー（下のアプリを操作できる）※Phase 2 で実装
- Chrome拡張（dictation-beta）から Native Messaging 経由で字幕テキスト・スタイルを受け取る

## プロジェクトステータス

🟢 **Phase 1 MVP 実装完了** — 2026-04-25 時点
- Tauri 2.0 骨組み
- Native Messaging stdio 通信（length-prefixed JSON）
- 透過・最前面フローティング窓
- 字幕描画（フォント・色・影・縁取り・背景アルファ）
- Phase 1 検証用テスト拡張
- Windows 向け Registry/manifest インストールスクリプト

⬜ **Phase 2 以降** — クリックスルー、マルチモニタ選択、右クリックメニュー、MSI インストーラ、macOS/Linux 対応。

## セットアップ（開発者向け）

### 前提

- Windows 10/11
- [Rust 1.70+](https://rustup.rs/)（`x86_64-pc-windows-msvc` toolchain）
- Visual Studio 2022 Build Tools（`VC++` ワークロード + Windows 10/11 SDK）
- PowerShell 5.1+ または PowerShell 7+
- Google Chrome
- Microsoft Edge WebView2 Runtime（Windows 11 は標準搭載）

### ビルド

```bash
cd src-tauri
cargo build           # debug → target/debug/dictation-overlay.exe
cargo build --release # release → target/release/dictation-overlay.exe
```

WSL 共有越しだと I/O が遅いため、環境変数で target ディレクトリをローカルに切ることを推奨：

```bash
export CARGO_TARGET_DIR="C:/dev/dictation-overlay-target"
cargo build
```

### Chrome Native Messaging への登録

1. Chrome で `chrome://extensions` を開き、デベロッパーモード ON → `test-extension/` フォルダを「パッケージ化されていない拡張機能を読み込む」
2. 読み込んだ拡張の ID をコピー（例: `aabbccddeeffgghhiijjkkllmmnnoopp`）
3. PowerShell で：

```powershell
# 初回のみ：このセッションだけスクリプト実行を許可（-Force で確認プロンプト無し）
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force

# ① 1つの拡張だけ登録（最も基本）
.\installer\register.ps1 -ExtensionId <拡張ID> -ExePath C:\dev\dictation-overlay-target\debug\dictation-overlay.exe

# ② 2つ以上の拡張をまとめて登録（test-extension + dictation-beta を両方繋げたい時）
.\installer\register.ps1 -ExtensionIds <ID1>,<ID2> -ExePath C:\dev\dictation-overlay-target\debug\dictation-overlay.exe

# ③ 既存の登録に追加（既に走らせた後で別の拡張も繋げたくなった時）
.\installer\register.ps1 -ExtensionIds <追加ID> -Append
```

PowerShell 7 (`pwsh`) が入っていれば `pwsh .\installer\register.ps1 ...` でも可（`-Scope Process` の Bypass は PS7 でも必要）。

**Tips**：
- 複数拡張を `,` で並べる時、PowerShell では空白を入れると別パラメータと解釈されるので注意。`<ID1>,<ID2>` のように密着で
- `-Append` は manifest の `allowed_origins` を読んでマージ → 重複は自動除去
- `-Append` を付けないと既存の登録を全置換するので、運用時は `-Append` を強く推奨

これで：
- `%LOCALAPPDATA%\Dictation\overlay\com.bayashi.dictation_overlay.json` が配置される
- Registry `HKCU\Software\Google\Chrome\NativeMessagingHosts\com.bayashi.dictation_overlay` に manifest パスが登録される

### 動作確認（Phase 1 マイルストーン）

1. Chrome の拡張ツールバーから `dictation-overlay test harness` のアイコンをクリック
2. ポップアップ →「接続」→「show_caption 送信」
3. 画面下部中央に透明なフローティング窓で字幕が表示される ✅

ログが見たいときは debug build をターミナルから直接起動して stderr を観察：

```bash
echo -ne '\x0f\x00\x00\x00{"type":"exit"}' | C:/dev/dictation-overlay-target/debug/dictation-overlay.exe
```

起動直後に `ready` JSON が stdout に length prefix 付きで書き出されれば OK。

### アンインストール

```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
.\installer\unregister.ps1
```

Registry 登録と manifest ファイルを削除する（`-KeepManifest` で manifest は残せる）。

## プロジェクト構成

```
dictation-overlay/
├── README.md
├── HANDOFF.md                    # 新セッション引継ぎ
├── PROJECT_DESIGN.md             # 設計 & 試行錯誤ログ
├── NATIVE_MESSAGING_SPEC.md      # dictation-beta 側に渡す連携仕様書
├── src/                          # WebView に読ませる字幕描画フロント
│   ├── index.html
│   ├── main.js
│   └── styles.css
├── src-tauri/                    # Rust ネイティブ本体
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── capabilities/default.json
│   ├── icons/                    # 仮アイコン（DO ロゴ）
│   └── src/
│       ├── main.rs               # Tauri エントリ + stdin ループ
│       └── native_messaging.rs   # length-prefixed JSON フレーミング
├── test-extension/               # Phase 1 検証用 Chrome 拡張
│   ├── manifest.json
│   ├── popup.html
│   └── popup.js
└── installer/
    ├── com.bayashi.dictation_overlay.json
    ├── register.ps1
    └── unregister.ps1
```

## dictation-beta との連携

dictation-beta 側の実装は `NATIVE_MESSAGING_SPEC.md` 参照。Phase 4 まで dictation-beta 本体には触らない方針だが、仕様書を渡してカルディ側で並行実装を進められる。

## ライセンス

MIT（予定）

---

*カルディ2（Claude Opus 4.7）が Phase 1 を実装。詳細な判断経緯は PROJECT_DESIGN.md の試行錯誤ログ参照。*
