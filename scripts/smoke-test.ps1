# Quick API smoke test (server must be running).
# Usage: .\scripts\smoke-test.ps1
#        .\scripts\smoke-test.ps1 -ApiUrl http://192.168.1.10:47821

param(
    [string]$ApiUrl = "http://127.0.0.1:47821",
    [string]$Username = "admin",
    [string]$Password = "admin"
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "_smoke-api.ps1")

Write-Host "Health check..."
$health = Invoke-RestMethod -Uri "$ApiUrl/api/v1/health" -TimeoutSec 5
Write-Host "  $($health.service) v$($health.version) - $($health.status) (db: $($health.database), demo_seeding=$($health.demo_seeding_enabled), hw=$($health.hardware_adapter))"
if ($health.version -notmatch '^0\.2\.') {
    throw "Expected API version 0.2.x, got $($health.version)"
}
if ($health.time_foundation) {
    $tf = $health.time_foundation
    Write-Host "  time_foundation: models=$($tf.workday_models) calendars=$($tf.work_calendars) active_ma=$($tf.active_employees) no_calendar=$($tf.employees_without_work_calendar) kw_no_soll=$($tf.current_week_drafts_without_soll)"
    if ($tf.workday_models -lt 1 -or $tf.work_calendars -lt 1) {
        throw "Expected work calendar foundation tables seeded"
    }
    if ($tf.current_week_drafts_without_soll -gt 0) {
        throw "Expected current_week_drafts_without_soll=0 after API start, got $($tf.current_week_drafts_without_soll)"
    }
}
if ($health.hardware_adapter -ne "sim") {
    throw "Expected hardware_adapter=sim in default smoke, got $($health.hardware_adapter)"
}
if ($health.hardware_tcp_listen) {
    throw "Expected no hardware_tcp_listen with sim adapter, got $($health.hardware_tcp_listen)"
}
if ($health.demo_seeding_enabled -ne $true) {
    throw "Expected demo_seeding_enabled=true in default smoke run"
}
if ($health.default_password_login_blocked -eq $true) {
    throw "Expected default_password_login_blocked=false in default smoke run"
}

Write-Host "OpenAPI spec..."
$spec = Invoke-RestMethod -Uri "$ApiUrl/api/v1/openapi.json" -TimeoutSec 5
if ($spec.openapi -ne "3.0.3") {
    throw "Expected openapi 3.0.3, got $($spec.openapi)"
}
Write-Host "  $($spec.info.title) v$($spec.info.version)"

Write-Host "Login..."
$loginBody = @{ username = $Username; password = $Password } | ConvertTo-Json
$login = Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/auth/login" `
    -Body $loginBody -ContentType "application/json"
$token = $login.token
$headers = @{ Authorization = "Bearer $token" }

Write-Host "  User: $($login.user.display_name) roles=$($login.user.roles -join ',')"

Write-Host "Dashboard..."
$dash = Invoke-RestMethod -Uri "$ApiUrl/api/v1/admin/dashboard" -Headers $headers
Write-Host "  Clocked in: $($dash.clocked_in_employees)/$($dash.employees_total)"
Write-Host "  Pending timesheets: $($dash.pending_timesheets), drafts: $($dash.draft_timesheets), absences: $($dash.pending_absences)"
Write-Host "  Shifts this week: $($dash.shifts_this_week) (planned: $($dash.planned_shifts_this_week))"
if ($null -ne $dash.time_access_mismatch_count) {
    Write-Host "  Time vs access mismatches: $($dash.time_access_mismatch_count)"
}
if ($null -ne $dash.employees_without_work_calendar) {
    Write-Host "  Time foundation: no_calendar=$($dash.employees_without_work_calendar) kw_no_soll=$($dash.timesheets_current_week_no_soll)"
    if ($dash.employees_without_work_calendar -gt 0) {
        throw "Expected all demo employees to have work calendar (got $($dash.employees_without_work_calendar) without)"
    }
}

Write-Host "Policy..."
$policy = Invoke-RestMethod -Uri "$ApiUrl/api/v1/admin/policy" -Headers $headers
Write-Host "  Max daily: $($policy.max_daily_minutes) min, weekly: $($policy.max_weekly_minutes) min"

