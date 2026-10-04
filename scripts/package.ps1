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

# Stage release artifacts
$outputDir = Resolve-Path (Join-Path $PSScriptRoot "..\target\release")
$stageDir = Join-Path $outputDir "..\release-artifacts"
New-Item -ItemType Directory -Force -Path $stageDir | Out-Null

$exe = Join-Path $outputDir "rust-explorer.exe"
if (Test-Path $exe) {
    Copy-Item $exe -Destination $stageDir -Force
}

$nsisFiles = Get-ChildItem -Path (Join-Path $outputDir "bundle\nsis\*.exe") -ErrorAction SilentlyContinue
foreach ($f in $nsisFiles) {
    Copy-Item $f.FullName -Destination $stageDir -Force
}

$msiFiles = Get-ChildItem -Path (Join-Path $outputDir "bundle\msi\*.msi") -ErrorAction SilentlyContinue
foreach ($f in $msiFiles) {
    Copy-Item $f.FullName -Destination $stageDir -Force
}

# Generate SHA-256 checksums
$stagedItems = Get-ChildItem -Path (Join-Path $stageDir "*") -Include "*.exe", "*.msi"
$checksumLines = foreach ($item in $stagedItems) {
    $hash = (Get-FileHash -Path $item.FullName -Algorithm SHA256).Hash.ToLower()
    "$hash  $($item.Name)"
}
$checksumPath = Join-Path $stageDir "SHA256SUMS.txt"
$checksumLines | Set-Content -Path $checksumPath -Encoding utf8

Write-Host "`n[OK] Release packaging completed successfully."
Write-Host "Staged release artifacts in: $stageDir"
Get-Content $checksumPath
