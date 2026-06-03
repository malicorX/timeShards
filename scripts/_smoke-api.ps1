# Shared helpers for headless API smoke scripts (dot-source from scripts/*.ps1).

function Stop-TimeshardsApiProcess {
    Get-Process -Name "timeshards-api" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 1
}

function Build-TimeshardsApi {
    param([string]$RepoRoot)
    Push-Location $RepoRoot
    try {
        $env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path
        Write-Host "Building timeshards-api..."
        cargo build -q --bin timeshards-api
        if ($LASTEXITCODE -ne 0) { throw "cargo build --bin timeshards-api failed with exit $LASTEXITCODE" }
    } finally {
        Pop-Location
    }
}

function Get-SmokeHealthTimeoutSec {
    param([int]$DefaultSec = 90)
    # Headless smoke runs `cargo run` cold; allow time for compile + seed on CI and dev machines.
    if ($env:GITHUB_ACTIONS -eq 'true') { return [Math]::Max($DefaultSec, 240) }
    return [Math]::Max($DefaultSec, 150)
}

function Wait-TimeshardsApiHealth {
    param(
        [string]$HealthUrl,
        [int]$TimeoutSec = 90,
        [scriptblock]$ReadyWhen,
        $ApiJob = $null
    )
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        if ($ApiJob -and $ApiJob.State -eq 'Failed') {
            Receive-Job $ApiJob
            throw 'API process failed to start'
        }
        try {
            $h = Invoke-RestMethod -Uri $HealthUrl -TimeoutSec 3
            if (& $ReadyWhen $h) { return $h }
        } catch { }
        Start-Sleep -Seconds 2
    }
    if ($ApiJob) { Receive-Job $ApiJob -ErrorAction SilentlyContinue }
    throw "API did not become healthy in time ($HealthUrl)"
}
