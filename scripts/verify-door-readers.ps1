# Verify door / reader mapping for hardware bridge pilots (API must be running).
# Usage: .\scripts\verify-door-readers.ps1
#        .\scripts\verify-door-readers.ps1 -ApiUrl http://127.0.0.1:47821

param(
    [string]$ApiUrl = "http://127.0.0.1:47821",
    [string]$Username = "admin",
    [string]$Password = "admin"
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "_smoke-api.ps1")

Write-Host "Door / reader mapping check ($ApiUrl)..."

$health = Invoke-RestMethod -Uri "$ApiUrl/api/v1/health" -TimeoutSec 5
Write-Host "  API: $($health.service) v$($health.version) hw=$($health.hardware_adapter)"
if ($health.hardware_tcp_listen) {
    Write-Host "  TCP ingest: $($health.hardware_tcp_listen)"
}

$loginBody = @{ username = $Username; password = $Password } | ConvertTo-Json
$login = Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/auth/login" `
    -Body $loginBody -ContentType "application/json"
$headers = @{ Authorization = "Bearer $($login.token)" }

$doors = @(Invoke-RestMethod -Uri "$ApiUrl/api/v1/access/doors" -Headers $headers)
if ($doors.Count -eq 0) {
    Write-Host "  WARN: No doors configured. Create zone + door in Zutritt tab."
    exit 1
}

$issues = 0
Write-Host ""
Write-Host "  Bridge must send these reader_id values:"
Write-Host "  door_id                  | door name        | reader_in        | reader_out"
$rule = ('-' * 25) + '+' + ('-' * 18) + '+' + ('-' * 18) + '+' + ('-' * 18)
Write-Host "  $rule"
foreach ($d in $doors) {
    $readerIn = if ($d.reader_in_id) { $d.reader_in_id } else { "(missing)" }
    $readerOut = if ($d.reader_out_id) { $d.reader_out_id } else { "(optional)" }
    $doorIdShort = $d.id.Substring(0, [Math]::Min(24, $d.id.Length))
    Write-Host ("  {0,-24}| {1,-16}| {2,-16}| {3}" -f $doorIdShort, $d.name, $readerIn, $readerOut)
    if (-not $d.reader_in_id) {
        Write-Host "    WARN: No entry reader_id (anti-passback / TCP need reader_in_id)" -ForegroundColor Yellow
        $issues++
    }
}

$rules = @(Invoke-RestMethod -Uri "$ApiUrl/api/v1/access/rules" -Headers $headers)
$allowCount = @($rules | Where-Object { $_.mode -eq "allow" }).Count
Write-Host ""
Write-Host "  Allow rules: $allowCount (fail-closed: no rule = no grant)"
if ($allowCount -lt 1) {
    Write-Host "  WARN: No allow rules. Assign Zutritt or use Personal setup." -ForegroundColor Yellow
    $issues++
}

if ($issues -gt 0) {
    Write-Host ""
    Write-Host "Door reader verify: $issues issue(s). Fix before bridge pilot." -ForegroundColor Yellow
    exit 1
}

Write-Host ""
Write-Host "Door reader verify OK."
Write-Host "  Test: .\scripts\send-hw-tcp.ps1 -CredentialUid DEMO-0002 -ReaderId sim.reader.main"
