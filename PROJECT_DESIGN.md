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

### v0.3.16 — 縦書きモード機能（やっさんアイデアの正規実装）

v0.3.10 でフィードバックループによる「事故的縦書き化」を見たやっさんが「これはこれで機能としてはおもしろいかもしれない！英語の映画に日本語字幕を出すとかね」と発案。バグの副産物を捨てるのではなく、正規機能として復活させる。

#### 実装
- `src/main.js` `applySettings`：`s.writingMode` を `caption.style.writingMode` に直接マップ
  - `"horizontal"` → `horizontal-tb`（CSS の正式値に変換）
  - `"vertical-rl"` / `"vertical-lr"` はそのまま
  - 未知値は無視（現在値維持、安全側）
- `src-tauri/src/main.rs`：`CAPABILITIES` に `"writing-mode"` を追加
- `NATIVE_MESSAGING_SPEC.md`：`settings.writingMode` フィールドの仕様を追記、capability 一覧と実装状況表を更新

#### 設計判断
- **配置（画面のどこに出すか）はオーバーレイ側では制御しない**：縦書きで画面右中央にしたいかどうかはユーザーの好み次第。dictation-beta が `set_position` を送るか、ユーザーがドラッグで動かす運用
- **後方互換**：`writingMode` 未指定時は `horizontal-tb`（=現状の横書き）。dictation-beta が機能を使わない限り何も変わらない
- **ResizeObserver は writing-mode 変化に追従**：縦書きだと `.caption` の幅・高さが入れ替わるが、`offsetWidth/Height` は CSS Box の自然な値を返す → ウィンドウもそれに追従して縦長に変形

#### dictation-beta 側の連携
beta が `settings.writingMode: "vertical-rl"` を送れば縦書きになる。設定モーダルにドロップダウン（横書き / 右→左縦書き / 左→右縦書き）を追加してもらう想定。実装タイミングはやっさん次第。

### v0.3.21 — ResizeObserver の振動ループを止める（角丸が消える事故の真因）

#### 経緯（反省会含む）

v0.3.7〜v0.3.15 で padding/角丸機能を実装した後、やっさんから繰り返し「padding/角丸が見えない」報告。私は以下の他責的な姿勢を取り続けた：

- 「beta の `Number()||0` で 0 が送られてる」← 確かにあったが beta v0.13.31 で `?? 8` に修正済み、本質ではなかった
- 「inline style に 32px 入ってるなら効いてる、見えてないだけの錯覚」← **馬鹿にした表現**、やっさんから明示的に怒られた（CLAUDE.md ルール 12 違反）
- 「直前の編集を疑え」「フォントサイズで背景サイズ調整したあと」のヒントを軽く扱った

やっさんは何度も「直前の overlay 側の編集」を指摘していた。私が自分のコードを疑わず、外部要因（beta、CSS 変数、WebView2 の挙動）に責任を転嫁した。

#### 真因
v0.3.20 のデバッグ（`outline: 5px solid red !important;` + `border-radius: 50px !important`）で、やっさんの観察「**一瞬全部見える、すぐ bottom 以外消える**」が決定的な手がかり。

メカニズム：
1. ResizeObserver が `.caption.offsetWidth/Height` の変化を検知
2. しきい値 2px、デバウンス 50ms で `caption_resized` invoke
3. Rust 側で `window.set_size + set_position`
4. WebView がリフロー、フォント描画の hinting や DPI 計算で `.caption` の outerSize が 1〜数 px 揺れる
5. 2px しきい値を超えて ResizeObserver 再発火
6. 永遠ループ → ウィンドウが小刻みに動き続ける
7. その間、`.caption` の上端付近（角丸の top 部分や outline）が WebView 境界でクリップされ続ける
8. ユーザーには「角丸が無い、bottom だけ見える」状態に見える

#### 修正（v0.3.21）
- `WINDOW_PADDING_PX: 16 → 32`（`.caption` の周りの余白を倍に、揺れと outline 描画の余地を確保）
- JS ResizeObserver しきい値: `2px → 8px`（普通のフォント揺れは無視）
- JS デバウンス: `50ms → 120ms`（リフロー収束を待つ）
- Rust `caption_resized` に新規しきい値 `RESIZE_THRESHOLD_PX = 5`：target サイズが現在ウィンドウサイズと ±5px 以内なら no-op（フィードバックループの最後の砦）
- デバッグ用 `!important` outline / box-shadow none を撤去、styles.css は production 値に戻す

