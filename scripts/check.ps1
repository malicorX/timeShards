# Runs Rust and frontend checks for the monorepo.
# Usage: .\scripts\check.ps1

$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path

Set-Location $RepoRoot

Write-Host "cargo check..."
cargo check --workspace
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host ""
Write-Host "svelte-check (server)..."
npm run check -w @timeshards/server
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host ""
Write-Host "svelte-check (client)..."
npm run check -w @timeshards/client
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host ""
Write-Host "All checks passed."
