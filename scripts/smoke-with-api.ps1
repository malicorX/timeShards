# Starts headless API, runs smoke-test.ps1, then stops the API.
# Usage: .\scripts\smoke-with-api.ps1

param(
    [string]$ApiUrl = "http://127.0.0.1:47821",
    [int]$HealthTimeoutSec = 0
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "_smoke-api.ps1")
if ($HealthTimeoutSec -le 0) { $HealthTimeoutSec = Get-SmokeHealthTimeoutSec -DefaultSec 90 }
$ApiUrl = Resolve-SmokeApiUrl -ApiUrl $ApiUrl
$apiUri = [Uri]$ApiUrl
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$DataDir = Join-Path $RepoRoot ".data"
$DbPath = Join-Path $DataDir "smoke.db"

if (-not (Test-Path $DataDir)) {
    New-Item -ItemType Directory -Path $DataDir | Out-Null
}

if (Test-Path $DbPath) {
    Remove-Item -Force $DbPath
    Write-Host "Removed previous smoke DB for a clean run."
}

Stop-TimeshardsApiProcess
Build-TimeshardsApi -RepoRoot $RepoRoot

$env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path
$env:TIMESHARDS_DB = $DbPath
$env:TIMESHARDS_API_HOST = $apiUri.Host
$env:TIMESHARDS_API_PORT = "$($apiUri.Port)"
$env:TIMESHARDS_HW_ADAPTER = "sim"
Remove-Item Env:TIMESHARDS_HW_TCP_ADDR -ErrorAction SilentlyContinue
Remove-Item Env:TIMESHARDS_DISABLE_DEMO -ErrorAction SilentlyContinue
Remove-Item Env:TIMESHARDS_BLOCK_DEFAULT_PASSWORDS -ErrorAction SilentlyContinue
Remove-Item Env:TIMESHARDS_ADMIN_PASSWORD -ErrorAction SilentlyContinue

Write-Host "Starting headless API ($ApiUrl, db: $DbPath, hw=sim)..."
$apiJob = Start-Job -ScriptBlock {
    param($Root, $Db, $ApiHost, $Port)
    Set-Location $Root
    $env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path
    $env:TIMESHARDS_DB = $Db
    $env:TIMESHARDS_API_HOST = $ApiHost
    $env:TIMESHARDS_API_PORT = $Port
    $env:TIMESHARDS_HW_ADAPTER = "sim"
    Remove-Item Env:TIMESHARDS_HW_TCP_ADDR -ErrorAction SilentlyContinue
    Remove-Item Env:TIMESHARDS_DISABLE_DEMO -ErrorAction SilentlyContinue
    Remove-Item Env:TIMESHARDS_BLOCK_DEFAULT_PASSWORDS -ErrorAction SilentlyContinue
    Remove-Item Env:TIMESHARDS_ADMIN_PASSWORD -ErrorAction SilentlyContinue
    $exe = Join-Path $Root "target\debug\timeshards-api.exe"
    if (Test-Path $exe) { & $exe 2>&1 } else { cargo run -q --bin timeshards-api 2>&1 }
} -ArgumentList $RepoRoot, $DbPath, $env:TIMESHARDS_API_HOST, $env:TIMESHARDS_API_PORT

Wait-TcpPortOpen -HostName $apiUri.Host -Port $apiUri.Port -TimeoutSec 30
$healthUrl = "$ApiUrl/api/v1/health"
Write-Host "Waiting for $healthUrl (max ${HealthTimeoutSec}s, demo seed)..."
Wait-TimeshardsApiHealth -HealthUrl $healthUrl -TimeoutSec $HealthTimeoutSec -ApiJob $apiJob -ReadyWhen {
    param($h) $h.status -eq 'ok' -and $h.demo_seeding_enabled -eq $true
} | Out-Null

Write-Host "API ready.`n"
try {
    & (Join-Path $PSScriptRoot "smoke-test.ps1") -ApiUrl $ApiUrl
    & (Join-Path $PSScriptRoot "verify-door-readers.ps1") -ApiUrl $ApiUrl
} finally {
    Write-Host "`nStopping API..."
    Stop-Job $apiJob -ErrorAction SilentlyContinue
    Remove-Job $apiJob -Force -ErrorAction SilentlyContinue
    Stop-TimeshardsApiProcess
}
