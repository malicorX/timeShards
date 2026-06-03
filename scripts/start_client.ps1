# Starts the TimeShards Client (employee desktop UI) in development mode.
# Start the server first: .\scripts\start_server.ps1
# Usage: .\scripts\start_client.ps1
#        .\scripts\start_client.ps1 -SkipNpmInstall

param(
    [switch]$SkipNpmInstall
)

$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
    Write-Error "Node.js/npm not found. Install Node 20+ from https://nodejs.org/"
}
if (-not (Test-Path "$env:USERPROFILE\.cargo\bin\cargo.exe")) {
    Write-Error "Rust/cargo not found. Install from https://rustup.rs/ (and VS C++ Build Tools on Windows)."
}

$env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path
Set-Location $RepoRoot

if (-not $SkipNpmInstall -and -not (Test-Path "$RepoRoot\node_modules")) {
    Write-Host "Installing npm dependencies (first time)..."
    npm install
}

Write-Host ""
Write-Host "Starting TimeShards Client (Tauri dev)..."
Write-Host "  Server URL (same PC): http://127.0.0.1:47821"
Write-Host "  Login:                admin / admin"
Write-Host "  Stop:                 Ctrl+C in this window"
Write-Host ""

npm run tauri:client -- dev
