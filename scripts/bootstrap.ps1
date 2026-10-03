$ErrorActionPreference = "Stop"

Write-Host "=== Rust Explorer Bootstrap ==="

# Check toolchains
Get-Command git, rustup, cargo, node, npm -ErrorAction Stop

# Install npm dependencies
Write-Host "Installing frontend dependencies..."
npm ci

# Fetch Rust dependencies
Write-Host "Fetching Cargo dependencies..."
cargo fetch

# Run doctor
Write-Host "Running xtask doctor..."
cargo xtask doctor

Write-Host "Bootstrap completed successfully."
