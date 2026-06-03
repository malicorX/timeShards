# Starts the TimeShards Server (REST API + admin desktop UI) in development mode.
# Usage: .\scripts\start_server.ps1
#        .\scripts\start_server.ps1 -SkipNpmInstall

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
Write-Host "Starting TimeShards Server (Tauri dev)..."
Write-Host "  API:     http://127.0.0.1:47821"
if ($env:TIMESHARDS_DISABLE_DEMO -eq "1") {
    Write-Host "  Login:   admin + TIMESHARDS_ADMIN_PASSWORD (demo off)"
} else {
    Write-Host "  Login:   admin / admin"
}
Write-Host "  HW:      optional TIMESHARDS_HW_ADAPTER / TIMESHARDS_HW_TCP_ADDR (see .env.example)"
Write-Host "  Stop:    Ctrl+C in this window"
Write-Host ""

npm run tauri:server -- dev
