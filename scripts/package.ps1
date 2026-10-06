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

# Optional code signing (active only if secrets are configured)
$stagedBinaries = Get-ChildItem -Path (Join-Path $stageDir "*") -Include "*.exe", "*.msi"
if ($env:WINDOWS_CERT_PFX_BASE64 -and $env:WINDOWS_CERT_PASSWORD) {
    Write-Host "`n--> Signing staged binaries with configured certificate..."
    $pfxPath = [IO.Path]::GetTempFileName() + ".pfx"
    try {
        [IO.File]::WriteAllBytes($pfxPath, [Convert]::FromBase64String($env:WINDOWS_CERT_PFX_BASE64))
        $signtoolCmd = Get-Command signtool.exe -ErrorAction SilentlyContinue
        $signtool = if ($signtoolCmd) { $signtoolCmd.Source } else { $null }
        if (!$signtool) {
            $signtoolPaths = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin\*\x64\signtool.exe" -ErrorAction SilentlyContinue
            if ($signtoolPaths) { $signtool = $signtoolPaths[-1].FullName }
        }

        if ($signtool) {
            foreach ($binary in $stagedBinaries) {
                Write-Host "Signing $($binary.Name)..."
                & $signtool sign /f $pfxPath /p $env:WINDOWS_CERT_PASSWORD /fd SHA256 /tr "http://timestamp.digicert.com" /td SHA256 $binary.FullName
            }
        } else {
            Write-Warning "signtool.exe not found on system; signing skipped."
        }
    } finally {
        if (Test-Path $pfxPath) { Remove-Item $pfxPath -Force }
    }
} else {
    Write-Host "`n[INFO] Code signing secrets not provided; binaries remain unsigned."
}

# Authenticode signature verification report
Write-Host "`n--> Authenticode Signature Status:"
foreach ($binary in $stagedBinaries) {
    $sig = Get-AuthenticodeSignature -FilePath $binary.FullName
    Write-Host "  $($binary.Name): $($sig.Status)"
}

# Generate SHA-256 checksums
$checksumLines = foreach ($item in $stagedBinaries) {
    $hash = (Get-FileHash -Path $item.FullName -Algorithm SHA256).Hash.ToLower()
    "$hash  $($item.Name)"
}
$checksumPath = Join-Path $stageDir "SHA256SUMS.txt"
$utf8NoBom = New-Object Text.UTF8Encoding $false
[IO.File]::WriteAllLines($checksumPath, $checksumLines, $utf8NoBom)

# Generate BUILD_METADATA.json from real artifacts
$rootPackage = Get-Content (Join-Path $PSScriptRoot "..\package.json") -Raw | ConvertFrom-Json
$desktopPackage = Get-Content (Join-Path $PSScriptRoot "..\apps\desktop\package.json") -Raw | ConvertFrom-Json
$gitSha = (git rev-parse HEAD 2>$null)
if (!$gitSha) { $gitSha = "unknown" }
$buildDate = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
$rustcVer = (rustc --version 2>$null)
$nodeVer = (node --version 2>$null)
$npmVer = (npm --version 2>$null)
$tauriVer = $desktopPackage.devDependencies.'@tauri-apps/cli'

$artifactList = foreach ($item in $stagedBinaries) {
    $format = if ($item.Extension -eq ".msi") {
        "msi"
    } elseif ($item.Name -like "*-setup.exe") {
        "nsis"
    } else {
        "executable"
    }
    $hash = (Get-FileHash -Path $item.FullName -Algorithm SHA256).Hash.ToUpper()
    [ordered]@{
        fileName = $item.Name
        format = $format
        sha256 = $hash
        sizeBytes = $item.Length
    }
}

$metadata = [ordered]@{
    productName = "Rust Explorer"
    version = $rootPackage.version
    commitSha = $gitSha
    buildProfile = "release"
    targetTriple = "x86_64-pc-windows-msvc"
    rustcVersion = $rustcVer
    nodeVersion = $nodeVer
    npmVersion = $npmVer
    tauriCliVersion = $tauriVer
    buildDate = $buildDate
    artifacts = $artifactList
}

$metadataJson = $metadata | ConvertTo-Json -Depth 5
$metadataPath = Join-Path $stageDir "BUILD_METADATA.json"
[IO.File]::WriteAllText($metadataPath, $metadataJson + "`n", $utf8NoBom)

Write-Host "`n[OK] Release packaging completed successfully."
Write-Host "Staged release artifacts in: $stageDir"
Get-Content $checksumPath
Write-Host "`nGenerated metadata:"
Get-Content $metadataPath
