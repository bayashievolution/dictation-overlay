# dictation-overlay — test extension (Phase 1 harness)

Phase 1 で「Chrome から送った文字列が透明フローティング窓に出る」ことを確認するための、最小限のテスト用 Chrome 拡張。

## セットアップ

1. ネイティブアプリをビルドして `overlay.exe` を用意する（プロジェクトルートで `cargo tauri build` または `cargo run`）。
2. `installer/register.ps1` を PowerShell で実行し、Native Messaging manifest を Registry に登録する。初回は `Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force` をそのセッションで 1 回打ってから。
3. `chrome://extensions` を開く → 「デベロッパーモード」ON → 「パッケージ化されていない拡張機能を読み込む」で `test-extension/` フォルダを選択。
4. 読み込んだら、右上に表示された拡張 ID をコピー。
5. `installer/com.bayashi.dictation_overlay.json` の `allowed_origins` にその拡張 ID（`chrome-extension://<ID>/`）が含まれていることを確認。
   - register.ps1 が拡張 ID を引数で受け取って書き出すので `.\installer\register.ps1 -ExtensionId <拡張ID>` と呼べばよい。
   - **dictation-beta も同時に繋ぎたい時**は `-ExtensionIds <test-ext ID>,<dictation-beta ID>` でまとめ登録するか、後から `-ExtensionIds <追加ID> -Append` で追加できる。
6. ツールバーの拡張アイコンをクリック → ポップアップから「接続」→「show_caption 送信」。
7. 画面下部中央に透明フローティング窓で字幕が出れば Phase 1 マイルストーン達成。

## アイコン

`icon.png` は仮（16x16 の単色で可）。Phase 4 で dictation-beta 本体に統合する際は本番の拡張側に巻き取る想定なので、ここでは最小限。

## このハーネスの目的

- `connectNative` が通るか
- 4-byte length prefix + JSON の往復が成立するか
- `show_caption` / `hide_caption` / `ping` / `exit` が期待通り動くか

Phase 2 以降の OS 別挙動検証（クリックスルー、マルチモニタなど）もこのハーネスから手動で叩く想定。
