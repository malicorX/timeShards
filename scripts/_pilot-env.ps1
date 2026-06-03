# Shared production-pilot environment (dot-source from run-api-pilot / start_server-pilot).
# Requires TIMESHARDS_ADMIN_PASSWORD before sourcing.

$ErrorActionPreference = "Stop"

if (-not $env:TIMESHARDS_ADMIN_PASSWORD) {
    Write-Host "Set TIMESHARDS_ADMIN_PASSWORD before pilot mode." -ForegroundColor Yellow
    Write-Host '  Example: $env:TIMESHARDS_ADMIN_PASSWORD = "YourSecret"; .\scripts\start-pilot.ps1'
    exit 1
}

$env:TIMESHARDS_DISABLE_DEMO = "1"
$env:TIMESHARDS_BLOCK_DEFAULT_PASSWORDS = "1"
if (-not $env:TIMESHARDS_HW_ADAPTER) {
    $env:TIMESHARDS_HW_ADAPTER = "sim"
}
