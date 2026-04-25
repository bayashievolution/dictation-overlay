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

## 試行錯誤ログ — 2026-04-25 Phase 1 開始

### 環境セットアップ
- Rust ツールチェーン未導入 → `winget install Rustlang.Rustup` で 1.95.0 stable 導入
- MSVC 未導入 → `winget install Microsoft.VisualStudio.2022.BuildTools` でバックグラウンド導入
  - Tauri の Windows ビルドは `x86_64-pc-windows-msvc` がデフォルト。GNU target でも動くが WebView2 周りとの相性で MSVC 推奨。
- Username に日本語（`ばやし`）が含まれるため PATH 設定が扱いにくい → フルパス `/c/Users/ばやし/.cargo/bin/cargo.exe` で運用
- Git 側：WSL 経由のワークツリーは `safe.directory` 例外登録が必要（Windows ホストから WSL 共有を叩くため）

### プロジェクト骨組みの判断
- **Tauri プロジェクト雛形は `cargo create-tauri-app` ではなく手書き** にした。理由：
  - フロントエンドは静的 HTML/CSS/JS のみで十分（字幕 1 行の描画、フレームワークは過剰）
  - `frontendDist: "../src"` で `src/` の生ファイルをそのままロード、ビルドステップなし
  - `withGlobalTauri: true` で `window.__TAURI__` を生 JS から触れる
- **Native Messaging の stdio と Tauri イベントループの合流**：
  - `std::io::stdin()` は同期ブロッキングなので、setup 時に別スレッドを spawn して読む
  - 受信した InMessage を `AppHandle::emit` で WebView に送る
  - stdout 書き込みは `Mutex<()>` で直列化（将来 `position_changed` などを Rust 側から非同期に送るときのため）
- **エントリポイント**：`#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`
  - debug は console（手動起動でログを見る用）
  - release は windows（Chrome から起動される本番用、コンソール窓を出さない）
- **ウィンドウ初期可視性**：`visible: false`。`show_caption` 受信でのみ `show()`、`hide_caption` で `hide()`
- **CSS の `box-shadow: 0 0 0 transparent`** を敢えて入れてる：将来 Phase 2 でクリックスルー状態の視覚フィードバックに使う予定の土台

### Phase 1 スコープで **敢えてやらない** こと（Phase 2 以降）
- クリックスルー（`WS_EX_TRANSPARENT`）— Tauri の `set_ignore_cursor_events` があるが、HANDOFF 通り Phase 2 で本格対応
- マルチモニタ選択 UI
- 右クリックメニュー
- `position_changed` のネイティブ → 拡張通知
- MSI インストーラ（今は PowerShell `register.ps1`）
- macOS / Linux

### dictation-beta との連携準備
- `NATIVE_MESSAGING_SPEC.md` を本リポジトリ直下に作成
  - dictation-beta 側のカルディ／カルディ2 がこれを読めば manifest.json 変更・connectNative 実装まで完了できる粒度
  - `capabilities` 配列で Phase 間の機能差を拡張側にネゴシエーションできるようにした

### ビルド検証（2026-04-25 中の同日内）
- WSL 共有越しに `cargo build` すると cargo の target/ を \\wsl.localhost\... に作ってしまい I/O が激遅
  - → `CARGO_TARGET_DIR="C:/dev/dictation-overlay-target"` で Windows ローカル SSD に逃がした
  - このパスは個人環境依存なので README と `.gitignore` に注記、リポジトリには含めない
- 初回ビルド時の障害：`icons/icon.ico not found`
  - tauri-build が Windows Resource ファイル生成のために必須。empty `bundle.icon` でも回避不可
  - → PowerShell + `System.Drawing` で 32/128/256 PNG と ICO を生成して `src-tauri/icons/` に配置
  - プレースホルダなので本番リリース前に差し替え予定（青地に白 `DO` の暫定ロゴ）
- ワーキングディレクトリ混乱：Bash の `cd src-tauri` 後に再度 `cd src-tauri` してしまい `src-tauri/src-tauri/` 階層が生成された
  - → 以後 `(cd src-tauri && cargo ...)` のようにサブシェルで実行するポリシーに
- Smoke test：`printf '\x0f\x00\x00\x00{"type":"exit"}' | dictation-overlay.exe` で
  - stdout 先頭に `ready` が length-prefix 付きで書き出されることを確認（103 bytes）
  - `exit` メッセージを受けたらプロセスが正常終了することを確認
  - WebView2 の stderr ログは native messaging では Chrome が吸わないので実害なし

## 試行錯誤ログ — 2026-04-25 Phase 2 実装

