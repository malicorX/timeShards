# Verify invalid TIMESHARDS_HW_ADAPTER falls back to sim (API still starts).
# Usage: .\scripts\smoke-unknown-hw.ps1

param(
    [string]$ApiUrl = "http://127.0.0.1:47821",
    [int]$HealthTimeoutSec = 0
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "_smoke-api.ps1")
if ($HealthTimeoutSec -le 0) { $HealthTimeoutSec = Get-SmokeHealthTimeoutSec -DefaultSec 90 }

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$DbPath = Join-Path $RepoRoot ".data\smoke-unknown-hw.db"
if (Test-Path $DbPath) { Remove-Item -Force $DbPath }

Stop-TimeshardsApiProcess
Build-TimeshardsApi -RepoRoot $RepoRoot

$apiJob = Start-Job -ScriptBlock {
    param($Root, $Db, $ApiHost, $Port)
    Set-Location $Root
    $env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path
    $env:TIMESHARDS_DB = $Db
    $env:TIMESHARDS_API_HOST = $ApiHost
    $env:TIMESHARDS_API_PORT = $Port
    $env:TIMESHARDS_HW_ADAPTER = "not-a-real-adapter"
    Remove-Item Env:TIMESHARDS_HW_TCP_ADDR -ErrorAction SilentlyContinue
    Remove-Item Env:TIMESHARDS_DISABLE_DEMO -ErrorAction SilentlyContinue
    Remove-Item Env:TIMESHARDS_BLOCK_DEFAULT_PASSWORDS -ErrorAction SilentlyContinue
    Remove-Item Env:TIMESHARDS_ADMIN_PASSWORD -ErrorAction SilentlyContinue
    cargo run -q --bin timeshards-api 2>&1
} -ArgumentList $RepoRoot, $DbPath, "127.0.0.1", "47821"

$healthUrl = "$ApiUrl/api/v1/health"
$health = Wait-TimeshardsApiHealth -HealthUrl $healthUrl -TimeoutSec $HealthTimeoutSec -ApiJob $apiJob -ReadyWhen {
    param($h)
    $h.status -eq 'ok' -and $h.hardware_adapter -eq 'sim' -and $h.hardware_adapter_configured -eq 'unknown'
}

if ($health.hardware_adapter -ne "sim") {
    throw "Expected active hardware_adapter=sim, got $($health.hardware_adapter)"
}
if ($health.hardware_adapter_configured -ne "unknown") {
    throw "Expected hardware_adapter_configured=unknown, got $($health.hardware_adapter_configured)"
}
Write-Host "  fallback OK (active=sim, configured=unknown)"

Stop-Job $apiJob -ErrorAction SilentlyContinue
Remove-Job $apiJob -Force -ErrorAction SilentlyContinue
Stop-TimeshardsApiProcess
Write-Host "Unknown HW adapter smoke OK."
