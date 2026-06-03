# Pilot desktop stack: Server (production env) + Client after API health.
# Usage: $env:TIMESHARDS_ADMIN_PASSWORD = "YourSecret"; .\scripts\start-pilot.ps1

param(
    [int]$HealthTimeoutSec = 180,
    [string]$ApiUrl = "http://127.0.0.1:47821"
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "_pilot-env.ps1")

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$ServerScript = Join-Path $RepoRoot "scripts\start_server-pilot.ps1"
$ClientScript = Join-Path $RepoRoot "scripts\start_client.ps1"

Write-Host "Pilot mode: demo off, default passwords blocked." -ForegroundColor Cyan
Write-Host "  Admin login: user admin + TIMESHARDS_ADMIN_PASSWORD (not admin/admin)" -ForegroundColor Cyan
Write-Host ""

if (-not (Test-Path $ServerScript)) {
    Write-Error "Missing $ServerScript"
}

Write-Host "Launching TimeShards Server (pilot env) in a new window..."
Start-Process powershell -ArgumentList @(
    "-NoExit",
    "-ExecutionPolicy", "Bypass",
    "-File", $ServerScript,
    "-SkipNpmInstall"
) -WorkingDirectory $RepoRoot

$healthUrl = "$ApiUrl/api/v1/health"
Write-Host "Waiting for API at $healthUrl (max ${HealthTimeoutSec}s, demo off)..."
$deadline = (Get-Date).AddSeconds($HealthTimeoutSec)
$ready = $false

while ((Get-Date) -lt $deadline) {
    try {
        $r = Invoke-RestMethod -Uri $healthUrl -TimeoutSec 3
        if ($r.status -eq "ok" -and $r.demo_seeding_enabled -eq $false) {
            $ready = $true
            Write-Host "  $($r.service) v$($r.version) — demo_seeding=$($r.demo_seeding_enabled)"
            if ($r.time_foundation) {
                $tf = $r.time_foundation
                Write-Host "  Zeitbasis: $($tf.workday_models) Modelle, $($tf.work_calendars) Kalender, $($tf.active_employees) MA aktiv"
            }
            break
        }
        if ($r.status -eq "ok" -and $r.demo_seeding_enabled -eq $true) {
            Write-Host "  API on $ApiUrl still has demo seeding — another server may be using port 47821." -ForegroundColor Yellow
        }
    } catch {
        Start-Sleep -Seconds 2
    }
}

if (-not $ready) {
    Write-Warning "Pilot API did not become healthy in time (expected demo_seeding=false)."
    Write-Host "  If port 47821 is busy, stop the other TimeShards instance and retry."
    Write-Host "  Or start client manually: .\scripts\start_client.ps1"
    exit 1
}

Write-Host "Pilot API is up. Starting client..."
& $ClientScript -SkipNpmInstall
