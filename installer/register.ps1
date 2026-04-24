<#
  .SYNOPSIS
    Register dictation-overlay as a Chrome Native Messaging host for the current user.

  .DESCRIPTION
    Creates / updates the host manifest JSON and the Registry entry that Chrome
    (and Edge, optionally) uses to discover Native Messaging hosts.

    Registry path (per-user):
      HKCU:\Software\Google\Chrome\NativeMessagingHosts\com.bayashi.dictation_overlay

  .PARAMETER ExtensionId
    The Chrome extension ID that is allowed to connect. On an unpacked extension
    this is shown on chrome://extensions. Pass just the ID, no "chrome-extension://" prefix.

  .PARAMETER ExePath
    Absolute path to overlay.exe. Defaults to <repo>\src-tauri\target\release\dictation-overlay.exe.

  .PARAMETER ManifestDir
    Directory that will hold the host manifest JSON. Defaults to %LOCALAPPDATA%\Dictation\overlay.

  .PARAMETER IncludeEdge
    Also register under HKCU:\Software\Microsoft\Edge\NativeMessagingHosts.

  .EXAMPLE
    pwsh .\register.ps1 -ExtensionId aabbccddeeffgghhiijjkkllmmnnoopp

  .EXAMPLE
    pwsh .\register.ps1 -ExtensionId aabb... -ExePath C:\Users\me\proj\src-tauri\target\debug\dictation-overlay.exe
#>

[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [string]$ExtensionId,

  [string]$ExePath,

  [string]$ManifestDir,

  [switch]$IncludeEdge
)

$ErrorActionPreference = 'Stop'

$HostName = 'com.bayashi.dictation_overlay'

# Resolve repo root (parent of this script's directory).
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot  = Split-Path -Parent $ScriptDir

if (-not $ExePath) {
  $ExePath = Join-Path $RepoRoot 'src-tauri\target\release\dictation-overlay.exe'
}
if (-not (Test-Path $ExePath)) {
  # fall back to debug build
  $dbg = Join-Path $RepoRoot 'src-tauri\target\debug\dictation-overlay.exe'
  if (Test-Path $dbg) { $ExePath = $dbg }
}
if (-not (Test-Path $ExePath)) {
  throw "overlay.exe が見つかりません: $ExePath`nまず 'cargo build --release' を実行してください。"
}
$ExePath = (Resolve-Path $ExePath).Path

if (-not $ManifestDir) {
  $ManifestDir = Join-Path $env:LOCALAPPDATA 'Dictation\overlay'
}
New-Item -ItemType Directory -Force -Path $ManifestDir | Out-Null

$ManifestPath = Join-Path $ManifestDir "$HostName.json"

$manifest = [ordered]@{
  name            = $HostName
  description     = 'dictation-overlay native host (transparent caption overlay for dictation-beta)'
  path            = $ExePath
  type            = 'stdio'
  allowed_origins = @("chrome-extension://$ExtensionId/")
}

$json = ($manifest | ConvertTo-Json -Depth 5)
# Chrome expects UTF-8 without BOM.
[System.IO.File]::WriteAllText($ManifestPath, $json, (New-Object System.Text.UTF8Encoding($false)))

Write-Host "manifest を書き出しました: $ManifestPath"

function Register-Key($rootKey) {
  $key = "Registry::$rootKey\$HostName"
  New-Item -Path "Registry::$rootKey" -Name $HostName -Force | Out-Null
  Set-ItemProperty -Path $key -Name '(default)' -Value $ManifestPath
  Write-Host "Registry 登録: $key => $ManifestPath"
}

Register-Key 'HKEY_CURRENT_USER\Software\Google\Chrome\NativeMessagingHosts'

if ($IncludeEdge) {
  Register-Key 'HKEY_CURRENT_USER\Software\Microsoft\Edge\NativeMessagingHosts'
}

Write-Host ''
Write-Host "完了。拡張 ID: $ExtensionId" -ForegroundColor Green
Write-Host "overlay.exe:  $ExePath" -ForegroundColor Green
Write-Host ''
Write-Host "次: test-extension を chrome://extensions から読み込み、ポップアップの [接続] → [show_caption 送信] を試してください。"