Write-Host "Clocked-in list..."
$clocked = Invoke-RestMethod -Uri "$ApiUrl/api/v1/time/clocked-in" -Headers $headers
Write-Host "  $($clocked.Count) employee(s) on clock"

Write-Host "Calendar week..."
$calWeek = Invoke-RestMethod -Uri "$ApiUrl/api/v1/time/calendar-week" -Headers $headers
Write-Host "  period_start=$($calWeek.period_start)"
if ($dash.week_start -and $calWeek.period_start -ne $dash.week_start) {
    Write-Host "  WARN: calendar-week period_start ($($calWeek.period_start)) != dashboard week_start ($($dash.week_start))"
}

Write-Host "Work summary..."
$ws = Invoke-RestMethod -Uri "$ApiUrl/api/v1/me/work-summary" -Headers $headers
Write-Host "  clocked_in=$($ws.is_clocked_in) employee_no=$($ws.employee_no) pending_ts=$($ws.pending_timesheets) draft_ts=$($ws.draft_timesheets) team_draft=$($ws.team_draft_timesheets) pending_abs=$($ws.pending_absences)"

Write-Host "Zone occupancy..."
$occ = Invoke-RestMethod -Uri "$ApiUrl/api/v1/access/occupancy" -Headers $headers
Write-Host "  $($occ.Count) zone(s)"

Write-Host "Access rules..."
$rules = Invoke-RestMethod -Uri "$ApiUrl/api/v1/access/rules" -Headers $headers
Write-Host "  $($rules.Count) rule(s)"

Write-Host "Timesheets rebuild..."
$rebuild = Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/time/timesheets/rebuild" -Headers $headers
Write-Host "  updated=$($rebuild.updated) warnings=$($rebuild.warnings.Count)"

