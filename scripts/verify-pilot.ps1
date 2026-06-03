# Pilot go-live gate: typecheck, foundation tests, API smoke, production-mode smoke.
# Usage: .\scripts\verify-pilot.ps1

$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $RepoRoot

& (Join-Path $PSScriptRoot "verify-all.ps1")
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "`n=== production smoke ===" -ForegroundColor Cyan
& (Join-Path $PSScriptRoot "smoke-production.ps1")
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "`nPilot verify OK (demo off + default passwords blocked)." -ForegroundColor Green