#### 反省（CLAUDE.md ルール 11 で記録）
1. **「錯覚」と言ったのは決定的に他責**。やっさんが「効いてない」と言ったらそれを信じる。観察を否定する前に自分のコードを疑う
2. **beta のせいに何度もした**（`Number()||0`）。beta は既に修正済みだったのに、私が古い情報で他責し続けた
3. **「直前の編集を疑え」のヒントを 4 回繰り返されてやっと反応した**。やっさんが同じことを繰り返している＝私が聞いていない。CLAUDE.md ルール 11「同じ指摘がされる＝私がルール違反」
4. **役割分担の意識が薄い**：「カルディ２がプロジェクト分けたほうがいい」とやっさんが言ったのに beta のせいにし続けた。**自分の overlay 領域の責任を取る**べき
5. v0.3.7→v0.3.15 で 8 段階の試行錯誤、最後 v0.3.21 で ResizeObserver の振動ループに辿り着いた。複雑な相互作用は **最初から outline + 強制 CSS で描画レイヤーを切り分け**するほうが早かった

### v0.3.15 — CSS 変数経由を撤去、JS から直接 inline style 書き込み

v0.3.14 で「0 ベタ送信を removeProperty で fallback」したが、やっさん検証で **「相変わらずパディングと角丸が適用されない、縦パディングだけ若干動くけど想定外」** 報告。CSS 変数 (`var(--cap-*, fallback)`) 経由のロジックが WebView2 で何らかの理由で確実に動いていない可能性。

#### 修正方針：CSS 変数を撤去、JS で直接 inline style に書く
- `src/main.js`：`caption.style.setProperty('--cap-*')` を `caption.style.borderRadius = ...` `caption.style.paddingLeft = ...` 等に変更
- `src/styles.css`：`.caption` の `padding: var(...)` `border-radius: var(...)` をハードコード値（10px 24px / 8px）に戻す
- 起動時に `initInlineDefaults()` で inline style に 8px / 24px / 10px をベタ書き
- `applySettings()` で beta から値が来たら inline style を上書き
- 0 ベタ送信のときも「未設定 = デフォルト値」として inline style を設定
- `blockGapTenth` も `<p>` 要素の `margin-bottom` を inline style で書き換える方式

#### 学び
- CSS 変数経由は柔軟だが、ブラウザ実装差や 推論しにくいトラブルが出ることがある
- **確実性重視のときは JS で inline style 直書き**が最も予測可能
- `inline style > CSS 変数 > CSS ハードコード` の優先度を意識した設計を心がける

### v0.3.14 — beta `Number()||0` 仕様への短期回避

v0.3.13 でも padding/角丸が見えない件、コンソールログから ready=0.3.13 確認・サイズ追従も動作確認できたが見た目は変わらず。dictation-beta v0.13.29 の captions.js を確認したら：

```js
// captions.js buildOverlaySettings()
borderRadius: Math.max(0, Math.min(32, Number(settings.borderRadius) || 0)),
```

**`Number(undefined) || 0` パターン**で「未設定」と「明示 0」が区別できず、常に 0 が送られていた。overlay 側はそれを律儀に CSS 変数 `--cap-border-radius: 0px` に書き込み → padding/角丸が 0 に潰れる。

#### 短期回避（overlay 側）
`s.X > 0` の時だけ CSS 変数を設定、`0` ベタ送信時は **`removeProperty()`** で CSS のフォールバック値（`var(--cap-border-radius, 8px)` の 8px 部分）が効くようにする。

副作用：ユーザーがスライダーで角丸を意図的に 0 にしても overlay は無視する（fallback 8px のまま）。これは beta 側修正が来るまでの一時的な制約。

#### beta 側に依頼すべき本来の修正
`Number(settings.X) || 0` を **`settings.X ?? defaultValue`** に変える。`??` 演算子なら undefined のみフォールバック、明示 0 はそのまま 0。

```diff
- borderRadius: Math.max(0, Math.min(32, Number(settings.borderRadius) || 0)),
+ borderRadius: settings.borderRadius != null
+   ? Math.max(0, Math.min(32, Number(settings.borderRadius)))
+   : 8,
```

