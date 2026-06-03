# Pre-go-live: run the full pilot gate, then print cutover reminders.
# Usage: .\scripts\pilot-ready.ps1

$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $RepoRoot

Write-Host "=== Pilot gate (verify:pilot) ===" -ForegroundColor Cyan
& (Join-Path $PSScriptRoot "verify-pilot.ps1")
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host ""
Write-Host "=== Go-live checklist (see docs/PILOT.md) ===" -ForegroundColor Cyan
Write-Host "  1. Set TIMESHARDS_DISABLE_DEMO=1 and TIMESHARDS_ADMIN_PASSWORD on the server"
Write-Host "  2. Start API or Server app; complete Go-Live-Assistent on Übersicht"
Write-Host "  3. Personal + Perioden + Zutritt master data; run Zeitbasis reparieren if KPIs warn"
Write-Host "  4. Trial week: client clocking, manager approvals, Zeit-Zutritt KPI = 0"
Write-Host "  5. Month-end: Monatsabschluss + Lohn-/Abwesenheiten-CSV"
Write-Host "  6. Optional hardware: TIMESHARDS_HW_ADAPTER=external + npm run verify:doors"
Write-Host ""
Write-Host "Pilot gate passed. Ship when checklist above is done on the target machine." -ForegroundColor Green