Write-Host "Work calendar foundation..."
$models = Invoke-RestMethod -Uri "$ApiUrl/api/v1/time/workday-models" -Headers $headers
if ($models.Count -lt 4) {
    throw "Expected >= 4 workday models (incl. wm-short-6h), got $($models.Count)"
}
$rotPlans = Invoke-RestMethod -Uri "$ApiUrl/api/v1/time/work-rotation-plans" -Headers $headers
if ($rotPlans.Count -lt 1) {
    throw "Expected >= 1 work rotation plan"
}
Write-Host "  rotation_plans=$($rotPlans.Count) (first cycle=$($rotPlans[0].cycle_days)d)"
$cals = Invoke-RestMethod -Uri "$ApiUrl/api/v1/time/work-calendars" -Headers $headers
if ($cals.Count -lt 1) {
    throw "Expected >= 1 work calendar"
}
$calId = $cals[0].id
$year = (Get-Date).Year
$from = "{0}-01-01" -f $year
$to = "{0}-01-07" -f $year
$days = Invoke-RestMethod -Uri "$ApiUrl/api/v1/time/work-calendars/$calId/days?from=$from&to=$to" -Headers $headers
if ($days.Count -lt 5) {
    throw "Expected >= 5 calendar days in first week of year, got $($days.Count)"
}
$assignments = Invoke-RestMethod -Uri "$ApiUrl/api/v1/time/employee-work-assignments" -Headers $headers
if ($assignments.Count -lt 1) {
    throw "Expected >= 1 employee work assignment"
}
Write-Host "  models=$($models.Count) calendars=$($cals.Count) days(sample)=$($days.Count) assignments=$($assignments.Count)"
$stdModel = @($models | Where-Object { $_.id -eq 'wm-std-8h' })[0]
if (-not $stdModel) { throw "Expected wm-std-8h workday model" }
$putBody = @{
    config = @{
        expected_minutes = [int]$stdModel.config.expected_minutes
        flex_band = $stdModel.config.flex_band
        core_time = $stdModel.config.core_time
        break_expectation = $stdModel.config.break_expectation
        auto_credit_expected = $stdModel.config.auto_credit_expected
        label = $stdModel.config.label
    }
} | ConvertTo-Json -Depth 6 -Compress
Invoke-RestMethod -Method Put -Uri "$ApiUrl/api/v1/time/workday-models/wm-std-8h" -Headers $headers `
    -Body $putBody -ContentType "application/json" | Out-Null
Write-Host "  workday-model PUT (wm-std-8h) OK"
Write-Host "Foundation fix (assign + rebuild KW)..."
$fix = Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/admin/foundation-fix" -Headers $headers
Write-Host "  foundation-fix: calendars=$($fix.calendars_assigned) timesheets=$($fix.timesheets_updated)"
$dashAfterFix = Invoke-RestMethod -Uri "$ApiUrl/api/v1/admin/dashboard" -Headers $headers
if ($dashAfterFix.timesheets_current_week_no_soll -gt 0) {
    throw "Expected timesheets_current_week_no_soll=0 after foundation-fix, got $($dashAfterFix.timesheets_current_week_no_soll)"
}
$tsList = @(Invoke-RestMethod -Uri "$ApiUrl/api/v1/time/timesheets" -Headers $headers)
$withSoll = @($tsList | Where-Object { $_.expected_minutes -gt 0 })
if ($withSoll.Count -lt 1) {
    throw "Expected at least one timesheet with expected_minutes after rebuild (got $($tsList.Count) total)"
}
Write-Host "  timesheets with Soll: $($withSoll.Count)/$($tsList.Count)"

Write-Host "Timesheet HTML export (Tagesdetails)..."
$htmlUri = "$ApiUrl/api/v1/reports/timesheets/export?format=html&status=draft"
$htmlResp = Invoke-WebRequest -Uri $htmlUri -Headers $headers -UseBasicParsing
if ($htmlResp.StatusCode -ne 200) {
    throw "Timesheet HTML export failed: $($htmlResp.StatusCode)"
}
if ($htmlResp.Content -notmatch 'Tagesdetails') {
    throw "Expected Tagesdetails section in HTML export after rebuild"
}
Write-Host "  HTML export OK ($($htmlResp.Content.Length) bytes)"

Write-Host "Time accounts (list)..."
$adminAccounts = Invoke-RestMethod -Uri "$ApiUrl/api/v1/time/accounts" -Headers $headers
Write-Host "  admin account rows=$($adminAccounts.Count)"

Write-Host "Monthly settlement preview..."
$adminEmp = @(Invoke-RestMethod -Uri "$ApiUrl/api/v1/admin/employees" -Headers $headers) | Select-Object -First 1
if ($adminEmp) {
    $y = (Get-Date).Year
    $m = (Get-Date).Month
    $preview = Invoke-RestMethod -Uri "$ApiUrl/api/v1/time/settlement-periods/preview?year=$y&month=$m&employee_id=$($adminEmp.id)" -Headers $headers
    Write-Host "  preview approved_weeks=$($preview.approved_weeks) pending=$($preview.pending_weeks)"
}

Write-Host "Demo employee login..."
$demoBody = @{ username = "demo"; password = "demo" } | ConvertTo-Json
$demoLogin = Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/auth/login" `
    -Body $demoBody -ContentType "application/json"
$demoHeaders = @{ Authorization = "Bearer $($demoLogin.token)" }
$demoWs = Invoke-RestMethod -Uri "$ApiUrl/api/v1/me/work-summary" -Headers $demoHeaders
Write-Host "  user=$($demoLogin.user.display_name) employee_no=$($demoLogin.user.employee_no) draft_ts=$($demoWs.draft_timesheets) my_pending_abs=$($demoWs.my_pending_absences) calendar=$($demoWs.work_calendar_assigned)"
if ($demoWs.work_calendar_assigned -ne $true) {
    throw "Expected demo work_calendar_assigned=true"
}
if ($demoWs.current_week -and $demoWs.current_week.expected_minutes -lt 1) {
    throw "Expected demo current_week.expected_minutes after lazy rebuild"
}
if ($demoWs.draft_timesheets -lt 1) {
    throw "Expected demo draft_timesheets >= 1"
}
if ($demoWs.my_pending_absences -lt 1) {
    throw "Expected demo my_pending_absences >= 1"
}
$meAccess = (Invoke-WebRequest -Uri "$ApiUrl/api/v1/access/me" -Headers $demoHeaders).Content | ConvertFrom-Json
$readerCount = @($meAccess.readers).Count
if ($readerCount -lt 2) {
    throw "Expected >= 2 readers on /access/me for simulate-scan, got $readerCount"
}
Write-Host "  access/me readers=$readerCount"
$ev0002 = @(
    (Invoke-WebRequest -Uri "$ApiUrl/api/v1/access/events?employee_no=0002&limit=5" -Headers $headers).Content | ConvertFrom-Json
)
if ($ev0002.Count -lt 1) {
    throw "Expected access events for employee_no=0002 filter"
}
Write-Host "  access/events employee_no filter OK"
$mgrLogin = Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/auth/login" `
    -Body (@{ username = "manager"; password = "demo" } | ConvertTo-Json) -ContentType "application/json"