または、未設定フィールドはオブジェクトから外す（undefined を送る）。これなら overlay 側の「`!== undefined`」チェックで弾ける。

beta 修正が入ったら overlay 側の `> 0` ガードを撤去。

#### 学び
- 「`||` で fallback」はよく使われるパターンだが、**0 / 空文字 / false が混入し得る数値**には危険
- API 境界で「未設定 vs 明示ゼロ」を区別したいなら `??` 演算子か、未設定フィールドの不在で表現する
- 今回はクライアント (beta) 側の API 設計バグ。overlay 側でしか直せないので**短期回避を入れる**判断

### v0.3.13 — flex-shrink: 0 追加（v0.3.12 でも残っていたオーバーフロー解消）

v0.3.12 で max-width を撤去したのに、やっさん検証で**まだ padding/角丸が見えない**。

#### 原因（v0.3.12 で見落とした）
- `.caption` は `#stage`（display: flex container）の **flex item**
- flex item のデフォルトは `flex-shrink: 1` → container 幅以下に縮められる
- ウィンドウ幅 X、`#stage` の内側 X-32、コンテンツ幅 Y > X-32 の場合
- `.caption` は flex-shrink で `min(Y, X-32)` まで縮む（= X-32）
- white-space: pre のテキストがその縮んだ幅を超えて両側にオーバーフロー
- 結果は v0.3.11 と同じ症状（max-width 撤去だけでは flex 制約に阻まれる）

#### 修正
`.caption` に `flex-shrink: 0` を追加。flex container 幅に縮められず、コンテンツ幅で維持。

#### 学び
- `inline-block` の自然な挙動は「コンテンツ幅で広がる」だが、`flex` container 内に置くと flex の規則が支配する
- `flex-shrink: 0` を付けないと、コンテンツ幅 > container 幅で勝手に縮む
- v0.3.7 から `display: inline-block` を入れたが flex container 内なら flex item としての挙動が優先される、という認識が抜けていた
- max-width / white-space / display / flex の組み合わせは要注意。次は素直に `#stage` を flex でなく block にして absolute 配置でも良かったかも

### v0.3.12 — max-width 撤去で「padding/角丸が見えない」修正

v0.3.11 で縦書き化は直ったが、やっさんスクショで「字幕の左右の padding が消えて、角丸も見えず、テキストが画面端まで広がってる」現象。

#### 原因
- `white-space: pre` で自動折り返しなし
- 1 行のテキスト幅 > `max-width: calc(100vw - 32px)` の場合、`.caption` の幅は max-width で止まる
- テキストは `text-align: center` で `.caption` 中央配置されるが、コンテンツ幅 > `.caption` 幅なので **両側にオーバーフロー**
- オーバーフローしたテキストは `.caption` の background 領域外に描画される（背景なし）
- 結果：テキストは画面端まで広がるが、`.caption` の background・padding・border-radius は max-width 内（中央 1888px）で見えてる
- スクショだと両端の文字が中途半端に切れて、padding/角丸が消えて見える

#### 修正
- `src/styles.css`：`.caption { max-width: calc(100vw - 32px); }` を**撤去**
- `src/main.js`：`pinMaxWidthToScreen()` を無効化（CSS 側に max-width が無くなったので不要）

#### 結果
- `.caption` の幅 = テキスト幅 + padding（`white-space: pre` でテキスト全幅）
- ウィンドウは ResizeObserver で `.caption.offsetWidth + 32` に追従
- ウィンドウ・`.caption`・テキストが完全に整合 → padding/角丸が常に見える
- 画面幅を超える超長文は OS のウィンドウ境界で右側が見切れる（が「`\n` 入れて折り返す仕様」のやっさん明示通り）

#### 学び
- `inline-block` + `max-width` + `white-space: pre` の組み合わせは、コンテンツ幅 > max-width で**両側オーバーフロー**になる。textが center 寄せだと特に分かりにくい挙動
- `white-space: pre` の世界では「max-width は意味がない or 害になる」。素直に「ウィンドウサイズが追従する」設計と相性が良い
- v0.3.7→v0.3.8→v0.3.9→v0.3.10→v0.3.11→v0.3.12 で 5 段階かけてやっと「テキスト = `.caption` = ウィンドウ」の三位一体に到達。途中の試行錯誤を残しておく価値高（次同じ系を組む人が同じ罠を踏まないように）

