# Production-pilot API: demo off, default passwords blocked. Set TIMESHARDS_ADMIN_PASSWORD first.
# Usage: $env:TIMESHARDS_ADMIN_PASSWORD = "YourSecret"; .\scripts\run-api-pilot.ps1

$ErrorActionPreference = "Stop"

if (-not $env:TIMESHARDS_ADMIN_PASSWORD) {
    Write-Host "Set TIMESHARDS_ADMIN_PASSWORD before starting (empty DB uses it for admin)." -ForegroundColor Yellow
    Write-Host '  Example: $env:TIMESHARDS_ADMIN_PASSWORD = "YourSecret"; .\scripts\run-api-pilot.ps1'
    exit 1
}

$env:TIMESHARDS_DISABLE_DEMO = "1"
$env:TIMESHARDS_BLOCK_DEFAULT_PASSWORDS = "1"
if (-not $env:TIMESHARDS_HW_ADAPTER) {
    $env:TIMESHARDS_HW_ADAPTER = "sim"
}

Write-Host "Pilot mode: DISABLE_DEMO=1, BLOCK_DEFAULT_PASSWORDS=1" -ForegroundColor Cyan
& (Join-Path $PSScriptRoot "run-api.ps1")
