# Cleanup temporary files and directories
# Run: powershell -ExecutionPolicy Bypass -File scripts/cleanup-temps.ps1

$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)

Write-Host "Cleaning temp files in: $root" -ForegroundColor Cyan

# Remove tmpclaude-* directories
$tmpDirs = Get-ChildItem -Path $root -Directory -Filter "tmpclaude-*" -ErrorAction SilentlyContinue
foreach ($dir in $tmpDirs) {
    Write-Host "  Removing: $($dir.FullName)" -ForegroundColor Yellow
    Remove-Item -Recurse -Force $dir.FullName
}

# Remove *.tmp files
$tmpFiles = Get-ChildItem -Path $root -Recurse -File -Filter "*.tmp" -ErrorAction SilentlyContinue
foreach ($file in $tmpFiles) {
    Write-Host "  Removing: $($file.FullName)" -ForegroundColor Yellow
    Remove-Item -Force $file.FullName
}

# Remove nul file (Windows artifact)
$nulFile = Join-Path $root "nul"
if (Test-Path $nulFile) {
    Write-Host "  Removing: $nulFile" -ForegroundColor Yellow
    Remove-Item -Force $nulFile
}

# Clean Cargo build cache (optional, uncomment if needed)
# Write-Host "  Cleaning Cargo target/debug incremental..." -ForegroundColor Yellow
# Remove-Item -Recurse -Force "$root\src-tauri\target\debug\incremental" -ErrorAction SilentlyContinue

Write-Host "Done." -ForegroundColor Green
