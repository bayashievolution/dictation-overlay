; dictation-overlay — Inno Setup インストーラスクリプト
;
; ビルド方法:
;   1. cargo build --release  （src-tauri\target\release\dictation-overlay.exe を作る）
;   2. ISCC.exe installer\dictation-overlay.iss
;      （または installer\build-installer.ps1 でまとめて実行）
;
; 出力:
;   <repo>\dist\dictation-overlay-setup-X.Y.Z.exe

#define AppName       "dictation-overlay"
#define AppVersion    "0.3.2"
#define AppPublisher  "bayashi"
#define AppURL        "https://github.com/bayashievolution/dictation-overlay"
#define ExeName       "dictation-overlay.exe"

[Setup]
AppId={{D7B2A0F3-3D4B-4F5C-9F2D-DCE2B1F1D0E3}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}/issues
AppUpdatesURL={#AppURL}/releases
VersionInfoVersion={#AppVersion}

DefaultDirName={localappdata}\Dictation\overlay
DefaultGroupName=dictation-overlay
DisableProgramGroupPage=yes

PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog

ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

Compression=lzma2/normal
SolidCompression=yes
WizardStyle=modern

OutputDir=..\dist
OutputBaseFilename=dictation-overlay-setup-{#AppVersion}

SetupIconFile=..\src-tauri\icons\icon.ico
UninstallDisplayIcon={app}\{#ExeName}
UninstallDisplayName=dictation-overlay (caption native helper)

; 情報パネル
AppContact=bayashi.evolution@gmail.com
LicenseFile=
InfoBeforeFile=
InfoAfterFile=POST_INSTALL.txt

[Languages]
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"

[Files]
; ネイティブ本体
Source: "..\src-tauri\target\release\{#ExeName}"; DestDir: "{app}"; Flags: ignoreversion
; アイコン（タスクバー / ショートカット用）
Source: "..\src-tauri\icons\icon.ico"; DestDir: "{app}"; Flags: ignoreversion
; 拡張 ID 登録/解除スクリプト（インストール後にユーザーが走らせる）
Source: "register.ps1";   DestDir: "{app}"; Flags: ignoreversion
Source: "unregister.ps1"; DestDir: "{app}"; Flags: ignoreversion
; インストール直後の説明
Source: "POST_INSTALL.txt"; DestDir: "{app}"; Flags: ignoreversion isreadme
; リファレンス（任意）
Source: "..\NATIVE_MESSAGING_SPEC.md"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist

[Icons]
; スタートメニューに「拡張IDを登録」ショートカットを置く。
; ユーザーは Chrome 拡張 ID を渡してこれを実行する。
Name: "{group}\dictation-overlay 拡張IDを登録"; \
    Filename: "powershell.exe"; \
    Parameters: "-NoExit -ExecutionPolicy Bypass -Command ""Set-Location -LiteralPath '{app}'; Write-Host '使い方: .\register.ps1 -ExtensionIds <ID1>,<ID2> -Append' -ForegroundColor Yellow; Write-Host ''"""; \
    WorkingDir: "{app}"; \
    IconFilename: "{app}\icon.ico"; \
    Comment: "PowerShell を開いて register.ps1 を実行できる状態にします"

Name: "{group}\dictation-overlay 拡張IDを解除"; \
    Filename: "powershell.exe"; \
    Parameters: "-NoExit -ExecutionPolicy Bypass -Command ""Set-Location -LiteralPath '{app}'; .\unregister.ps1"""; \
    WorkingDir: "{app}"; \
    IconFilename: "{app}\icon.ico"

Name: "{group}\アンインストール"; Filename: "{uninstallexe}"

[Run]
; インストーラー終了時にチェックボックスを出して、希望すればその場で
; PowerShell ウィンドウを起動して register.ps1 を走らせられる。
Filename: "powershell.exe"; \
    Parameters: "-NoExit -ExecutionPolicy Bypass -Command ""Set-Location -LiteralPath '{app}'; Write-Host '使い方: .\register.ps1 -ExtensionIds <Chrome拡張ID> -Append' -ForegroundColor Yellow"""; \
    Description: "PowerShell を開いて拡張IDを登録する"; \
    Flags: postinstall skipifsilent unchecked nowait

[UninstallRun]
; アンインストール時に自動で manifest と Registry を削除する。
Filename: "powershell.exe"; \
    Parameters: "-ExecutionPolicy Bypass -File ""{app}\unregister.ps1"""; \
    Flags: runhidden; \
    RunOnceId: "RemoveNativeMessagingHost"

[UninstallDelete]
; インストーラ生成物の残骸を確実に消す
Type: filesandordirs; Name: "{app}"
