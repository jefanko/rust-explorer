param (
    [string]$BinaryPath = "target\release\rust-explorer-native.exe"
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $BinaryPath)) {
    $BinaryPath = "target\debug\rust-explorer-native.exe"
}

if (-not (Test-Path $BinaryPath)) {
    Write-Error "Binary not found at $BinaryPath"
    exit 1
}

Write-Host "=== Rust Explorer Native - Memory Measurement ===" -ForegroundColor Cyan
Write-Host "Binary: $BinaryPath"

$proc = Start-Process -FilePath $BinaryPath -PassThru
try {
    Start-Sleep -Seconds 3

    $p = Get-Process -Id $proc.Id
    $wsMB = [math]::Round($p.WorkingSet64 / 1MB, 2)
    $pmMB = [math]::Round($p.PrivateMemorySize64 / 1MB, 2)

    Write-Host "Fresh Launch (Idle 3s):" -ForegroundColor Green
    Write-Host "  Working Set:    $wsMB MB"
    Write-Host "  Private Bytes:  $pmMB MB"

    $maxIdleAllowedMB = 60.0
    if ($wsMB -le $maxIdleAllowedMB) {
        Write-Host "  [PASS] Idle Working Set is under threshold ($wsMB MB <= $maxIdleAllowedMB MB)" -ForegroundColor Green
    } else {
        Write-Host "  [WARN] Idle Working Set exceeded threshold ($wsMB MB > $maxIdleAllowedMB MB)" -ForegroundColor Yellow
    }

    # Monitor for 5 more seconds
    Start-Sleep -Seconds 5
    $p = Get-Process -Id $proc.Id
    $wsMB2 = [math]::Round($p.WorkingSet64 / 1MB, 2)
    $pmMB2 = [math]::Round($p.PrivateMemorySize64 / 1MB, 2)

    Write-Host "After 8s Idle:" -ForegroundColor Green
    Write-Host "  Working Set:    $wsMB2 MB"
    Write-Host "  Private Bytes:  $pmMB2 MB"

    Write-Host "`nSummary against Tauri Baseline (150 - 350 MB):" -ForegroundColor Cyan
    $reduction = [math]::Round((1.0 - ($wsMB2 / 200.0)) * 100, 1)
    Write-Host "  Estimated RAM reduction vs Tauri: ~${reduction}% saved" -ForegroundColor Green

} finally {
    if (-not $proc.HasExited) {
        Stop-Process -Id $proc.Id -Force
    }
}

Write-Host "=== Memory measurement completed successfully ===" -ForegroundColor Cyan
exit 0
