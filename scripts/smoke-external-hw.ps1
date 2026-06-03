# Verify external hardware adapter stub boots (REST simulate still works).
# Usage: .\scripts\smoke-external-hw.ps1

param(
    [string]$ApiUrl = "http://127.0.0.1:47821",
    [int]$HealthTimeoutSec = 0
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "_smoke-api.ps1")
if ($HealthTimeoutSec -le 0) { $HealthTimeoutSec = Get-SmokeHealthTimeoutSec -DefaultSec 90 }

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$DataDir = Join-Path $RepoRoot ".data"
$DbPath = Join-Path $DataDir "smoke-external-hw.db"

if (-not (Test-Path $DataDir)) {
    New-Item -ItemType Directory -Path $DataDir | Out-Null
}
if (Test-Path $DbPath) { Remove-Item -Force $DbPath }

Stop-TimeshardsApiProcess
Build-TimeshardsApi -RepoRoot $RepoRoot

$env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path
$env:TIMESHARDS_DB = $DbPath
$env:TIMESHARDS_API_HOST = "127.0.0.1"
$env:TIMESHARDS_API_PORT = "47821"
$env:TIMESHARDS_HW_ADAPTER = "external"
if ($env:GITHUB_ACTIONS -eq 'true') {
    $hwPort = 47840 + (Get-Random -Maximum 200)
    $env:TIMESHARDS_HW_TCP_ADDR = "127.0.0.1:$hwPort"
    Write-Host "  CI: dynamic hardware TCP port $hwPort"
} else {
    $env:TIMESHARDS_HW_TCP_ADDR = "127.0.0.1:47831"
}
Remove-Item Env:TIMESHARDS_DISABLE_DEMO -ErrorAction SilentlyContinue
Remove-Item Env:TIMESHARDS_BLOCK_DEFAULT_PASSWORDS -ErrorAction SilentlyContinue

if ($env:GITHUB_STEP_SUMMARY) { Set-Content -Path $env:GITHUB_STEP_SUMMARY -Value "## External HW smoke`n" }
Trace-SmokeStep "start-api"
Write-Host "Starting API (TIMESHARDS_HW_ADAPTER=external, TCP $($env:TIMESHARDS_HW_TCP_ADDR))..."
$apiJob = Start-Job -ScriptBlock {
    param($Root, $Db, $ApiHost, $Port, $HwAdapter, $TcpAddr)
    Set-Location $Root
    $env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path
    $env:TIMESHARDS_DB = $Db
    $env:TIMESHARDS_API_HOST = $ApiHost
    $env:TIMESHARDS_API_PORT = $Port
    $env:TIMESHARDS_HW_ADAPTER = $HwAdapter
    if ($TcpAddr) { $env:TIMESHARDS_HW_TCP_ADDR = $TcpAddr }
    Remove-Item Env:TIMESHARDS_DISABLE_DEMO -ErrorAction SilentlyContinue
    Remove-Item Env:TIMESHARDS_BLOCK_DEFAULT_PASSWORDS -ErrorAction SilentlyContinue
    Remove-Item Env:TIMESHARDS_ADMIN_PASSWORD -ErrorAction SilentlyContinue
    $exe = Join-Path $Root "target\debug\timeshards-api.exe"
    if (Test-Path $exe) { & $exe 2>&1 } else { cargo run -q --bin timeshards-api 2>&1 }
} -ArgumentList $RepoRoot, $DbPath, $env:TIMESHARDS_API_HOST, $env:TIMESHARDS_API_PORT, $env:TIMESHARDS_HW_ADAPTER, $env:TIMESHARDS_HW_TCP_ADDR