$mgrWs = Invoke-RestMethod -Uri "$ApiUrl/api/v1/me/work-summary" -Headers @{ Authorization = "Bearer $($mgrLogin.token)" }
Write-Host "  manager pending_ts=$($mgrWs.pending_timesheets) pending_abs=$($mgrWs.pending_absences)"
if ($mgrWs.pending_timesheets -lt 1) {
    throw "Expected manager pending_timesheets >= 1"
}
if ($mgrWs.pending_absences -lt 1) {
    throw "Expected manager pending_absences >= 1"
}

Write-Host "Time accounts (approve posts flex)..."
$pendingTs = @(Invoke-RestMethod -Uri "$ApiUrl/api/v1/time/timesheets?status=pending" -Headers $headers)
if ($pendingTs.Count -ge 1) {
    $tsId = $pendingTs[0].id
    Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/time/timesheets/$tsId/approve" -Headers $headers | Out-Null
    $adminAccountsAfter = Invoke-RestMethod -Uri "$ApiUrl/api/v1/time/accounts" -Headers $headers
    $flex = @($adminAccountsAfter | Where-Object { $_.account_kind -eq 'flex' })
    if ($flex.Count -lt 1) {
        throw "Expected flex time account after timesheet approve"
    }
    Write-Host "  flex balance after approve=$($flex[0].balance_minutes) min"
} else {
    throw "Expected pending timesheet to test account posting on approve"
}

Write-Host "Payroll CSV export..."
$payYear = (Get-Date).Year
$payMonth = (Get-Date).Month
$payrollTmp = Join-Path $env:TEMP "timeshards-smoke-payroll.csv"
Invoke-WebRequest -Uri "$ApiUrl/api/v1/reports/payroll/export?year=$payYear&month=$payMonth&format=csv&aggregate=employee" `
    -Headers $headers -OutFile $payrollTmp | Out-Null
$payrollBytes = [System.IO.File]::ReadAllBytes($payrollTmp)
if ($payrollBytes.Length -lt 3 -or $payrollBytes[0] -ne 0xEF -or $payrollBytes[1] -ne 0xBB -or $payrollBytes[2] -ne 0xBF) {
    throw "Payroll CSV missing UTF-8 BOM (Excel-friendly export)"
}
$payrollText = [System.IO.File]::ReadAllText($payrollTmp)
if ($payrollText -notmatch 'personal_nr;name') {
    throw "Payroll CSV missing expected header"
}
Remove-Item -Force $payrollTmp -ErrorAction SilentlyContinue
Write-Host "  payroll CSV OK ($($payrollBytes.Length) bytes, UTF-8 BOM)"

Write-Host "Absences payroll CSV export..."
$emps = Invoke-RestMethod -Uri "$ApiUrl/api/v1/admin/employees" -Headers $headers
if ($emps.Count -lt 1) {
    throw "Need at least one employee for absences export smoke"
}
$smokeEmpId = $emps[0].id
$absDay = [Math]::Min(15, [DateTime]::DaysInMonth($payYear, $payMonth))
$absStart = "{0:0000}-{1:00}-{2:00}T08:00:00+02:00" -f $payYear, $payMonth, $absDay
$absEnd = "{0:0000}-{1:00}-{2:00}T17:00:00+02:00" -f $payYear, $payMonth, ($absDay + 1)
$absCreateBody = @{
    employee_id  = $smokeEmpId
    absence_type = 'urlaub'
    starts_at    = $absStart
    ends_at      = $absEnd
    reason       = 'smoke payroll export'
} | ConvertTo-Json
$smokeAbs = Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/absences" -Headers $headers `
    -Body $absCreateBody -ContentType 'application/json'
Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/absences/$($smokeAbs.id)/approve" -Headers $headers `
    -Body '{}' -ContentType 'application/json' | Out-Null
