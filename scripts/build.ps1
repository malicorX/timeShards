# Build TimeShards Server and Client installers (Tauri release).
# Usage: .\scripts\build.ps1
# First run can take 10+ minutes.

param(
    [switch]$SkipNpmInstall
)

$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path

Set-Location $RepoRoot

if (-not $SkipNpmInstall -and -not (Test-Path "$RepoRoot\node_modules")) {
    Write-Host "Installing npm dependencies..."
    npm install
}

Write-Host "Building TimeShards Server..."
npm run tauri:server -- build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host ""
Write-Host "Building TimeShards Client..."
npm run tauri:client -- build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host ""
Write-Host "Done. Installers are under:"
Write-Host "  apps\server\src-tauri\target\release\bundle\"
Write-Host "  apps\client\src-tauri\target\release\bundle\"