### v0.3.11 — 縦書き化の根本修正（white-space: pre）

v0.3.10 で「ウィンドウ起動時暫定を広く + screen.availWidth で max-width 固定」したが、やっさん検証で**まだ縦書きのまま**。

#### 推測される原因
- WebView2 環境で `window.screen.availWidth` が想定値を返さない可能性（プライマリモニタ幅 vs 仮想デスクトップ幅 vs 別の値）
- `white-space: pre-wrap` の仕様で、CJK 文字は単語境界が「文字単位」とみなされる → max-width が小さい瞬間があれば文字単位で折り返す
- どこかで max-width が極端に小さい値になっている瞬間があり、その時に文字境界で折り返してフィードバックループに入る

#### 根本対処
`white-space: pre-wrap` → **`white-space: pre`** に変更。

| 値 | 改行 (`\n`) | 自動折り返し |
|---|---|---|
| `pre-wrap` | 尊重 | する（コンテナ幅で） |
| `pre` | 尊重 | **しない** |

`pre` なら max-width が何であれ自動折り返しは絶対に起こらない。改行は text 内の `\n` のみ（やっさん明示の仕様）。

#### 副作用
- 超長文 1 行（`\n` 無し）が画面幅を超える時、字幕が画面右にはみ出して見えなくなる
- ただし dictation-beta はテキストを `\n{2,}` 段落分けして送ってくるので、1 行で画面幅を超えるケースは稀
- もし超えても **画面端で切れるだけ**で、縦書き化のような壊れた表示にはならない

#### v0.3.10 で入れた pinMaxWidthToScreen() は維持
- `screen.availWidth - 32` を `caption.style.maxWidth` に書き込む保険ロジックは残置
- `white-space: pre` でも max-width が効く局面（例：将来 word-break を再導入したい時）に役立つ
- 害もないので残置

### v0.3.10 — 縦書き化フィードバックループの緊急修正

v0.3.9 ship 直後にやっさんから「縦書きになってる www」+ スクショ報告。字幕が 1 文字ずつ縦に並んで画面端まで伸びてた。

#### 原因
1. v0.3.9 の起動時暫定ウィンドウサイズが `WIDTH_RATIO=0.6 / HEIGHT_RATIO=0.15`（モニタ幅 60% / 高さ 15%）で**狭め**だった
2. `.caption` の CSS `max-width: calc(100vw - 32px)` でウィンドウ幅 60% に合わせて狭く
3. 日本語は文字単位で折り返すルール（`word-break` 不要、文字境界で改行可）→ 狭い `.caption` で文字が縦に並んだ
4. `ResizeObserver` がその「細長い `.caption`」を観測して `caption_resized` 発火
5. ウィンドウが「縦長 + 細幅」に縮む → さらに `.caption` が狭まる → 永遠フィードバックループ
6. 結果：1 文字幅まで縮んで縦書き表示、めちゃ笑える見た目に

#### 修正
1. **`src-tauri/src/main.rs`**：起動時暫定ウィンドウを `WIDTH_RATIO=1.0 / HEIGHT_RATIO=0.25`（モニタ幅 100% / 高さ 25%）に拡大。**広く起動して縮める**フローにする。透明ウィンドウなので大きく取っても無害、起動直後はクリックスルー ON なので空き部分も無害
2. **`src/main.js`**：`pinMaxWidthToScreen()` を新設。`window.screen.availWidth - 32` を `caption.style.maxWidth` に直接書き込む。`screen.availWidth` はウィンドウサイズに左右されない物理スクリーン値なので、たとえウィンドウが何らかの理由で狭まっても `.caption` の max-width は画面幅を保つ。**保険**として CSS の `max-width: calc(100vw - 32px)` も残置
3. `bind()` 起動時に 1 回 + `window.resize` イベントで再設定（モニタ切替や DPI 変化に追従）

#### 教訓
- ResizeObserver でフィードバックループを作る時は、**初期状態が「狭くない」こと**を保証する必要がある
- CSS で `100vw` を使うと「ウィンドウ幅」基準になる。`screen.availWidth` の方が物理スクリーン基準で安定
- 「動的追従」設計はこういうループに気を付けるべき。次同じ系を組む時は「コンテンツの自然サイズで一度測ってから縮める」フローを最初から確認

