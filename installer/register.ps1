<#
  .SYNOPSIS
    Register dictation-overlay as a Chrome Native Messaging host for the current user.

  .DESCRIPTION
    Creates / updates the host manifest JSON and the Registry entry that Chrome
    (and Edge, optionally) uses to discover Native Messaging hosts.

    Multiple Chrome extension IDs are allowed in `allowed_origins`. This is useful
    while developing — typically you want both the test-extension and the real
    dictation-beta extension to be able to connect.

    Registry path (per-user):
      HKCU:\Software\Google\Chrome\NativeMessagingHosts\com.bayashi.dictation_overlay

  .PARAMETER ExtensionId
    Single Chrome extension ID. Backward-compatible alias; merged into the
    final list together with -ExtensionIds.

  .PARAMETER ExtensionIds
    One or more Chrome extension IDs (comma-separated on the command line).

  .PARAMETER Append
    Read the existing manifest's allowed_origins (if any) and union them with
    the IDs given on this invocation. Without -Append the manifest's
    allowed_origins is fully replaced.

  .PARAMETER ExePath
    Absolute path to overlay.exe. Defaults to <repo>\src-tauri\target\release\dictation-overlay.exe
    falling back to debug build if release is missing.

  .PARAMETER ManifestDir
    Directory that will hold the host manifest JSON. Defaults to %LOCALAPPDATA%\Dictation\overlay.

  .PARAMETER IncludeEdge
    Also register under HKCU:\Software\Microsoft\Edge\NativeMessagingHosts.

  .EXAMPLE
    .\register.ps1 -ExtensionId aabbccddeeffgghhiijjkkllmmnnoopp
    # Single extension (legacy syntax, full replace).

  .EXAMPLE
    .\register.ps1 -ExtensionIds aabb..., ccdd...
    # Two extensions registered together (test-extension + dictation-beta).

  .EXAMPLE
    .\register.ps1 -ExtensionIds eeff... -Append
    # Add eeff... to whatever the manifest already had — keeps existing IDs.

  .EXAMPLE
    .\register.ps1 -ExtensionId eeff... -ExePath C:\Users\me\proj\src-tauri\target\debug\dictation-overlay.exe
#>

[CmdletBinding()]
param(
  [string]$ExtensionId,
  [string[]]$ExtensionIds,
  [switch]$Append,
  [string]$ExePath,
  [string]$ManifestDir,
  [switch]$IncludeEdge
)

$ErrorActionPreference = 'Stop'

$HostName = 'com.bayashi.dictation_overlay'

# -------- collect & validate extension IDs --------------------------------

$ids = @()
if ($ExtensionId)  { $ids += $ExtensionId }
if ($ExtensionIds) { $ids += $ExtensionIds }

# Strip whitespace and any "chrome-extension://...//" wrapping users may have pasted.
$ids = $ids | ForEach-Object {
  $v = ($_ -replace '^\s+|\s+$', '')
  $v = $v -replace '^chrome-extension://', ''
  $v = $v -replace '/+$', ''
  $v
} | Where-Object { $_ -and $_.Length -gt 0 }

if (-not $ids -or $ids.Count -eq 0) {
  throw "拡張IDを 1 つ以上指定してください。例: -ExtensionId aabb... または -ExtensionIds aabb...,ccdd..."
}

# Validate extension ID format (32 lowercase a-p chars). Loose check — warn only.
foreach ($id in $ids) {
  if ($id -notmatch '^[a-p]{32}$') {
    Write-Warning "拡張ID '$id' は標準的な形式（小文字 a-p の 32 文字）ではありません。続行しますが意図通りか確認してください。"
  }
}

# -------- resolve overlay.exe path ----------------------------------------

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot  = Split-Path -Parent $ScriptDir

if (-not $ExePath) {
  $ExePath = Join-Path $RepoRoot 'src-tauri\target\release\dictation-overlay.exe'
}
if (-not (Test-Path $ExePath)) {
  $dbg = Join-Path $RepoRoot 'src-tauri\target\debug\dictation-overlay.exe'
  if (Test-Path $dbg) { $ExePath = $dbg }
}
if (-not (Test-Path $ExePath)) {
  throw "overlay.exe が見つかりません: $ExePath`nまず 'cargo build --release' を実行してください。"
}
$ExePath = (Resolve-Path $ExePath).Path

# -------- compute manifest path -------------------------------------------

if (-not $ManifestDir) {
  $ManifestDir = Join-Path $env:LOCALAPPDATA 'Dictation\overlay'
}
New-Item -ItemType Directory -Force -Path $ManifestDir | Out-Null
$ManifestPath = Join-Path $ManifestDir "$HostName.json"

# -------- build allowed_origins (with optional append + dedupe) -----------

$newOrigins = $ids | ForEach-Object { "chrome-extension://$_/" }

$candidates = @()
if ($Append -and (Test-Path $ManifestPath)) {
  try {
    $existing = Get-Content -Raw -Path $ManifestPath | ConvertFrom-Json
    if ($existing.allowed_origins) {
      $candidates += @($existing.allowed_origins)
    }
  } catch {
    Write-Warning "既存 manifest の読み込みに失敗（破損？）。-Append を無視して新規作成します: $($_.Exception.Message)"
  }
}
$candidates += $newOrigins

# Dedupe while preserving insertion order.
$seen   = [System.Collections.Generic.HashSet[string]]::new()
$origins = New-Object System.Collections.Generic.List[string]
foreach ($o in $candidates) {
  if ($seen.Add($o)) { [void]$origins.Add($o) }
}

# -------- write manifest --------------------------------------------------

$manifest = [ordered]@{
  name            = $HostName
  description     = 'dictation-overlay native host (transparent caption overlay for dictation-beta)'
  path            = $ExePath
  type            = 'stdio'
  allowed_origins = @($origins)  # @( ) ensures JSON array even with one element
}

$json = ($manifest | ConvertTo-Json -Depth 5)
[System.IO.File]::WriteAllText($ManifestPath, $json, (New-Object System.Text.UTF8Encoding($false)))

Write-Host "manifest を書き出しました: $ManifestPath"
Write-Host "  allowed_origins:"
foreach ($o in $origins) { Write-Host "    - $o" }

# -------- registry --------------------------------------------------------

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
Write-Host "完了。" -ForegroundColor Green
Write-Host "  overlay.exe : $ExePath" -ForegroundColor Green
Write-Host "  拡張 ID 数  : $($origins.Count)" -ForegroundColor Green
if ($Append) { Write-Host "  モード      : Append（既存を保持してマージ）" -ForegroundColor Green }
else         { Write-Host "  モード      : Replace（既存を全置換）"             -ForegroundColor Green }
Write-Host ''
Write-Host "次: 拡張を Chrome に読み込み、ポップアップ／UI から [接続] → [show_caption 送信] を試してください。"