$healthUrl = "$ApiUrl/api/v1/health"
Write-Host "Waiting for $healthUrl (max ${HealthTimeoutSec}s, hw=external)..."
$health = Wait-TimeshardsApiHealth -HealthUrl $healthUrl -TimeoutSec $HealthTimeoutSec -ApiJob $apiJob -ReadyWhen {
    param($h) $h.status -eq 'ok' -and $h.hardware_adapter -eq 'external'
}
Write-Host "  hardware_adapter=external"
$tcpListen = $health.hardware_tcp_listen
if (-not $tcpListen) {
    throw "Expected health.hardware_tcp_listen when TIMESHARDS_HW_TCP_ADDR is set"
}
$tcpHost, $tcpPortStr = $tcpListen -split ':', 2
$tcpPort = [int]$tcpPortStr
Wait-TcpPortOpen -HostName $tcpHost -Port $tcpPort -TimeoutSec 30
Write-Host "  TCP listen ready on $tcpListen"

Trace-SmokeStep "login-simulate"
$login = Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/auth/login" `
    -Body (@{ username = "admin"; password = "admin" } | ConvertTo-Json) -ContentType "application/json"
$headers = @{ Authorization = "Bearer $($login.token)" }
$scan = Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/access/simulate-scan" `
    -Headers $headers -Body (@{ reader_id = "sim.reader.main"; credential_uid = "DEMO-ADMIN-001" } | ConvertTo-Json) `
    -ContentType "application/json"
if ($scan.decision -notin @("grant", "allow")) {
    throw "Expected grant on simulate-scan with external adapter, got $($scan.decision)"
}
Write-Host "  REST simulate-scan OK ($($scan.decision))"

Trace-SmokeStep "hardware-present"
Write-Host "Hardware channel present..."
$evBefore = @(Invoke-RestMethod -Uri "$ApiUrl/api/v1/access/events?limit=5" -Headers $headers).Count
$body = @{ reader_id = "sim.reader.main.out"; credential_uid = "DEMO-ADMIN-001" } | ConvertTo-Json
$queued = Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/access/hardware-present" `
    -Headers $headers -Body $body -ContentType "application/json"
if (-not $queued.queued) {
    throw "Expected hardware-present queued=true"
}
Start-Sleep -Milliseconds $(if ($env:GITHUB_ACTIONS -eq 'true') { 1500 } else { 400 })
$ev = Invoke-RestMethod -Uri "$ApiUrl/api/v1/access/events?limit=5" -Headers $headers
if ($ev.Count -le $evBefore) {
    throw "Expected new access event after hardware-present"
}
if ($ev[0].decision -notin @("grant", "allow")) {
    throw "Expected grant on hardware-present path, got $($ev[0].decision)"
}
Write-Host "  hardware-present OK ($($ev[0].decision))"

Trace-SmokeStep "tcp-ingest"
Write-Host "TCP credential ingest..."
function Get-AccessEventCount {
    param([hashtable]$AuthHeaders)
    return (Get-SmokeAccessEvents -ApiUrl $ApiUrl -AuthHeaders $AuthHeaders).Count
}
$tcpBefore = Get-AccessEventCount -AuthHeaders $headers
$line = '{"reader_id":"sim.reader.main","credential_uid":"DEMO-0003"}'
$tcp = New-Object System.Net.Sockets.TcpClient
try {
    $tcp.Connect($tcpHost, $tcpPort)
    $stream = $tcp.GetStream()
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($line + "`n")
    $stream.Write($bytes, 0, $bytes.Length)
    $stream.Flush()
    Start-Sleep -Milliseconds 800
} finally {
    $tcp.Close()
}
$tcpAfter = Wait-CountIncreased -Before $tcpBefore -GetCount { Get-AccessEventCount -AuthHeaders $headers } -Label 'TCP JSON ingest'
$tcpEvents = Get-SmokeAccessEvents -ApiUrl $ApiUrl -AuthHeaders $headers
if ($tcpEvents[0].decision -notin @("grant", "allow")) {
    throw "Expected grant on TCP ingest path, got $($tcpEvents[0].decision)"
}
Write-Host "  TCP ingest OK ($($tcpEvents[0].decision))"

