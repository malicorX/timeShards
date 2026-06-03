# Verify work-calendar foundation: unit tests + headless API smoke.
# Usage: .\scripts\verify-foundation.ps1

$ErrorActionPreference = "Stop"
if (Get-Variable -Name PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
    $PSNativeCommandUseErrorActionPreference = $false
}
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

Write-Host "=== timeshards-db tests ===" -ForegroundColor Cyan
cargo test -p timeshards-db
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "`n=== API smoke ===" -ForegroundColor Cyan
npm run smoke:api
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "`nFoundation verify OK." -ForegroundColor Green