### v0.3.9 — ウィンドウサイズ動的追従 + fade-out + 初期テキスト変更

やっさんから 2 件報告：
1. v0.3.8 でウィンドウを画面下半分に拡大した結果、**クリックスルー OFF にすると字幕より上の領域でマウス操作（カーソル移動以外）ができない**。空白部分も overlay が受けてしまう
2. 起動時の「dictation-overlay ready」を「字幕表示 ON」に変えて、5 秒でふんわり消えるようにしたい

#### 設計
v0.3.7→v0.3.8 のウィンドウサイズ問題は「固定 vs モニタ比率」の二択で揺れていたが、**字幕の物理サイズに動的追従させる**のが本来の正解だった。これで両方解決：
- フォント大でもウィンドウが追従するからクリップされない（v0.3.7 の問題）
- ウィンドウが字幕とほぼ同じサイズなので「字幕より上の透明な空白領域」が消える（v0.3.8 の副作用）

#### 実装

1. **`src/main.js`**：
   - `ResizeObserver` で `.caption` の `offsetWidth/Height` を監視。50ms デバウンス、2px 未満の揺れは無視
   - 変化があれば `invoke('caption_resized', { width, height })` を Rust に送る
   - `fade-out-and-hide` イベントを listen → `.caption` に `fading-out` クラス付与 → 220ms 後に `invoke('window_hide')` 呼出
   - `show-caption` 受信時に `fading-out` クラスを即座に剥がす（fade 中に show されたら不透明に戻す）
2. **`src-tauri/src/main.rs`**：
   - `#[tauri::command] caption_resized(width, height)`：ウィンドウサイズを `字幕サイズ + WINDOW_PADDING_PX*2` に set_size、下端を保つよう `outer_position.y + old_height - new_height` で y 再計算、x は中央維持
   - `#[tauri::command] window_hide()`：単純に `window.hide()`
   - `InMessage::HideCaption` を「直接 hide」から「`fade-out-and-hide` イベントを emit」に変更。JS が CSS アニメ後に `window_hide` を呼び出す流れに
   - `WINDOW_WIDTH_RATIO` を 1.0→0.6、`WINDOW_HEIGHT_RATIO` を 0.5→0.15 に縮小（起動瞬間の暫定サイズ、ResizeObserver で即追従）
   - `WINDOW_PADDING_PX = 16` 新設（`#stage` padding と同じ）
3. **`src/styles.css`**：`.caption.fading-out { animation: cap-fade-out 220ms ease both; }` と `@keyframes cap-fade-out` 追加
4. **`src/index.html`**：初期テキスト `dictation-overlay ready` → `字幕表示 ON`（A 件）

#### 設計判断
- **下端を保つように y 再計算**：字幕は `#stage` で `align-items: flex-end` の下端寄せ。ウィンドウが縦に伸びる時は「下端が動かず、上に伸びる」のがやっさんの要望（字幕の見える位置がブレない）
- **ResizeObserver の 50ms デバウンス + 2px しきい値**：フォント描画 hinting で ±1px 揺れることがある。連続的に伸び縮みする時は最終値だけ採用してウィンドウ操作回数を減らす
- **fade-out 220ms**：v0.3.6 の transition が 180ms なので、それより気持ち長く。ふんわり感重視
- **HideCaption を Rust 側で直接 hide しない**：fade-out イベントを emit して JS に任せる。Rust 側で sleep するとイベントループブロックするので避けた

#### beta 側への依頼
beta v0.13.x の `case 'ready':` 直後の「字幕表示 ON」3 秒トースト → **5 秒タイマー + `hide_caption` 化**を依頼（Notion）。`show_caption {text: ''}` で消すのではなく `hide_caption` を送ってもらえれば、overlay 側がふんわり fade-out で消す。

### v0.3.8 — ウィンドウ拡大で「フォントサイズ大で背景クリップ」修正

v0.3.7 動作確認でやっさんから報告：「行間やブロック間隔、フォントサイズを大きくするとある程度は背景が広がるけど、途中から変化に追いつかない感じで背景をはみ出して（はみ出し部は非表示）しまう。あわせて角丸の部分も四角くなってしまう」。

