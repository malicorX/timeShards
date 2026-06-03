# Quick Zeitbasis check against a running API (no auth).
# Usage: .\scripts\foundation-health.ps1
#        $env:TIMESHARDS_API_URL = "http://127.0.0.1:47821"

$ErrorActionPreference = "Stop"
$ApiUrl = if ($env:TIMESHARDS_API_URL) { $env:TIMESHARDS_API_URL } else { "http://127.0.0.1:47821" }

$h = Invoke-RestMethod -Uri "$ApiUrl/api/v1/health"
if ($h.database -ne "ok") {
    Write-Error "API database not ok: $($h.database)"
}
if (-not $h.time_foundation) {
    Write-Error "time_foundation missing on health response"
}

$tf = $h.time_foundation
Write-Host "Zeitbasis:" -ForegroundColor Cyan
Write-Host "  models=$($tf.workday_models) calendars=$($tf.work_calendars) active_ma=$($tf.active_employees)"
Write-Host "  ohne_kalender=$($tf.employees_without_work_calendar) kw_ohne_soll=$($tf.current_week_drafts_without_soll)"

$bad = $false
if ($tf.employees_without_work_calendar -gt 0) {
    Write-Host "FAIL: active employees without work calendar" -ForegroundColor Red
    $bad = $true
}
if ($tf.current_week_drafts_without_soll -gt 0) {
    Write-Host "FAIL: current week drafts missing Soll (run foundation-fix or restart API)" -ForegroundColor Red
    $bad = $true
}
if ($bad) { exit 1 }

Write-Host "Zeitbasis OK." -ForegroundColor Green
