$ErrorActionPreference = "Stop"

Write-Host "=== Fast Verification Suite ==="

Write-Host "`n--> Rust formatting check..."
cargo fmt --all -- --check

Write-Host "`n--> Rust Clippy check..."
cargo clippy --workspace --all-targets -- -D warnings

Write-Host "`n--> Rust workspace tests..."
cargo test --workspace

Write-Host "`n--> UI Typecheck..."
npm run typecheck

Write-Host "`n--> UI Production Build..."
npm run build:ui

Write-Host "`n--> Doctor check..."
cargo xtask doctor

Write-Host "`nAll verification checks PASSED successfully."
