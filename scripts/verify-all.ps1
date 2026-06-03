# Full local verification: typecheck + time foundation tests + API smoke.
# Usage: .\scripts\verify-all.ps1

$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $RepoRoot

& (Join-Path $PSScriptRoot "check.ps1")
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

& (Join-Path $PSScriptRoot "verify-foundation.ps1")
exit $LASTEXITCODE