### 設計判断
- **クリックスルーは起動時デフォルト ON**。オーバーレイの本質要件なので、OFF で立ち上げると UX がちぐはぐ。拡張側から `set_click_through: false` を送ればドラッグやリサイズができる「位置調整モード」に入れる
- **Tauri 2.0 の `set_ignore_cursor_events(bool)` を信頼**。HANDOFF では「Win32 SetWindowLong 直叩き」と書いてあったが、実際は Tauri 内部で `GWL_EXSTYLE | WS_EX_TRANSPARENT` を立てているので十分。macOS でも同 API が `NSWindow.ignoresMouseEvents` にマップされるのでクロスプラットフォームで同じコード
- **モニタ配置アルゴリズム**：`position_on_monitor_bottom(window, monitor, margin)` を実装。monitor の position と size、window の outer_size から物理ピクセルで中央下配置を計算。マルチモニタで負の座標もあり得るので `(mon_w - win_w).max(0) / 2` のように下限クランプ
- **起動時のプライマリ配置**：setup フック内で `primary_monitor()` → `position_on_monitor_bottom` を呼ぶ。`visible: false` でも `outer_size()` は設定値を返すので初期配置で問題なし
- **新メッセージの命名**：`set_*` / `list_*` で統一。レスポンスは過去完了系（`click_through`, `monitor_list`）。HANDOFF の将来機能 `position_changed` と整合

### 追加したメッセージ
- `set_click_through { enabled }` → `click_through { enabled }`
- `list_monitors` → `monitor_list { monitors: [...] }`
- `set_monitor { index }` → 配置成功時は無応答（副作用のみ）、範囲外なら `error { code: "monitor_out_of_range" }`

### capabilities の拡張
- Phase 1：`["transparency", "always-on-top"]`
- Phase 2：`["transparency", "always-on-top", "click-through", "multi-monitor"]`
- ready メッセージの JSON 長が 103 → 135 bytes になったので smoke test で確認

### 試してみて分かったこと
- Tauri 2.0 の `WebviewWindow::available_monitors()` は `Vec<Monitor>` を返す。`Monitor` 型は `Eq` を実装していないので `is_primary` 判定は position 比較で代用した（tauri 側で安定した識別子が無いため）
- `Monitor::name()` は `Option<&String>`。Windows では `\\.\DISPLAY1` 形式、macOS では実名（"Built-in Retina" など）
- 負の座標のマルチモニタ配置は Tauri が自動で仮想デスクトップ座標として解釈してくれる（Tao 内部で HMONITOR の virtual desktop 座標を使用）

### 実機検証状況（2026-04-25 時点）
- ✅ Phase 1 全項目 + Phase 2 の **クリックスルー ON 常態** / **ON/OFF トグル** / **list_monitors**：やっさん環境（Windows 11）で動作確認済
- ⚠️ Phase 2 の **set_monitor による外部モニタ移動**：外部モニタ非接続環境のため未検証。`collect_monitors()` のロジック・`position_on_monitor_bottom()` のクランプは Rust 単体テスト相当の机上確認のみ。外部モニタ接続時に実地検証が必要（Phase 3 のインストーラ検証時あたりに合わせてやる）

### v0.3.3 — ドラッグ移動の有効化（hotfix）

dictation-beta カルディ２が、やっさんの実機検証中に発見：「クリックスルー OFF にしても字幕窓をマウスでつかんで動かせない」。`WindowEvent::Moved` リスナーは正しいが、そもそも OS が move を発火する条件（タイトルバー or drag region）が無かった。

#### 修正
- `src/styles.css` の `.caption` に **`-webkit-app-region: drag`** を 1 行追加
- WebView2（Chromium）の標準 CSS で、Tauri がこれを HitTest::Caption 相当に変換してくれるので、字幕ボックスをつかんだドラッグで窓全体が動くようになる
- 重要：**`html, body` には drag を付けない**。透過部分は今まで通りマウスが素通りする必要がある（クリックスルー OFF 中、字幕外の透明領域でクリックすると下のアプリへ）。`.caption` ボックス内だけ drag region

#### Phase 2 のクリックスルーとの相互作用
- **クリックスルー ON 中**：OS が WS_EX_TRANSPARENT で全マウスイベントを下のアプリに流すので、CSS の drag region 設定があっても効かない（期待通り）
- **クリックスルー OFF 中**：drag region が効いて、字幕ボックスをドラッグすると窓が動く → `WindowEvent::Moved` 発火 → 150ms debounce で `position_changed` 送出
- これで dictation-beta 側の `onOverlayPositionChanged`（v0.13.3 で実装済み）が「位置: モニタ #N (X, Y) W×H」をリアルタイム反映する

