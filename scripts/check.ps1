$ErrorActionPreference = "Stop"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $cargoBin = "$env:USERPROFILE\.cargo\bin"
    if (Test-Path $cargoBin) {
        $env:Path = "$cargoBin;$env:Path"
    }
}

function Run-Step {
    param([string]$Message, [scriptblock]$Action)
    Write-Host "`n--> $Message..."
    & $Action
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Verification failed during: $Message (Exit code: $LASTEXITCODE)"
        exit $LASTEXITCODE
    }
}

Write-Host "=== Fast Verification Suite ==="

Run-Step "Rust formatting check" { cargo fmt --all -- --check }
Run-Step "Rust Clippy check" { cargo clippy --workspace --all-targets -- -D warnings }
Run-Step "Rust workspace tests" { cargo test --workspace }
Run-Step "UI Typecheck" { npm run typecheck }
Run-Step "UI Unit tests" { npm run test:unit }
Run-Step "UI Production Build" { npm run build:ui }
Run-Step "Doctor check" { cargo xtask doctor }

Write-Host "`nAll verification checks PASSED successfully."
