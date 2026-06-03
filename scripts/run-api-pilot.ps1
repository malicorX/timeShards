# Production-pilot API: demo off, default passwords blocked. Set TIMESHARDS_ADMIN_PASSWORD first.
# Usage: $env:TIMESHARDS_ADMIN_PASSWORD = "YourSecret"; .\scripts\run-api-pilot.ps1

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "_pilot-env.ps1")
Write-Host "Pilot mode: DISABLE_DEMO=1, BLOCK_DEFAULT_PASSWORDS=1" -ForegroundColor Cyan
& (Join-Path $PSScriptRoot "run-api.ps1")
