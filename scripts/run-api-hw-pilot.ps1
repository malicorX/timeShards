# Pilot API with external hardware TCP ingest (M4 site bridge path).
# Usage: $env:TIMESHARDS_ADMIN_PASSWORD = "YourSecret"; .\scripts\run-api-hw-pilot.ps1

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "_pilot-env.ps1")

$env:TIMESHARDS_HW_ADAPTER = "external"
if (-not $env:TIMESHARDS_HW_TCP_ADDR) {
    $env:TIMESHARDS_HW_TCP_ADDR = "127.0.0.1:47831"
}

Write-Host "Pilot + external HW: TCP $($env:TIMESHARDS_HW_TCP_ADDR)" -ForegroundColor Cyan
& (Join-Path $PSScriptRoot "run-api.ps1")
