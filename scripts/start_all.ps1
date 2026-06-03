# Starts TimeShards Server in a new window, waits for API health, then starts the Client.
# Usage: .\scripts\start_all.ps1

param(
    [int]$HealthTimeoutSec = 180,
    [string]$ApiUrl = "http://127.0.0.1:47821"
)

$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$ServerScript = Join-Path $RepoRoot "scripts\start_server.ps1"
$ClientScript = Join-Path $RepoRoot "scripts\start_client.ps1"

if (-not (Test-Path $ServerScript)) {
    Write-Error "Missing $ServerScript"
}

Write-Host "Launching TimeShards Server in a new window..."
Start-Process powershell -ArgumentList @(
    "-NoExit",
    "-ExecutionPolicy", "Bypass",
    "-File", $ServerScript,
    "-SkipNpmInstall"
) -WorkingDirectory $RepoRoot

$healthUrl = "$ApiUrl/api/v1/health"
Write-Host "Waiting for API at $healthUrl (max ${HealthTimeoutSec}s)..."
$deadline = (Get-Date).AddSeconds($HealthTimeoutSec)
$ready = $false

while ((Get-Date) -lt $deadline) {
    try {
        $r = Invoke-RestMethod -Uri $healthUrl -TimeoutSec 3
        if ($r.status -eq "ok") {
            $ready = $true
            break
        }
    } catch {
        Start-Sleep -Seconds 2
    }
}

if (-not $ready) {
    Write-Warning "API did not respond in time. Start the client manually when the server is ready:"
    Write-Host "  .\scripts\start_client.ps1"
    exit 1
}

Write-Host "API is up. Starting client..."
& $ClientScript -SkipNpmInstall