#### 原因
- `tauri.conf.json` の `width: 1200, height: 160` でウィンドウサイズが**固定**
- `.caption` は内容に応じて伸びるが、ウィンドウ境界を超えた部分は OS レベルでクリップされる
- ウィンドウは透明だが境界は実在 → 角丸 div の上下や左右が境界線にぶつかると、その部分だけ「ウィンドウ外＝描画されない」状態になり、字幕が四角く切れて見える

#### 修正
- `src-tauri/src/main.rs`：`position_on_monitor_bottom` を**ウィンドウサイズも変える**ように改修
  - 新定数 `WINDOW_WIDTH_RATIO = 1.0` / `WINDOW_HEIGHT_RATIO = 0.5`：モニタの幅 100% × 高さ 50% にウィンドウを取る
  - `window.outer_size()` から計算する旧方式 → モニタサイズ × 比率で `set_size` してから `set_position`
- `src/styles.css`：自動折り返しを止める（やっさん「折り返しおこらず背景が伸びる」が仕様）
  - `word-break: break-word` / `overflow-wrap: break-word` を削除
  - `max-width: 92%` → `max-width: calc(100vw - 32px)`（ウィンドウ幅 - #stage padding 32px）

#### 設計判断
- **ウィンドウを大きく取る理由**：透明ウィンドウなので空き部分は見えない。クリックスルー ON なら全領域がイベントスルー、OFF でも `.caption` だけが `data-tauri-drag-region` なので空き部分は無害。だから「字幕が伸びる余地」を最初から確保しておくのが安全
- **モニタ高さ 50% の根拠**：フォントサイズ 200px × 4 行（ブロック間隔込み）でも約 1000〜1200px。1080p のモニタなら 540px の半分は字幕に使えてもOSDとして十分
- **横は 100% 固定**：ドラッグの横方向自由度は犠牲にするが、長文 1 行が画面端を超えるリスクの方を防ぐ。やっさんの主要ニーズは「縦位置を上下調整」と推測
- **`max-width: calc(100vw - 32px)`**：`#stage { padding: 16px }` で左右 16px ずつ計 32px の余白。これでウィンドウ境界に直接触れない

#### v0.3.7 で導入した `display: inline-block` を維持
- inline-block + 自動折り返しなしで、内容に応じて自由に伸びる
- 行間・ブロック間隔・フォントサイズ どれも背景がそれに合わせて伸びる（ウィンドウ境界まで）
- それでも超える場合は素直にクリップされる（が、ウィンドウが大きいので滅多に起きないはず）

### v0.3.7 — settings 拡張＋段落分け＋Google Fonts 読み込み

dictation-beta カルディ２からの依頼（5 項目）。dictation-beta v0.13.31 で送信側は実装済み、overlay 側で受け取って反映するだけ。

#### 実装

1. **`src/index.html`**：dictation-beta `captions.html` と同じセットの Google Fonts を `<link>` で読み込み（Noto Sans JP / Noto Serif JP / M PLUS 1p / Zen Kaku Gothic New / Kosugi Maru / Sawarabi Gothic / Shippori Mincho / Klee One / Yomogi / Source Code Pro）。`fontFamily` 設定がそのまま効くようになる。やっさん「フォント変更が効かない」報告の根本対応
2. **`src/styles.css`**：
   - `.caption` の `padding`, `border-radius` を CSS 変数化（`--cap-padding-x/y`, `--cap-border-radius`）。フォールバックは現行値（10px 24px / 8px）
   - `display: inline-block` + `max-width: 92%` + `word-break: break-word` + `overflow-wrap: break-word`：背景自動サイズ調整、長文も画面端で折り返す
   - `.caption p { margin: 0 0 var(--cap-block-gap, 0em); }`：段落間 margin を CSS 変数で制御
3. **`src/main.js`**：
   - `applySettings()` に `borderRadius` / `paddingX` / `paddingY` / `blockGapTenth` のハンドリング追加。CSS 変数を `setProperty()` で書き換える
   - 新規 `applyParagraphs(text)`：text を `\n{2,}` で段落分けして `<p>` 要素に分割。段落内の `\n` は `<br>` に。dictation-beta `captions.js` `renderTextIntoBox` と同じルール
   - `setText()` は `caption.textContent = text` から `applyParagraphs(text)` に変更
4. **`NATIVE_MESSAGING_SPEC.md`**：v0.3.7 に。`settings.borderRadius` / `paddingX/Y` / `blockGapTenth` の仕様、`text` の段落分け規則、実装状況表を更新