Write-Host "  created + approved in-month absence $($smokeAbs.id)"
$absTmp = Join-Path $env:TEMP "timeshards-smoke-absences.csv"
Invoke-WebRequest -Uri "$ApiUrl/api/v1/reports/absences/export?year=$payYear&month=$payMonth&format=csv" `
    -Headers $headers -OutFile $absTmp | Out-Null
$absText = [System.IO.File]::ReadAllText($absTmp)
if ($absText -notmatch 'personal_nr;name;jahr;monat;typ') {
    throw "Absences export CSV missing expected header"
}
if ($absText -match '\(keine freigegebenen Abwesenheiten') {
    throw "Expected approved absence row in payroll absences export"
}
Remove-Item -Force $absTmp -ErrorAction SilentlyContinue
Write-Host "  absences export CSV OK (in-month row)"

$demoTemplates = Invoke-RestMethod -Uri "$ApiUrl/api/v1/time/shift-templates" -Headers $demoHeaders
Write-Host "  shift_templates=$($demoTemplates.Count) (scoped to own employee)"
$demoShifts = Invoke-RestMethod -Uri "$ApiUrl/api/v1/time/shifts" -Headers $demoHeaders
Write-Host "  shifts_visible_to_demo=$($demoShifts.Count)"
$demoEvents = Invoke-RestMethod -Uri "$ApiUrl/api/v1/time/events?limit=5" -Headers $demoHeaders
Write-Host "  time_events=$($demoEvents.Count)"

Write-Host "Demo clock-in / clock-out..."
Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/time/clock-in" -Headers $demoHeaders | Out-Null
$wsIn = Invoke-RestMethod -Uri "$ApiUrl/api/v1/me/work-summary" -Headers $demoHeaders
if (-not $wsIn.is_clocked_in) {
    throw "Expected demo is_clocked_in=true after clock-in"
}
$clocked = Invoke-RestMethod -Uri "$ApiUrl/api/v1/time/clocked-in" -Headers $headers
$demoOnClock = @($clocked | Where-Object { $_.employee_no -eq "0002" }).Count
if ($demoOnClock -lt 1) {
    throw "Expected demo on clocked-in list after clock-in"
}
Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/time/clock-out" -Headers $demoHeaders | Out-Null
$wsOut = Invoke-RestMethod -Uri "$ApiUrl/api/v1/me/work-summary" -Headers $demoHeaders
if ($wsOut.is_clocked_in) {
    throw "Expected demo is_clocked_in=false after clock-out"
}
if (-not $wsOut.current_week) {
    throw "Expected demo work-summary.current_week after clock-out (auto-rebuild)"
}
if ($wsOut.current_week.expected_minutes -lt 1) {
    throw "Expected demo current_week.expected_minutes > 0, got $($wsOut.current_week.expected_minutes)"
}
if ($wsOut.current_week.worked_minutes -lt 1) {
    throw "Expected demo current_week.worked_minutes > 0 after punch pair, got $($wsOut.current_week.worked_minutes)"
}
Write-Host "  clock-in/out OK (KW Saldo $($wsOut.current_week.balance_minutes) min, Soll $($wsOut.current_week.expected_minutes))"

Write-Host "Demo break start / end..."
Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/time/clock-in" -Headers $demoHeaders | Out-Null
Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/time/break-start" -Headers $demoHeaders | Out-Null
$wsBreak = Invoke-RestMethod -Uri "$ApiUrl/api/v1/me/work-summary" -Headers $demoHeaders
if (-not $wsBreak.is_on_break) {
    throw "Expected demo is_on_break=true after break-start"
}
Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/time/break-end" -Headers $demoHeaders | Out-Null
Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/time/clock-out" -Headers $demoHeaders | Out-Null
Write-Host "  break flow OK"

Write-Host "Admin time events (all staff)..."
$adminEvents = @(Invoke-RestMethod -Uri "$ApiUrl/api/v1/time/events?limit=100" -Headers $headers)
$demoOnlyEvents = @(Invoke-RestMethod -Uri "$ApiUrl/api/v1/time/events?limit=100" -Headers $demoHeaders)
if ($adminEvents.Count -lt $demoOnlyEvents.Count) {
    throw "Admin should see at least as many time events as demo ($($demoOnlyEvents.Count)), got $($adminEvents.Count)"
}
if ($adminEvents.Count -gt 0 -and -not $adminEvents[0].employee_no) {
    throw "Expected employee_no on admin time/events list"
}
Write-Host "  admin events=$($adminEvents.Count) demo events=$($demoOnlyEvents.Count)"

Write-Host "Demo timesheet rebuild (scoped to self)..."
$demoRebuild = Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/time/timesheets/rebuild" -Headers $demoHeaders
if ($demoRebuild.updated -gt 1) {
    throw "Expected demo rebuild to update at most 1 timesheet, got $($demoRebuild.updated)"
}
Write-Host "  demo rebuild updated=$($demoRebuild.updated)"

Write-Host "Demo timesheet export (scoped to self)..."
$demoCsv = (Invoke-WebRequest -Uri "$ApiUrl/api/v1/reports/timesheets/export?format=csv&status=draft" -Headers $demoHeaders).Content
if ($demoCsv -match '(?m)^0001;' -or $demoCsv -match '(?m)^0003;') {
    throw "Demo export must not include other employees (0001/0003)"
}
if ($demoCsv -notmatch '(?m)^0002;') {
    throw "Demo export should include own employee 0002"
}
Write-Host "  draft export scoped OK"

Write-Host "Demo access export (forbidden)..."
try {
    Invoke-WebRequest -Uri "$ApiUrl/api/v1/reports/access/export?format=csv" -Headers $demoHeaders | Out-Null
    throw "Expected demo access export to be forbidden"
} catch {
    $status = $null
    if ($_.Exception.Response) { $status = [int]$_.Exception.Response.StatusCode }
    if ($status -ne 403) {
        throw "Expected HTTP 403 for demo access export, got $status ($($_.Exception.Message))"
    }
}
Write-Host "  demo cannot export access log (403)"

Write-Host "Demo badge simulate-scan (in)..."
function Get-AccessEvents {
    param([hashtable]$AuthHeaders)
    $content = (Invoke-WebRequest -Uri "$ApiUrl/api/v1/access/events?limit=100" -Headers $AuthHeaders).Content
    if ([string]::IsNullOrWhiteSpace($content) -or $content.Trim() -eq '[]') {
        return @()
    }
    $data = $content | ConvertFrom-Json
    if ($null -eq $data) { return @() }
    return @($data)
}

function Get-DemoAccessEventCount {
    param([hashtable]$AuthHeaders)
    $events = Get-AccessEvents -AuthHeaders $AuthHeaders
    return @($events | Where-Object { $_.employee_no -eq "0002" }).Count
}
$eventsBefore = Get-DemoAccessEventCount -AuthHeaders $headers
$scanBody = @{ reader_id = "sim.reader.main"; credential_uid = "DEMO-0002" } | ConvertTo-Json
$scan = Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/access/me/simulate-scan" `
    -Headers $demoHeaders -Body $scanBody -ContentType "application/json"
