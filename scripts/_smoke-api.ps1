# Shared helpers for headless API smoke scripts (dot-source from scripts/*.ps1).

# Cargo/npm write progress to stderr; GHA PowerShell 7 treats that as NativeCommandError otherwise.
if (Get-Variable -Name PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
    $PSNativeCommandUseErrorActionPreference = $false
}

function Trace-SmokeStep {
    param([string]$Name)
    Write-Host ""
    Write-Host "=== $Name ==="
    if ($env:GITHUB_STEP_SUMMARY) {
        Add-Content -Path $env:GITHUB_STEP_SUMMARY -Value "- $Name"
    }
}

function Stop-TimeshardsApiProcess {
    Get-Process -Name "timeshards-api" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 1
}

function Build-TimeshardsApi {
    param([string]$RepoRoot)
    $exe = Join-Path $RepoRoot "target\debug\timeshards-api.exe"
    if ($env:GITHUB_ACTIONS -eq 'true' -and (Test-Path $exe)) {
        Write-Host "Using pre-built timeshards-api.exe"
        return
    }
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

# In Start-Job scriptblocks after env setup:
#   $exe = Join-Path $Root "target\debug\timeshards-api.exe"
#   if (Test-Path $exe) { & $exe 2>&1 } else { cargo run -q --bin timeshards-api 2>&1 }

function Get-SmokeHealthTimeoutSec {
    param([int]$DefaultSec = 90)
    # Headless smoke runs `cargo run` cold; allow time for compile + seed on CI and dev machines.
    if ($env:GITHUB_ACTIONS -eq 'true') { return [Math]::Max($DefaultSec, 240) }
    return [Math]::Max($DefaultSec, 150)
}

function Get-SmokePollDelayMs {
    if ($env:GITHUB_ACTIONS -eq 'true') { return 400 }
    return 200
}

function Test-LocalPortListening {
    param(
        [string]$HostName = '127.0.0.1',
        [int]$Port
    )
    $client = New-Object System.Net.Sockets.TcpClient
    try {
        $iar = $client.BeginConnect($HostName, $Port, $null, $null)
        if ($iar.AsyncWaitHandle.WaitOne(500) -and $client.Connected) {
            return $true
        }
    } catch { }
    finally {
        if ($client.Connected) { $client.Close() }
        $client.Dispose()
    }
    return $false
}

function Resolve-SmokeApiPort {
    param(
        [int]$PreferredPort = 47821,
        [string]$HostName = '127.0.0.1'
    )
    if (-not (Test-LocalPortListening -HostName $HostName -Port $PreferredPort)) {
        return $PreferredPort
    }
    for ($try = 0; $try -lt 40; $try++) {
        $candidate = 47840 + (Get-Random -Maximum 200)
        if (-not (Test-LocalPortListening -HostName $HostName -Port $candidate)) {
            Write-Host "Port $PreferredPort in use; smoke API will use $candidate"
            return $candidate
        }
    }
    throw "Could not find a free port for smoke API (preferred $PreferredPort, range 47840-47999)"
}

function Resolve-SmokeApiUrl {
    param([string]$ApiUrl = 'http://127.0.0.1:47821')
    $uri = [Uri]$ApiUrl
    $port = Resolve-SmokeApiPort -PreferredPort $uri.Port -HostName $uri.Host
    if ($port -eq $uri.Port) { return $ApiUrl }
    return "$($uri.Scheme)://$($uri.Host):$port"
}

function Wait-TcpPortOpen {
    param(
        [string]$HostName = '127.0.0.1',
        [int]$Port,
        [int]$TimeoutSec = 0
    )
    if ($TimeoutSec -le 0) {
        $TimeoutSec = if ($env:GITHUB_ACTIONS -eq 'true') { 60 } else { 30 }
    }
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        $client = New-Object System.Net.Sockets.TcpClient
        try {
            $iar = $client.BeginConnect($HostName, $Port, $null, $null)
            if ($iar.AsyncWaitHandle.WaitOne(800) -and $client.Connected) {
                $client.Close()
                return
            }
        } catch { }
        finally {
            if ($client.Connected) { $client.Close() }
            $client.Dispose()
        }
        Start-Sleep -Milliseconds 200
    }
    throw "TCP $HostName`:$Port not accepting connections within ${TimeoutSec}s"
}

function Get-SmokeAccessEvents {
    param(
        [string]$ApiUrl,
        [hashtable]$AuthHeaders,
        [string]$Query = "limit=100"
    )
    $data = Invoke-RestMethod -Uri "$ApiUrl/api/v1/access/events?$Query" -Headers $AuthHeaders
    if ($null -eq $data) { return @() }
    return @($data)
}

function Get-SmokeWebContent {
    param(
        [string]$Uri,
        [hashtable]$Headers
    )
    (Invoke-WebRequest -Uri $Uri -Headers $Headers -UseBasicParsing).Content
}

function Wait-CountIncreased {
    param(
        [int]$Before,
        [scriptblock]$GetCount,
        [string]$Label,
        [int]$TimeoutSec = 15
    )
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    $after = $Before
    while ((Get-Date) -lt $deadline) {
        $after = & $GetCount
        if ($after -gt $Before) { return $after }
        Start-Sleep -Milliseconds (Get-SmokePollDelayMs)
    }
    throw "Expected count to increase ($Label): before=$Before after=$after"
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
