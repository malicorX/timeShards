# Release / pilot ship gate (alias for verify:pilot).
# Usage: .\scripts\ship.ps1

$ErrorActionPreference = "Stop"
& (Join-Path $PSScriptRoot "verify-pilot.ps1")
exit $LASTEXITCODE