Write-Host "  decision=$($scan.decision) reason=$($scan.reason_code)"
if ($scan.decision -notin @("grant", "allow")) {
    throw "Expected demo simulate-scan decision grant, got $($scan.decision)"
}
$eventsAfter = Wait-CountIncreased -Before $eventsBefore -GetCount { Get-DemoAccessEventCount -AuthHeaders $headers } -Label 'demo simulate-scan'
if ($eventsAfter -gt $eventsBefore + 1) {
    Write-Host "  note: $($eventsAfter - $eventsBefore) new demo events (expected >= 1)"
}

Write-Host "Building occupancy (after entry)..."
$dashAfterIn = Invoke-RestMethod -Uri "$ApiUrl/api/v1/admin/dashboard" -Headers $headers
if ($dashAfterIn.people_in_building -lt 1) {
    throw "Expected people_in_building >= 1 after entry, got $($dashAfterIn.people_in_building)"
}
Write-Host "  people_in_building=$($dashAfterIn.people_in_building)"

Write-Host "Demo badge simulate-scan (in again, anti-passback)..."
$scan2 = Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/access/me/simulate-scan" `
    -Headers $demoHeaders -Body $scanBody -ContentType "application/json"
Write-Host "  decision=$($scan2.decision) reason=$($scan2.reason_code)"
if ($scan2.decision -ne "deny" -or $scan2.reason_code -ne "antipassback") {
    throw "Expected antipassback on duplicate entry, got $($scan2.decision)/$($scan2.reason_code)"
}