#### 学び
- 透過 + 装飾なし窓を作る時は、ドラッグできる「掴み代」を CSS で明示しないと窓が物理的に動かせない（タイトルバーに頼れない）
- `WindowEvent::Moved` リスナーは正しく書いたつもりでも、**そもそも Move が発火しない**ケースが存在する。ソースコード読みで「リスナーがある」だけで安心せず、UX として動かす経路があるか確認する必要

### v0.3.2 — Phase 3c: Inno Setup インストーラ

#### 設計判断
- **インストーラは per-user (HKCU)** で固定。管理者権限プロンプトを避けてユーザーが気軽に試せる方を優先（`PrivilegesRequired=lowest`）
- **インストール先 `%LOCALAPPDATA%\Dictation\overlay\`** は手動運用（`installer/register.ps1` 既定値）と一致
- **拡張ID入力ウィザードは入れない**：Phase 4 では公式 dictation の拡張 ID を埋め込むだろうが、今は dev 用なので手動で `register.ps1` を打つ運用に。ただしインストール直後にスタートメニューから一発で PowerShell が「`cd %app%`」状態で開くようにして、コピペ 1 行で済むよう導線設計
- **アンインストール時に `unregister.ps1` を `runhidden` で自動呼び出し**：Registry と manifest が確実に消える。`UninstallDelete: filesandordirs` で {app} ディレクトリも除去
- **`InfoAfterFile=POST_INSTALL.txt`**：インストール完了画面で使い方が表示される。日本語で書いた

#### register.ps1 の修正
- `$ExePath` 解決順を変更し、**自分と同じディレクトリの dictation-overlay.exe** を最優先で見るようにした（インストール後シナリオ）。dev シナリオ（`<repo>\src-tauri\target\...`）は次点
- これで dev 時もインストール後も同じ register.ps1 が動く

#### build-installer.ps1
- `cargo build --release` → `ISCC.exe dictation-overlay.iss` をワンショットで実行
- ISCC.exe の自動検出（`Program Files`、`Program Files (x86)`、`LOCALAPPDATA\Programs` を順に探す）
- 未検出時は Inno Setup の DL URL を案内して exit 1
- `-SkipBuild` で cargo build をスキップ、`-IsccPath` で明示指定可能

#### 試した時の注意点
- `.iss` ファイルの `OutputDir=..\dist` は ISCC.exe の CWD（= `installer\`）からの相対なので、結果として `<repo>\dist\` に出る
- `ArchitecturesAllowed=x64compatible` で x64 と ARM64 の両方を許可（Windows 11 ARM 上でも動く想定）
- `PrivilegesRequiredOverridesAllowed=dialog` でユーザーが選択すれば管理者インストール（HKLM）も可能だが、既定は HKCU
- Inno Setup は実機にインストールしないとビルドできない。やっさん環境で `winget install JRSoftware.InnoSetup` か手動 DL 必要 → README にリンク記載済み

### v0.3.1 — Phase 3b: システムトレイメニュー

#### 設計判断
- **HTML右クリックメニュー（ウィンドウ上で右クリック）は今回見送り**
  - クリックスルー ON 中は OS が右クリックも下のアプリへ流すため、メニューを開けない
  - クリックスルー OFF 中だけで動かしてもユーザーは結局トレイ経由で OFF にする必要があるので、トレイで全部完結させる方が UX が一貫する
  - 仕様書の「実装状況」表に「保留」と明記
- **トレイメニューは Tauri 2.0 の `tray-icon` feature** を使用。Cargo.toml に `features = ["tray-icon"]` を追加。`tauri-plugin-*` ではなく core 機能なので依存追加は最小限
- **CheckMenuItem の状態同期**：トレイから ON/OFF した時はメニュー側で auto-toggle されるが、拡張からの `set_click_through` や `show_caption` で状態が変わる時はトレイメニューが古い値を表示してしまう。`Manager::manage(MenuHandles)` で `CheckMenuItem` の Clone を保管 → `handle_message` から `app.try_state::<MenuHandles>()` で取り出して `set_checked` する `sync_*` ヘルパーを書いた
- **トレイ「終了」の goodbye reason**：`"user_close"`。`exit_requested`（拡張から）と区別することで、拡張側 UI が「ユーザーがトレイから閉じた」を表示できる
- **show_menu_on_left_click(true)**：Windows 慣習では左クリック=デフォルトアクション、右クリック=メニュー。だが本プロダクトには「メイン UI」がない（オーバーレイは UI ではなく字幕表示）ので、左右どちらでもメニューを出す方がユーザーが迷わない

#### 効果
- クリックスルー ON 中でも、ユーザーがトレイから即座に OFF にできるようになった（位置調整が物理的に可能に）
- 拡張がクラッシュした／port を閉じ忘れた状況でも、ユーザーがトレイから「終了」を押せばプロセスは確実に死ぬ
- 拡張側の追加実装は不要。capabilities に `tray-menu` を加えただけで、メッセージ型は既存の click_through / goodbye / position_changed をそのまま流用

### v0.3.0 — Phase 3a: position_changed + goodbye

dictation-beta カルディ２から Phase 3 GO サインをもらって着手。3 サブフェーズに分割：

- **3a (このバージョン)**: `position_changed` 通知 + `goodbye` 予告（小・連携層）
- 3b (次): システムトレイアイコン + 右クリックメニュー
- 3c (次): Inno Setup インストーラ

#### 設計判断
- **position_changed のデバウンス**：OS は move/resize イベントを 60〜100Hz で打つので、毎回送ると拡張側があふれる。バックグラウンドスレッドで 150ms に 1 回 dirty フラグをドレインして送る方式に。leading edge ではなく trailing edge（最新値が必ず届く）にすることで、ドラッグ終端の正しい位置を必ず通知できる
- **GEOMETRY_DIRTY を AtomicBool で**：`Mutex<Instant>` で時刻ベースの debounce も可能だが、複雑。フラグ + 固定間隔ループのほうが読みやすく、CPU コストもほぼゼロ（150ms に 1 回しか走らない）
- **`goodbye { reason }` を OutMessage に追加**：dictation-beta カルディ２との議論で決めた A 案（予告メッセージ）を採用。さらに拡張側でフラグ運用する B 案も併用するのが推し（仕様書に記載）。今回は `exit_requested` のみ。Phase 3b で右クリック終了用に `user_close` を追加予定
- **`stdin EOF → app.exit(0)` のときは goodbye 不要**：Chrome 側の disconnect が起点なので、拡張側はもう port が閉じていることを知っている。送ろうとしても stdout が壊れている可能性が高い
- **capabilities に `position-report` を追加**：`position_changed` を受け取る前提で UI を組むかどうかの拡張側判定に使える

#### 試した気持ち悪さ
- `WindowEvent::Moved` は WebView 内のウィンドウ移動（タイトルバードラッグ等）を拾う。クリックスルー ON 中は OS はそもそもイベントを発火しない（マウスが下のアプリへ抜ける）。でも `set_position` プログラム的呼び出しでも Moved が飛ぶので、`position_changed` が起こり得る → 拡張側は idempotent に扱うべき（実装済みのサンプルコードはそうなってる）

### v0.2.1 — register.ps1 複数ID対応
dictation-beta カルディ２からの依頼で `installer/register.ps1` を改修。背景：
- 開発期は test-extension（接続検証用）と dictation-beta（本物）の **両方を同時に繋ぎたい**
- 旧版は単一 `-ExtensionId` のみ → manifest を上書きするため、片方を登録するともう片方が繋がらなくなる
- 結果、やっさんが手動で manifest JSON を編集して両 ID を入れる運用になっていた

**追加した点**：
- `-ExtensionIds string[]` パラメータ（カンマ区切りで複数受け取り）
- `-Append` スイッチ（既存 manifest の allowed_origins を読んで union → 重複除去 → 書き戻し）
- 旧 `-ExtensionId`（単数）は後方互換でそのまま受け取る。`-ExtensionId` と `-ExtensionIds` は同時指定可能で、両方が連結される
- 拡張 ID の正規化：先頭の `chrome-extension://` や末尾の `/` を勝手に削いで素の ID にする（コピペ事故防止）
- 拡張 ID の loose validation：`^[a-p]{32}$` でない場合は `Write-Warning` で警告（強制終了はしない）
- 完了メッセージで allowed_origins 一覧と Append/Replace モードを表示

**設計判断**：
- 重複除去は「既存→新規」の順で `HashSet.Add` に通すことで挿入順を保ちながらユニーク化
- `@($origins)` で書き出して、要素が 1 つでも JSON 配列形式を強制（PowerShell の `ConvertTo-Json` は単一要素を文字列にしてしまう挙動への対処）
- `-Append` 無指定時は明示的な「全置換」と扱う。Phase 4 の MSI 配布時は公式 ID で全置換するのが正解になるはずなので、Replace を素朴な既定値として残した