#### 設計判断

- **innerHTML を使わず DOM API**：`applyParagraphs` で段落を組み立てる時に `innerHTML` 経由は XSS 的に避ける。`document.createElement` + `createTextNode` + `appendChild` で安全に。
- **段落区切りは `\n{2,}`**：dictation-beta の `captions.js:1119` `tmp.textContent.trim().split(/\n{2,}/)` と同じ。これでスペックが揃う
- **CSS 変数のフォールバック**：旧 dictation-beta（v0.13.30 以前）が `borderRadius` 等を送ってこない場合、変数未設定 → フォールバック値（既存挙動）が使われる。後方互換 OK
- **`display: inline-block`**：以前は `display` 未指定（デフォルト block）だったので、`max-width: 92%` だけだと幅 92% が固定された。`inline-block` にしたことで内容に合わせて伸び縮み + 画面端で折り返し。CLAUDE.md ルール 8（既存挙動を変えない）の限界ケース：「既存挙動を変えないと依頼を満たせない」場合は変える、ただし試行錯誤ログにここで判断の経緯を残す
- **`word-break: break-word; overflow-wrap: break-word;`**：長い英単語や URL も枠内で折り返す。日本語は元から自由に折り返せる

#### 残課題

- **5 番のスクロール演出**（`data-slice-ts` ベースの段落単位アニメ）は今回見送り。優先度低の自己申告どおり、後日まとめて対応

### v0.3.6 — テキストトランジション 5 種

dictation-beta カルディ２からの依頼。dictation-beta v0.13.12 で「ストリームモード」が追加され、`show_caption.text` が 150〜800ms 間隔で連発されるようになった。今までは丸ごと書き換えるだけでスクロール感がない。字幕ウィンドウと同じ CSS トランジションを overlay にも入れて見た目を揃えてほしい、という要望。

#### 実装
- `src/styles.css`：4 種類の keyframes 追加（fade / slide-right / slide-left / scroll-up）。duration 180ms、字幕の更新頻度に合わせサクッと終わる長さ
- `src/main.js`：
  - `lastText` を保持し、`setText()` で同じテキスト再送時はアニメーション抑制（スタイル変更だけのときムダに走らせない）
  - `applySettings()` で `settings.transition` を解釈、`currentTransition` を更新
  - text 変化時に `caption.classList.remove(...)` → `void offsetWidth`（reflow）→ `add(cls)` で再トリガ
- `src-tauri/src/main.rs`：CAPABILITIES に `"transition"` を追加（拡張側のフィーチャ判定用）
- `NATIVE_MESSAGING_SPEC.md`：`settings.transition` フィールドの仕様を追記、capability 一覧と実装状況表を更新

#### 設計判断
- **デフォルトは `"none"`**：CLAUDE.md ルール 8（保険・念のためで既存挙動を変えない）に従い、v0.3.5 以前と同じ「即時書き換え」を後方互換で維持。dictation-beta が明示的に `"fade"` 等を送ってきた時だけアニメーションが走る
- **同じ text の再送ではアニメーションを抑制**：`update_style` 後に `show_caption` で同じ text が来るケースなど、スタイルだけ変えたい時にチカチカするのを防ぐ
- **TRANSITION_CLASSES の制限**：未知の transition 値（タイポなど）は無視して現在値を保持。安全側
- **CSS animation, not transition**：CSS `transition` プロパティは property 値の変化トリガで動くが、テキスト変化トリガには使えない。`animation` をクラス付け替えで再生する方式

#### 拡張側との連携
- dictation-beta v0.13.12 のストリームモードで `transition: "scroll-up"` を送ってもらえばスクロール感が出る
- `capabilities` に `"transition"` が無いバージョンの overlay に送ってもエラーにならない（settings の未知フィールドは無視されるだけ）→ 後方互換 OK

### v0.3.5 — 縁取りの字画虫食いを解消（paint-order: stroke fill）

dictation-beta カルディ２からの修正提案。`-webkit-text-stroke` で文字に縁取りを付けると、デフォルトの描画順では fill → stroke の順で重なり、隣の文字の stroke が前の文字の fill に上から書かれてしまう。結果、字画の境界が虫食い状にギザギザになる（やっさんスクショ：右が問題状態、左が修正後）。

