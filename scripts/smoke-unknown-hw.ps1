# Verify invalid TIMESHARDS_HW_ADAPTER falls back to sim (API still starts).
# Usage: .\scripts\smoke-unknown-hw.ps1

param(
    [string]$ApiUrl = "http://127.0.0.1:47821",
    [int]$HealthTimeoutSec = 60
)

$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$DbPath = Join-Path $RepoRoot ".data\smoke-unknown-hw.db"
if (Test-Path $DbPath) { Remove-Item -Force $DbPath }

$apiJob = Start-Job -ScriptBlock {
    param($Root, $Db, $ApiHost, $Port)
    Set-Location $Root
    $env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path
    $env:TIMESHARDS_DB = $Db
    $env:TIMESHARDS_API_HOST = $ApiHost
    $env:TIMESHARDS_API_PORT = $Port
    $env:TIMESHARDS_HW_ADAPTER = "not-a-real-adapter"
    Remove-Item Env:TIMESHARDS_HW_TCP_ADDR -ErrorAction SilentlyContinue
    cargo run -q --bin timeshards-api 2>&1
} -ArgumentList $RepoRoot, $DbPath, "127.0.0.1", "47821"

$healthUrl = "$ApiUrl/api/v1/health"
$deadline = (Get-Date).AddSeconds($HealthTimeoutSec)
while ((Get-Date) -lt $deadline) {
    try {
        $h = Invoke-RestMethod -Uri $healthUrl -TimeoutSec 2
        if ($h.status -eq "ok") { break }
    } catch { }
    Start-Sleep -Seconds 1
}

$health = Invoke-RestMethod -Uri $healthUrl
if ($health.hardware_adapter -ne "sim") {
    throw "Expected active hardware_adapter=sim, got $($health.hardware_adapter)"
}
if ($health.hardware_adapter_configured -ne "unknown") {
    throw "Expected hardware_adapter_configured=unknown, got $($health.hardware_adapter_configured)"
}
Write-Host "  fallback OK (active=sim, configured=unknown)"

Stop-Job $apiJob -ErrorAction SilentlyContinue
Remove-Job $apiJob -Force -ErrorAction SilentlyContinue
Get-Process timeshards-api -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Write-Host "Unknown HW adapter smoke OK."