Write-Host "TCP compact line (reader;credential)..."
$tcpBefore2 = Get-AccessEventCount -AuthHeaders $headers
$tcp2 = New-Object System.Net.Sockets.TcpClient
try {
    $tcp2.Connect($tcpHost, $tcpPort)
    $stream2 = $tcp2.GetStream()
    $bytes2 = [System.Text.Encoding]::UTF8.GetBytes("sim.reader.main.out;DEMO-ADMIN-001`n")
    $stream2.Write($bytes2, 0, $bytes2.Length)
    $stream2.Flush()
    Start-Sleep -Milliseconds 800
} finally {
    $tcp2.Close()
}
$tcpAfter2 = Wait-CountIncreased -Before $tcpBefore2 -GetCount { Get-AccessEventCount -AuthHeaders $headers } -Label 'TCP compact ingest'
Write-Host "  TCP compact line OK"

$healthFinal = Invoke-RestMethod -Uri $healthUrl
if (-not $healthFinal.hardware_tcp_listen) {
    throw "Expected health.hardware_tcp_listen when TIMESHARDS_HW_TCP_ADDR is set"
}
Write-Host "  health.hardware_tcp_listen=$($healthFinal.hardware_tcp_listen)"

Write-Host "TCP door state (alarm)..."
$doors = @(Invoke-RestMethod -Uri "$ApiUrl/api/v1/access/doors" -Headers $headers)
if ($doors.Count -lt 1) {
    throw "Expected at least one door for door-state smoke"
}
$doorId = $doors[0].id
$doorLine = "door;$doorId;alarm"
$tcp3 = New-Object System.Net.Sockets.TcpClient
try {
    $tcp3.Connect($tcpHost, $tcpPort)
    $stream3 = $tcp3.GetStream()
    $bytes3 = [System.Text.Encoding]::UTF8.GetBytes($doorLine + "`n")
    $stream3.Write($bytes3, 0, $bytes3.Length)
    $stream3.Flush()
    Start-Sleep -Milliseconds 800
} finally {
    $tcp3.Close()
}
$doorRow = $null
$doorDeadline = (Get-Date).AddSeconds(15)
while ((Get-Date) -lt $doorDeadline) {
    Start-Sleep -Milliseconds (Get-SmokePollDelayMs)
    $doorsAfter = @(Invoke-RestMethod -Uri "$ApiUrl/api/v1/access/doors" -Headers $headers)
    $doorRow = $doorsAfter | Where-Object { $_.id -eq $doorId } | Select-Object -First 1
    if ($doorRow.status -eq 'alarm') { break }
}
if ($doorRow.status -ne "alarm") {
    throw "Expected door status alarm after TCP ingest, got $($doorRow.status)"
}
Write-Host "  TCP door state OK (alarm)"

Write-Host "TCP reader offline (audit)..."
$tcp4 = New-Object System.Net.Sockets.TcpClient
try {
    $tcp4.Connect($tcpHost, $tcpPort)
    $stream4 = $tcp4.GetStream()
    $bytes4 = [System.Text.Encoding]::UTF8.GetBytes("reader_offline;sim.reader.main`n")
    $stream4.Write($bytes4, 0, $bytes4.Length)
    $stream4.Flush()
    Start-Sleep -Milliseconds 800
} finally {
    $tcp4.Close()
}
$auditHw = @()
$auditDeadline = (Get-Date).AddSeconds(15)
while ((Get-Date) -lt $auditDeadline) {
    Start-Sleep -Milliseconds (Get-SmokePollDelayMs)
    $auditHw = @(
        Invoke-RestMethod -Uri "$ApiUrl/api/v1/admin/audit?actor_type=hardware&action=reader_offline&limit=10" -Headers $headers
    )
    if ($auditHw.Count -ge 1) { break }
}
if ($auditHw.Count -lt 1) {
    throw "Expected hardware reader_offline audit entry"
}
Write-Host "  reader_offline audit OK"

Stop-Job $apiJob -ErrorAction SilentlyContinue
Remove-Job $apiJob -Force -ErrorAction SilentlyContinue
Stop-TimeshardsApiProcess
Write-Host "External hardware smoke OK."
