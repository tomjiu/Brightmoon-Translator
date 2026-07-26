# Remove agent/editor temp files under the repo root.
$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $root

$removed = 0
Get-ChildItem -Recurse -Force -ErrorAction SilentlyContinue | Where-Object {
  $_.Name -like 'tmpclaude*' -or
  $_.Name -like 'pi-session*' -or
  $_.Name -like '*.tmp' -or
  ($_.Name -eq 'nul' -and -not $_.PSIsContainer)
} | ForEach-Object {
  $p = $_.FullName
  if ($_.Name -eq 'nul') { $p = '\\?\' + $p }
  try {
    Remove-Item -LiteralPath $p -Recurse -Force -ErrorAction Stop
    Write-Host "removed $p"
    $removed++
  } catch {
    Write-Warning "skip $p : $_"
  }
}

foreach ($d in @('.tmp', '.playwright-mcp')) {
  $path = Join-Path $root $d
  if (Test-Path -LiteralPath $path) {
    Remove-Item -LiteralPath $path -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "removed $path"
    $removed++
  }
}

Write-Host "done. removed_or_tried=$removed"
