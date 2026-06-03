# Headless TimeShards API (no Tauri). Ctrl+C to stop.
# Usage: .\scripts\run-api.ps1
#        $env:TIMESHARDS_DB = "M:\path\custom.db"; .\scripts\run-api.ps1
#        $env:TIMESHARDS_ADMIN_PASSWORD = "secret"; $env:TIMESHARDS_DISABLE_DEMO = "1"; .\scripts\run-api.ps1
#        $env:TIMESHARDS_BLOCK_DEFAULT_PASSWORDS = "1"; .\scripts\run-api.ps1   # staging: block admin/demo/manager defaults

$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$DataDir = Join-Path $RepoRoot ".data"

if (-not $env:TIMESHARDS_DB) {
    if (-not (Test-Path $DataDir)) {
        New-Item -ItemType Directory -Path $DataDir | Out-Null
    }
    $env:TIMESHARDS_DB = Join-Path $DataDir "timeshards-api.db"
}

if (-not $env:TIMESHARDS_API_HOST) {
    $env:TIMESHARDS_API_HOST = "127.0.0.1"
}
if (-not $env:TIMESHARDS_API_PORT) {
    $env:TIMESHARDS_API_PORT = "47821"
}

$env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path
Set-Location $RepoRoot

Write-Host "TimeShards API"
Write-Host "  DB:   $($env:TIMESHARDS_DB)"
Write-Host "  URL:  http://$($env:TIMESHARDS_API_HOST):$($env:TIMESHARDS_API_PORT)"
if ($env:TIMESHARDS_HW_ADAPTER) {
    Write-Host "  HW:   adapter=$($env:TIMESHARDS_HW_ADAPTER) tcp=$($env:TIMESHARDS_HW_TCP_ADDR)"
}
Write-Host "  Logins: admin/admin, demo/demo, manager/demo (unless DISABLE_DEMO / BLOCK_DEFAULT_PASSWORDS)"
Write-Host "  Env reference: .env.example"
Write-Host ""

cargo run --bin timeshards-api