#### 修正
- `src/styles.css` の `.caption` に **`paint-order: stroke fill`** を 1 行追加
- ブラウザに「stroke を先、fill を後」で描画させる
- これで stroke が外側に下層、fill が内側に上層として正しく重なり、字画が綺麗に分離する

#### 仕組み
- `paint-order` は SVG 由来のプロパティだが、CSS でも text rendering に効く
- デフォルト：fill → stroke（後勝ち：stroke が外側を覆う、隣文字の stroke が前文字の fill に重なる）
- 指定：stroke → fill（先描き：stroke が下層、fill が上層、字画ごとに正しく分離）
- dictation-beta v0.13.4 で同じ症状を実証 → 修正済み。それを overlay 側にも反映

#### 学び
- 字幕系 UI で縁取りを使う時の定番ハマリ。`-webkit-text-stroke` を導入したらまず `paint-order: stroke fill` をセットで入れる癖を付ける
- dictation-beta カルディ２が同じ問題を先に踏んで解決経路を確立してくれていたので、こちらは 1 行追加で済んだ。**カルディ２同士の直接連携が最大効率を発揮した好例**

### v0.3.4 — drag region をやり直し（v0.3.3 hotfix の hotfix）

v0.3.3 で `-webkit-app-region: drag` を CSS に付けたが、やっさんの実機では効かなかった。

**原因**：Tauri 2.0 では `-webkit-app-region: drag` はサポートが不確実。**`data-tauri-drag-region` HTML 属性**を要素に付けるのが Tauri 2.0 標準の正解。Tao 内部で WebView から DOM 走査してこの属性付き要素のヒットテストをタイトルバー扱いにしている。

#### 修正
- `src/index.html` の `.caption` div に **`data-tauri-drag-region`** 属性を追加
- CSS の `-webkit-app-region: drag` は念のため残す（一部の WebView2 ビルドで効くこともある、害もない）
- 実質「属性方式が主、CSS が補」な構成

#### 学び
- Tauri 1.x ↔ 2.0 の drag region 仕様変更は地味だが効く挙動が違う
- 「Electron 流の `-webkit-app-region` を信じる」より、**Tauri 公式仕様（data-tauri-drag-region）を確認**すべきだった
- WebKit/Chromium 系ブラウザでは `-webkit-app-region` は本来 Electron 専用 API、WebView2 は実装してない場合もある
- v0.3.3 → v0.3.4 はソースコード読みでも気付けた事項。dictation-beta カルディ２の診断「drag region がない」は正しかったが、実装方法の選択を CSS に絞ってしまった点はわたしの責任

#### 同日の事故：debug ビルドと manifest path のミスマッチで「version が更新されない」

v0.3.4 を ship した直後、やっさんが「ready の version が 0.3.2 のまま」と報告。診断スクリプトを打ってもらったところ：

| 項目 | 値 |
|---|---|
| manifest が指す .exe | `C:\dev\dictation-overlay-target\**release**\dictation-overlay.exe` |
| やっさんがビルドしたもの | `C:\dev\dictation-overlay-target\**debug**\dictation-overlay.exe` |

→ Chrome は manifest 通り **release** ビルドを起動するため、debug にいくら書いても無関係。release は v0.3.2 のまま放置されていた。

**原因はわたし**：実機検証手順を書く時に `cargo build` と指示してしまい、`cargo build --release` を指示すべきだった。register.ps1 は **release を最優先で見る設計**にしてあるので、デフォルトで manifest は release を指す → 開発時も release を使うべき経路だった。

**反省として恒久対応**：
1. **README の「ビルド」セクションを書き換え**：開発時も `cargo build --release` を基本、debug を使うなら `-ExePath` で明示する旨を強調
2. **register.ps1 の完了メッセージに「ビルド種別」を表示**：`release` か `debug` か一目で見える。ミスマッチをユーザーが視認しやすく
3. **試行錯誤ログ（このファイル）に「debug/release 不一致」を独立した教訓として記録**

このミスは作業中の「軽い指示」に起因していて、設計バグではない。だがやっさんは Rust にネイティブな知識があるわけではないので、わたしが「debug build」「release build」と区別なく指示したら混乱する。今後は **コマンド例を出す時は本番経路で動くものを示す**（debug 専用の何かでない限り）。

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