Write-Host "Demo badge simulate-scan (out)..."
$scanOutBody = @{ reader_id = "sim.reader.main.out"; credential_uid = "DEMO-0002" } | ConvertTo-Json
$scanOut = Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/access/me/simulate-scan" `
    -Headers $demoHeaders -Body $scanOutBody -ContentType "application/json"
Write-Host "  decision=$($scanOut.decision) reason=$($scanOut.reason_code)"
if ($scanOut.decision -notin @("grant", "allow")) {
    throw "Expected demo out-scan grant, got $($scanOut.decision)"
}

Write-Host "Building occupancy (after exit)..."
$dashAfterOut = Invoke-RestMethod -Uri "$ApiUrl/api/v1/admin/dashboard" -Headers $headers
if ($dashAfterOut.people_in_building -ne 0) {
    throw "Expected people_in_building = 0 after exit, got $($dashAfterOut.people_in_building)"
}
Write-Host "  people_in_building=0"

$mgrHeaders = @{ Authorization = "Bearer $($mgrLogin.token)" }
Write-Host "Manager badge simulate-scan..."
$mgrScan = Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/access/me/simulate-scan" `
    -Headers $mgrHeaders -Body (@{ reader_id = "sim.reader.main"; credential_uid = "DEMO-0003" } | ConvertTo-Json) `
    -ContentType "application/json"
Write-Host "  decision=$($mgrScan.decision) reason=$($mgrScan.reason_code)"
if ($mgrScan.decision -notin @("grant", "allow")) {
    throw "Expected manager simulate-scan grant, got $($mgrScan.decision)"
}
$mgrAccessCsv = (Invoke-WebRequest -Uri "$ApiUrl/api/v1/reports/access/export?format=csv&limit=50" -Headers $mgrHeaders).Content
if ($mgrAccessCsv -notmatch '(?m)^0002;' -and $mgrAccessCsv -notmatch '(?m);0002;') {
    throw "Manager access export should include demo employee 0002 after scans"
}
Write-Host "  manager access export includes demo scans"

$teamDrafts = Invoke-RestMethod -Uri "$ApiUrl/api/v1/time/timesheets?status=draft" -Headers $mgrHeaders
Write-Host "  manager sees $($teamDrafts.Count) draft timesheet(s) (team queue)"
if ($teamDrafts.Count -lt 1) {
    throw "Expected manager to see at least 1 draft timesheet"
}

Write-Host "Admin hardware-present (worker channel)..."
$hwSince = (Get-Date).ToUniversalTime().ToString("o")
$hwBody = @{ reader_id = "sim.reader.main"; credential_uid = "DEMO-0002" } | ConvertTo-Json
$hwQueued = Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/access/hardware-present" `
    -Headers $headers -Body $hwBody -ContentType "application/json"
if (-not $hwQueued.queued) {
    throw "Expected hardware-present queued=true"
}
$hwLatest = $null
$sinceQs = [uri]::EscapeDataString($hwSince)
$hwPollMax = if ($env:GITHUB_ACTIONS -eq 'true') { 50 } else { 25 }
for ($i = 0; $i -lt $hwPollMax; $i++) {
    $sinceEv = @(
        (Invoke-WebRequest -Uri "$ApiUrl/api/v1/access/events?limit=5&since=$sinceQs" -Headers $headers).Content | ConvertFrom-Json
    )
    $hwLatest = $sinceEv | Where-Object { $_.employee_no -eq "0002" } | Select-Object -First 1
    if ($hwLatest) { break }
    Start-Sleep -Milliseconds (Get-SmokePollDelayMs)
}
if (-not $hwLatest) {
    throw "Expected demo access event after hardware-present (since poll)"
}
if ($hwLatest.decision -notin @("grant", "allow")) {
    throw "Expected grant on hardware-present path, got $($hwLatest.decision) ($($hwLatest.reason_code))"
}
Write-Host "  hardware-present OK ($($hwLatest.decision), since poll)"

Write-Host "Timesheets submit-drafts (admin bulk)..."
$submit = Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/v1/time/timesheets/submit-drafts" -Headers $headers
Write-Host "  submitted=$($submit.submitted)"

Write-Host ""
Write-Host "Smoke test OK."
