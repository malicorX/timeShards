# Staging smoke: demo seeding on, but default passwords blocked at login.
# Usage: .\scripts\smoke-strict.ps1

param(
    [string]$ApiUrl = "http://127.0.0.1:47821",
    [int]$HealthTimeoutSec = 0
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "_smoke-api.ps1")
if ($HealthTimeoutSec -le 0) { $HealthTimeoutSec = Get-SmokeHealthTimeoutSec -DefaultSec 90 }
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$DataDir = Join-Path $RepoRoot ".data"
$DbPath = Join-Path $DataDir "smoke-strict.db"

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
$env:TIMESHARDS_BLOCK_DEFAULT_PASSWORDS = "1"
$env:TIMESHARDS_HW_ADAPTER = "sim"
Remove-Item Env:TIMESHARDS_DISABLE_DEMO -ErrorAction SilentlyContinue
Remove-Item Env:TIMESHARDS_HW_TCP_ADDR -ErrorAction SilentlyContinue
Remove-Item Env:TIMESHARDS_ADMIN_PASSWORD -ErrorAction SilentlyContinue

Write-Host "Starting API (strict passwords, demo seed on, db: $DbPath)..."
$apiJob = Start-Job -ScriptBlock {
    param($Root, $Db, $ApiHost, $Port)
    Set-Location $Root
    $env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path
    $env:TIMESHARDS_DB = $Db
    $env:TIMESHARDS_API_HOST = $ApiHost
    $env:TIMESHARDS_API_PORT = $Port
    $env:TIMESHARDS_BLOCK_DEFAULT_PASSWORDS = "1"
    $env:TIMESHARDS_HW_ADAPTER = "sim"
    Remove-Item Env:TIMESHARDS_DISABLE_DEMO -ErrorAction SilentlyContinue
    Remove-Item Env:TIMESHARDS_HW_TCP_ADDR -ErrorAction SilentlyContinue
    Remove-Item Env:TIMESHARDS_ADMIN_PASSWORD -ErrorAction SilentlyContinue
    cargo run -q --bin timeshards-api 2>&1
} -ArgumentList $RepoRoot, $DbPath, $env:TIMESHARDS_API_HOST, $env:TIMESHARDS_API_PORT

$healthUrl = "$ApiUrl/api/v1/health"
Write-Host "Waiting for $healthUrl (max ${HealthTimeoutSec}s, strict)..."
$health = Wait-TimeshardsApiHealth -HealthUrl $healthUrl -TimeoutSec $HealthTimeoutSec -ApiJob $apiJob -ReadyWhen {
    param($h)
    $h.status -eq 'ok' -and $h.demo_seeding_enabled -eq $true -and $h.default_password_login_blocked -eq $true
}
if ($health.demo_seeding_enabled -ne $true) {
    throw "Expected demo_seeding_enabled=true in strict smoke"
}
if ($health.default_password_login_blocked -ne $true) {
    throw "Expected default_password_login_blocked=true with TIMESHARDS_BLOCK_DEFAULT_PASSWORDS=1"
}
Write-Host "  demo_seeding=true default_password_login_blocked=true"

foreach ($pair in @(
        @{ u = "admin"; p = "admin" },
        @{ u = "demo"; p = "demo" }
    )) {
    $body = @{ username = $pair.u; password = $pair.p } | ConvertTo-Json
    try {
        Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/auth/login" `
            -Body $body -ContentType "application/json" | Out-Null
        throw "Expected $($pair.u) login to be forbidden in strict mode"
    } catch {
        $status = $null
        if ($_.Exception.Response) { $status = [int]$_.Exception.Response.StatusCode }
        if ($status -ne 403) {
            throw "Expected HTTP 403 for $($pair.u), got $status"
        }
    }
    Write-Host "  $($pair.u) default password rejected (403)"
}

Stop-Job $apiJob -ErrorAction SilentlyContinue
Remove-Job $apiJob -Force -ErrorAction SilentlyContinue
Stop-TimeshardsApiProcess

Write-Host ""
Write-Host "Strict smoke OK."
