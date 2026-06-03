# M4 hardware pilot: door mapping + one TCP scan + event check (API must be running, adapter=external).
# Usage: .\scripts\hw-pilot-test.ps1
#        $env:TIMESHARDS_ADMIN_PASSWORD = "secret"; .\scripts\hw-pilot-test.ps1 -ApiUrl http://127.0.0.1:47821

param(
    [string]$ApiUrl = "http://127.0.0.1:47821",
    [string]$Username = "admin",
    [string]$Password = "",
    [string]$CredentialUid = "DEMO-0002",
    [string]$ReaderId = "sim.reader.main"
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "_smoke-api.ps1")

if (-not $Password -and $env:TIMESHARDS_ADMIN_PASSWORD) {
    $Password = $env:TIMESHARDS_ADMIN_PASSWORD
}
if (-not $Password) {
    $Password = "admin"
}

Write-Host "=== Hardware pilot test ($ApiUrl) ===" -ForegroundColor Cyan

$health = Invoke-RestMethod -Uri "$ApiUrl/api/v1/health" -TimeoutSec 5
Write-Host "  adapter=$($health.hardware_adapter) tcp=$($health.hardware_tcp_listen)"
if ($health.hardware_adapter -ne "external") {
    throw "Expected hardware_adapter=external. Start API with TIMESHARDS_HW_ADAPTER=external (npm run api:hw-pilot)."
}
if (-not $health.hardware_tcp_listen) {
    throw "Expected hardware_tcp_listen in health. Set TIMESHARDS_HW_TCP_ADDR (e.g. 127.0.0.1:47831)."
}

& (Join-Path $PSScriptRoot "verify-door-readers.ps1") -ApiUrl $ApiUrl -Username $Username -Password $Password
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$tcpHost, $tcpPortStr = $health.hardware_tcp_listen -split ':', 2
$tcpPort = [int]$tcpPortStr

$loginBody = @{ username = $Username; password = $Password } | ConvertTo-Json
$login = Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/auth/login" `
    -Body $loginBody -ContentType "application/json"
$headers = @{ Authorization = "Bearer $($login.token)" }

$before = (Get-SmokeAccessEvents -ApiUrl $ApiUrl -AuthHeaders $headers -Query "limit=1").Count
& (Join-Path $PSScriptRoot "send-hw-tcp.ps1") -TcpHost $tcpHost -Port $tcpPort `
    -ReaderId $ReaderId -CredentialUid $CredentialUid

$deadline = (Get-Date).AddSeconds(10)
$found = $false
while ((Get-Date) -lt $deadline) {
    $events = @(Get-SmokeAccessEvents -ApiUrl $ApiUrl -AuthHeaders $headers -Query "limit=10")
    if ($events.Count -gt $before) {
        $latest = $events[0]
        Write-Host "  Latest event: decision=$($latest.decision) reason=$($latest.reason) uid=$($latest.credential_uid)"
        if ($latest.decision -eq "grant") {
            $found = $true
            break
        }
    }
    Start-Sleep -Milliseconds (Get-SmokePollDelayMs)
}

if (-not $found) {
    throw "Expected grant access event after TCP scan within 10s"
}

Write-Host ""
Write-Host "Hardware pilot test OK." -ForegroundColor Green
