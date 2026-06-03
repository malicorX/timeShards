# Opens Windows Firewall for TimeShards API (default port 47821).
# Run in an elevated PowerShell: Right-click → "Run as administrator"

param(
    [int]$Port = 47821,
    [string]$RuleName = "TimeShards API"
)

$existing = Get-NetFirewallRule -DisplayName $RuleName -ErrorAction SilentlyContinue
if ($existing) {
    Write-Host "Rule '$RuleName' already exists."
    exit 0
}

New-NetFirewallRule -DisplayName $RuleName `
    -Direction Inbound `
    -Action Allow `
    -Protocol TCP `
    -LocalPort $Port `
    -Profile Domain, Private

Write-Host "Firewall rule '$RuleName' created for TCP port $Port (Domain + Private profiles)."
Write-Host "Clients on the LAN can reach http://<this-pc-ip>:$Port"
