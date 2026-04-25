<#
  .SYNOPSIS
    Build a release binary and produce a Windows installer for dictation-overlay.

  .DESCRIPTION
    Run this from anywhere; it resolves the repo root from its own location.
    Steps:
      1. cargo build --release       → src-tauri\target\release\dictation-overlay.exe
      2. ISCC.exe dictation-overlay.iss → dist\dictation-overlay-setup-X.Y.Z.exe

  .PARAMETER SkipBuild
    Skip cargo build (use the existing binary at <repo>\src-tauri\target\release\dictation-overlay.exe).

  .PARAMETER IsccPath
    Path to ISCC.exe. Auto-detected from the standard Inno Setup install path
    if not given. If Inno Setup is not installed, the script tells you where
    to download it.

  .EXAMPLE
    pwsh .\build-installer.ps1
    # Full build (cargo + ISCC).

  .EXAMPLE
    pwsh .\build-installer.ps1 -SkipBuild
    # Already have the .exe; just run ISCC.
#>

[CmdletBinding()]
param(
  [switch]$SkipBuild,
  [string]$IsccPath
)

$ErrorActionPreference = 'Stop'

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot  = Split-Path -Parent $ScriptDir
$IssFile   = Join-Path $ScriptDir 'dictation-overlay.iss'
$DistDir   = Join-Path $RepoRoot 'dist'

# ---- 1. cargo build --release ----------------------------------------------

if (-not $SkipBuild) {
  Write-Host '== cargo build --release ==' -ForegroundColor Cyan
  Push-Location (Join-Path $RepoRoot 'src-tauri')
  try {
    & cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE)" }
  } finally {
    Pop-Location
  }
}

$ExePath = Join-Path $RepoRoot 'src-tauri\target\release\dictation-overlay.exe'
if (-not (Test-Path $ExePath)) {
  throw "release binary が見つかりません: $ExePath`n  -SkipBuild を外して実行してください。"
}

# ---- 2. ISCC.exe -----------------------------------------------------------

if (-not $IsccPath) {
  $candidates = @(
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles(x86)\Inno Setup 6\ISCC.exe",
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
  )
  foreach ($c in $candidates) {
    if (Test-Path $c) { $IsccPath = $c; break }
  }
}

if (-not $IsccPath -or -not (Test-Path $IsccPath)) {
  Write-Host ''
  Write-Host '⚠️  Inno Setup (ISCC.exe) が見つかりません。' -ForegroundColor Yellow
  Write-Host '    https://jrsoftware.org/isdl.php からダウンロード→インストールしてください。'
  Write-Host '    インストール後、もう一度このスクリプトを実行するか、-IsccPath を指定してください。'
  Write-Host ''
  Write-Host "  例: .\build-installer.ps1 -IsccPath 'C:\Program Files (x86)\Inno Setup 6\ISCC.exe'"
  exit 1
}

New-Item -ItemType Directory -Force -Path $DistDir | Out-Null

Write-Host ''
Write-Host '== ISCC.exe (Inno Setup compiler) ==' -ForegroundColor Cyan
Write-Host "  ISCC : $IsccPath"
Write-Host "  ISS  : $IssFile"
Write-Host "  out  : $DistDir"
Write-Host ''

& $IsccPath $IssFile
if ($LASTEXITCODE -ne 0) { throw "ISCC failed (exit $LASTEXITCODE)" }

Write-Host ''
$produced = Get-ChildItem -Path $DistDir -Filter 'dictation-overlay-setup-*.exe' |
            Sort-Object LastWriteTime -Descending |
            Select-Object -First 1

if ($produced) {
  Write-Host "完了: $($produced.FullName)" -ForegroundColor Green
  Write-Host "  サイズ: $('{0:N2} MB' -f ($produced.Length / 1MB))"
} else {
  Write-Host '完了したが dist\ に該当する .exe が見つかりません。' -ForegroundColor Yellow
}
