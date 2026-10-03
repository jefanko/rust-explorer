$ErrorActionPreference = "Stop"

$cargoBin = "$env:USERPROFILE\.cargo\bin"
if (Test-Path $cargoBin) {
    $env:Path = "$cargoBin;$env:Path"
}

Write-Host "=== Rust Explorer Packaging & Installer Build ==="
Write-Host "Node version: $(node --version)"
Write-Host "Cargo version: $(cargo --version)"

# Build release desktop bundle
Write-Host "`n--> Building Tauri release bundle..."
npx --workspace=@rust-explorer/desktop tauri build

if ($LASTEXITCODE -ne 0) {
    Write-Error "Tauri build failed with exit code $LASTEXITCODE"
    exit $LASTEXITCODE
}

Write-Host "`n[OK] Release packaging completed successfully."
