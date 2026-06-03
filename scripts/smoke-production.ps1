# Production-mode smoke: demo seeding off, default passwords rejected at login.
# Usage: .\scripts\smoke-production.ps1

param(
    [string]$ApiUrl = "http://127.0.0.1:47821",
    [int]$HealthTimeoutSec = 0
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "_smoke-api.ps1")
if ($HealthTimeoutSec -le 0) { $HealthTimeoutSec = Get-SmokeHealthTimeoutSec -DefaultSec 90 }
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$DataDir = Join-Path $RepoRoot ".data"
$DbPath = Join-Path $DataDir "smoke-production.db"

if (-not (Test-Path $DataDir)) {
    New-Item -ItemType Directory -Path $DataDir | Out-Null
}

if (Test-Path $DbPath) {
    Remove-Item -Force $DbPath
}

Stop-TimeshardsApiProcess
Build-TimeshardsApi -RepoRoot $RepoRoot

$env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path
$env:TIMESHARDS_DB = $DbPath
$env:TIMESHARDS_API_HOST = "127.0.0.1"
$env:TIMESHARDS_API_PORT = "47821"
$env:TIMESHARDS_DISABLE_DEMO = "1"
$env:TIMESHARDS_ADMIN_PASSWORD = "SmokeProd-9x!"
$env:TIMESHARDS_HW_ADAPTER = "sim"
Remove-Item Env:TIMESHARDS_HW_TCP_ADDR -ErrorAction SilentlyContinue
Remove-Item Env:TIMESHARDS_BLOCK_DEFAULT_PASSWORDS -ErrorAction SilentlyContinue

Write-Host "Starting API (production flags, db: $DbPath)..."
$apiJob = Start-Job -ScriptBlock {
    param($Root, $Db, $ApiHost, $Port, $AdminPassword)
    Set-Location $Root
    $env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path
    $env:TIMESHARDS_DB = $Db
    $env:TIMESHARDS_API_HOST = $ApiHost
    $env:TIMESHARDS_API_PORT = $Port
    $env:TIMESHARDS_DISABLE_DEMO = "1"
    $env:TIMESHARDS_ADMIN_PASSWORD = $AdminPassword
    $env:TIMESHARDS_HW_ADAPTER = "sim"
    Remove-Item Env:TIMESHARDS_HW_TCP_ADDR -ErrorAction SilentlyContinue
    Remove-Item Env:TIMESHARDS_BLOCK_DEFAULT_PASSWORDS -ErrorAction SilentlyContinue
    $exe = Join-Path $Root "target\debug\timeshards-api.exe"
    if (Test-Path $exe) { & $exe 2>&1 } else { cargo run -q --bin timeshards-api 2>&1 }
} -ArgumentList $RepoRoot, $DbPath, $env:TIMESHARDS_API_HOST, $env:TIMESHARDS_API_PORT, $env:TIMESHARDS_ADMIN_PASSWORD

$healthUrl = "$ApiUrl/api/v1/health"
Write-Host "Waiting for $healthUrl (max ${HealthTimeoutSec}s, production)..."
$health = Wait-TimeshardsApiHealth -HealthUrl $healthUrl -TimeoutSec $HealthTimeoutSec -ApiJob $apiJob -ReadyWhen {
    param($h) $h.status -eq 'ok' -and $h.demo_seeding_enabled -eq $false
}
Write-Host "API ready."

if ($health.demo_seeding_enabled -eq $true) {
    throw "Expected demo_seeding_enabled=false with TIMESHARDS_DISABLE_DEMO=1"
}
if ($health.default_password_login_blocked -ne $true) {
    throw "Expected default_password_login_blocked=true in production smoke"
}
Write-Host "  demo_seeding_enabled=false default_password_login_blocked=true"
if (-not $health.time_foundation) {
    throw "Expected time_foundation on health in production smoke"
}
$tf = $health.time_foundation
Write-Host "  time_foundation: models=$($tf.workday_models) calendars=$($tf.work_calendars) active_ma=$($tf.active_employees) no_calendar=$($tf.employees_without_work_calendar) kw_no_soll=$($tf.current_week_drafts_without_soll)"
if ($tf.workday_models -lt 1 -or $tf.work_calendars -lt 1) {
    throw "Work calendar foundation not seeded in production mode"
}
if ($tf.employees_without_work_calendar -gt 0) {
    throw "Expected all active employees to have work calendar in production smoke"
}
if ($tf.current_week_drafts_without_soll -gt 0) {
    throw "Expected current_week_drafts_without_soll=0 in production smoke"
}

Write-Host "Default password login blocked..."
$loginBody = @{ username = "admin"; password = "admin" } | ConvertTo-Json
try {
    Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/auth/login" `
        -Body $loginBody -ContentType "application/json" | Out-Null
    throw "Expected admin/admin login to be forbidden in production mode"
} catch {
    $status = $null
    if ($_.Exception.Response) { $status = [int]$_.Exception.Response.StatusCode }
    if ($status -ne 403) {
        throw "Expected HTTP 403 for default admin login, got $status ($($_.Exception.Message))"
    }
}
Write-Host "  admin/admin rejected (403)"

Write-Host "Custom admin password login..."
$prodBody = @{ username = "admin"; password = $env:TIMESHARDS_ADMIN_PASSWORD } | ConvertTo-Json
$prodLogin = Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/auth/login" `
    -Body $prodBody -ContentType "application/json"
if ($prodLogin.user.username -ne "admin") {
    throw "Expected admin login with TIMESHARDS_ADMIN_PASSWORD"
}
Write-Host "  admin with env password OK"

Stop-Job $apiJob -ErrorAction SilentlyContinue
Remove-Job $apiJob -Force -ErrorAction SilentlyContinue
Stop-TimeshardsApiProcess

Write-Host ""
Write-Host "Production smoke OK."
