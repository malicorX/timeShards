# Send one credential line to the external hardware TCP ingest (TIMESHARDS_HW_TCP_ADDR).
# Usage: .\scripts\send-hw-tcp.ps1 -CredentialUid DEMO-0002 -ReaderId sim.reader.main

param(
    [string]$TcpHost = "127.0.0.1",
    [int]$Port = 47831,
    [ValidateSet("credential", "door", "reader_offline")]
    [string]$Kind = "credential",
    [string]$ReaderId = "sim.reader.main",
    [string]$CredentialUid = "DEMO-0002",
    [string]$DoorId = "",
    [string]$DoorState = "alarm",
    [ValidateSet("json", "compact")]
    [string]$Format = "json"
)

$ErrorActionPreference = "Stop"

$line = if ($Kind -eq "door") {
    if (-not $DoorId) { throw "DoorId required when Kind=door (GET /api/v1/access/doors)" }
    if ($Format -eq "json") {
        (@{ kind = "door"; door_id = $DoorId; state = $DoorState } | ConvertTo-Json -Compress)
    } else {
        "door;$DoorId;$DoorState"
    }
} elseif ($Kind -eq "reader_offline") {
    if ($Format -eq "json") {
        (@{ kind = "reader_offline"; reader_id = $ReaderId } | ConvertTo-Json -Compress)
    } else {
        "reader_offline;$ReaderId"
    }
} elseif ($Format -eq "json") {
    (@{ reader_id = $ReaderId; credential_uid = $CredentialUid } | ConvertTo-Json -Compress)
} else {
    "$ReaderId;$CredentialUid"
}

$tcp = New-Object System.Net.Sockets.TcpClient
try {
    $tcp.Connect($TcpHost, $Port)
    $stream = $tcp.GetStream()
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($line + "`n")
    $stream.Write($bytes, 0, $bytes.Length)
    $stream.Flush()
    Start-Sleep -Milliseconds 800
    Write-Host "Sent to ${TcpHost}:${Port} ($Format): $line"
} finally {
    $tcp.Close()
}
