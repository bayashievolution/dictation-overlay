<#
  .SYNOPSIS
    Unregister dictation-overlay from Chrome Native Messaging (current user).
#>

[CmdletBinding()]
param(
  [switch]$IncludeEdge,
  [switch]$KeepManifest
)

$ErrorActionPreference = 'Continue'
$HostName = 'com.bayashi.dictation_overlay'

function Remove-Key($rootKey) {
  $key = "Registry::$rootKey\$HostName"
  if (Test-Path $key) {
    Remove-Item -Path $key -Force
    Write-Host "Registry 削除: $key"
  } else {
    Write-Host "Registry 未登録（スキップ）: $key"
  }
}

Remove-Key 'HKEY_CURRENT_USER\Software\Google\Chrome\NativeMessagingHosts'
if ($IncludeEdge) {
  Remove-Key 'HKEY_CURRENT_USER\Software\Microsoft\Edge\NativeMessagingHosts'
}

if (-not $KeepManifest) {
  $manifestDir = Join-Path $env:LOCALAPPDATA 'Dictation\overlay'
  $manifestPath = Join-Path $manifestDir "$HostName.json"
  if (Test-Path $manifestPath) {
    Remove-Item $manifestPath -Force
    Write-Host "manifest 削除: $manifestPath"
  }
}

Write-Host '完了。' -ForegroundColor Green
