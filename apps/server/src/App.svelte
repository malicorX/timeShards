<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { api, downloadFile, openHtmlExport, setToken, getToken, type LoginResponse } from './lib/api';
  import {
    anchorFromPeriodStart,
    toLocalDatetimeInputValue,
    fromLocalDatetimeInputValue,
    isoToLocalDatetimeInput,
    formatIsoLocalShort,
  } from './lib/datetime';
  import { statusLabel } from './lib/statusLabels';
  import WorkCalendarCard from './components/WorkCalendarCard.svelte';
  import ShiftWeekCard from './components/ShiftWeekCard.svelte';
  import TimesheetsCard from './components/TimesheetsCard.svelte';

  type Tab = 'overview' | 'personnel' | 'time' | 'absence' | 'access' | 'system';

  function handleUiMessage(msg: { type: 'error' | 'success'; text: string }) {
    if (msg.type === 'error') error = msg.text;
    else success = msg.text;
  }
  let tab = $state<Tab>('overview');
  let apiUrl = $state('http://127.0.0.1:47821');
  let apiUrls = $state<string[]>([]);
  let apiBind = $state('');
  let dbPath = $state('');
  let apiHealth = $state<{
    status: string;
    version: string;
    database: string;
    service: string;
    demo_seeding_enabled?: boolean;
    default_password_login_blocked?: boolean;
    hardware_adapter?: string;
    hardware_adapter_configured?: string | null;
    hardware_tcp_listen?: string | null;
  } | null>(null);
  let username = $state('admin');
  let password = $state('admin');
  let user = $state<LoginResponse['user'] | null>(null);
  let error = $state('');
  let success = $state('');

  let zones = $state<{ id: string; name: string; site_id: string }[]>([]);
  let doors = $state<
    {
      id: string;
      name: string;
      status: string;
      zone_id: string | null;
      reader_in_id?: string | null;
      reader_out_id?: string | null;
    }[]
  >([]);
  let badges = $state<
    {
      id: string;
      credential_uid: string;
      employee_id: string | null;
      employee_no?: string | null;
      employee_name?: string | null;
      status: string;
    }[]
  >([]);
  let accessExportFrom = $state('');
  let accessExportTo = $state('');
  let accessEvents = $state<
    {
      id: string;
      occurred_at: string;
      decision: string;
      reason_code?: string;
      employee_no?: string;
      employee_name?: string;
      zone_name?: string;
      door_name?: string;
    }[]
  >([]);
  let simulateUid = $state('DEMO-ADMIN-001');

  let users = $state<
    { id: string; username: string; display_name: string; roles: string[]; status: string }[]
  >([]);
  let includeInactiveUsers = $state(false);
  let resetPasswordUserId = $state('');
  let resetPasswordValue = $state('');
  let personnelSearch = $state('');
  let personnelShowSetupOpen = $state(false);
  let personnelShowNoCalendar = $state(false);
  let editingEmployeeId = $state('');
  let editDisplayName = $state('');
  let editOrgUnit = $state('');
  let absenceFilter = $state<'all' | 'pending' | 'approved' | 'rejected'>('all');
  let absenceEmployeeFilter = $state('');
  let auditLimit = $state(100);
  let clockedIn = $state<
    {
      employee_no: string;
      display_name: string;
      last_kind: string;
      last_at: string;
      is_on_break: boolean;
    }[]
  >([]);
  let employees = $state<
    {
      id: string;
      employee_no: string;
      display_name: string;
      user_id: string | null;
      org_unit?: string | null;
      username?: string | null;
      active?: boolean;
      active_to?: string | null;
      work_calendar_assigned?: boolean;
    }[]
  >([]);
  let includeInactiveEmployees = $state(false);
  let auditObjectFilter = $state('');
  let auditActionFilter = $state('');
  let auditActorFilter = $state('');
  let roles = $state<{ id: string; name: string }[]>([]);
  let shiftWeekAnchor = $state(new Date());
  let timesheetFilter = $state<'all' | 'pending' | 'draft' | 'approved' | 'rejected'>('all');
  let shiftFilter = $state<'all' | 'planned' | 'published' | 'cancelled'>('all');
  let workCalendarCard: WorkCalendarCard | undefined = $state();
  let shiftWeekCard: ShiftWeekCard | undefined = $state();
  let timesheetsCard: TimesheetsCard | undefined = $state();
  let dashboard = $state<{
    pending_timesheets: number;
    draft_timesheets: number;
    pending_absences: number;
    clocked_in_employees: number;
    employees_total: number;
    shifts_this_week: number;
    planned_shifts_this_week: number;
    doors_alarm: number;
    doors_forced_open: number;
    doors_open: number;
    people_in_building: number;
    door_alerts: { id: string; name: string; status: string }[];
    demo_seeding_enabled?: boolean;
    default_password_login_blocked?: boolean;
    hardware_adapter?: string;
    hardware_adapter_configured?: string | null;
    hardware_tcp_listen?: string | null;
    employees_without_work_calendar?: number;
    timesheets_current_week_no_soll?: number;
  } | null>(null);

  const doorReaders = $derived.by(() => {
    const out: { id: string; label: string }[] = [];
    for (const d of doors) {
      if (d.reader_in_id) {
        out.push({ id: d.reader_in_id, label: `${d.name} — Eingang` });
      }
      if (d.reader_out_id) {
        out.push({ id: d.reader_out_id, label: `${d.name} — Ausgang` });
      }
    }
    return out;
  });
  let newUser = $state({
    username: '',
    password: '',
    display_name: '',
    role_name: 'employee',
  });
  let newEmployee = $state({
    display_name: '',
    employee_no: '',
    org_unit: '',
    issue_badge: true,
    grant_zone_access: true,
    grant_work_calendar: true,
  });
  let linkUserByEmployee = $state<Record<string, string>>({});
  let accessEventFilter = $state<'all' | 'grant' | 'deny'>('all');
  let accessEventEmployeeNo = $state('');
  let currentPassword = $state('');
  let newPassword = $state('');

  let newZone = $state({ name: '' });
  let newDoor = $state({ name: '', zone_id: '', reader_id: 'sim.reader.main' });
  let newBadge = $state({ employee_id: '', credential_uid: '' });
  let accessRules = $state<
    {
      id: string;
      principal_id: string;
      zone_id?: string | null;
      employee_name?: string;
      zone_name?: string;
      mode: string;
      valid_from: string;
      valid_to?: string | null;
      schedule_json?: string | null;
    }[]
  >([]);
  let newAccessRule = $state({
    employee_id: '',
    zone_id: '',
    use_schedule: true,
    start: '08:00',
    end: '18:00',
    valid_from_local: '',
    valid_to_local: '',
  });
  let accessRuleEmployeeFilter = $state('');
  let accessRuleZoneFilter = $state('');
  let zoneOccupancy = $state<
    { zone_name: string; inside_count: number; occupants: { display_name: string }[] }[]
  >([]);
  let auditLog = $state<
    {
      actor_type: string;
      action: string;
      object_type: string;
      object_id?: string | null;
      occurred_at: string;
      reason?: string;
    }[]
  >([]);
  let sites = $state<{ id: string; name: string; timezone: string }[]>([]);
  let simulateEmployeeId = $state('');
  let simulateReader = $state('sim.reader.main');
  let rejectReason = $state('');
  let absences = $state<
    {
      id: string;
      employee_name: string;
      employee_no: string;
      absence_type: string;
      starts_at: string;
      ends_at: string;
      status: string;
      reason?: string;
    }[]
  >([]);
  let newAbsence = $state({
    employee_id: '',
    absence_type: 'urlaub',
    starts_local: toLocalDatetimeInputValue(new Date()),
    ends_local: toLocalDatetimeInputValue(
      new Date(Date.now() + 7 * 24 * 60 * 60 * 1000),
    ),
    reason: '',
  });
  let absenceConflict = $state<string | null>(null);

  const canApprove = $derived(
    user?.roles.some((r) =>
      ['system_admin', 'hr_admin', 'manager'].includes(r),
    ) ?? false,
  );

  const canCorrectTime = $derived(
    user?.roles.some((r) => ['system_admin', 'hr_admin', 'manager'].includes(r)) ?? false,
  );

  const displayedEmployees = $derived(
    employees.filter((e) => {
      if (personnelShowSetupOpen && e.active !== false) {
        if (employeeHasActiveBadge(e.id) && employeeHasZoneAllow(e.id)) return false;
      }
      if (personnelShowNoCalendar && e.active !== false && e.work_calendar_assigned !== false) {
        return false;
      }
      return true;
    }),
  );

  const displayedAccessRules = $derived(
    accessRules.filter((r) => {
      if (accessRuleEmployeeFilter && r.principal_id !== accessRuleEmployeeFilter) {
        return false;
      }
      if (accessRuleZoneFilter && r.zone_id !== accessRuleZoneFilter) {
        return false;
      }
      return true;
    }),
  );

  $effect(() => {
    loadServerInfo();
  });

  $effect(() => {
    if (tab !== 'absence' || !user) return;
    newAbsence.employee_id;
    newAbsence.starts_local;
    newAbsence.ends_local;
    void checkAbsenceConflict();
  });

  $effect(() => {
    if (!user) return;
    const t = tab;
    error = '';
    if (t === 'overview') {
      void refreshDashboard();
      void refreshClockedIn();
    } else if (t === 'personnel') {
      void refreshPersonnel();
      void refreshAccess();
    }
    else if (t === 'time') void refreshTime();
    else if (t === 'absence') void refreshAbsences();
    else if (t === 'access') {
      void refreshAccess();
      void refreshDashboard();
    }
    else if (t === 'system') void refreshSystem();
  });

  function defaultAccessExportRange() {
    const to = new Date();
    const from = new Date(to.getTime() - 7 * 24 * 60 * 60 * 1000);
    accessExportFrom = toLocalDatetimeInputValue(from);
    accessExportTo = toLocalDatetimeInputValue(to);
  }

  async function loadServerInfo() {
    try {
      const info = await invoke<{
        api_bind: string;
        api_urls: string[];
        database_path: string;
      }>('get_server_info');
      apiBind = info.api_bind;
      apiUrls = info.api_urls;
      apiUrl = info.api_urls[0] ?? apiUrl;
      dbPath = info.database_path;
    } catch {
      /* backend still starting */
    }
    if (!accessExportFrom) defaultAccessExportRange();
    await refreshHealth();
  }

  async function refreshHealth() {
    apiHealth = await api<{
      status: string;
      version: string;
      database: string;
      service: string;
      demo_seeding_enabled?: boolean;
      default_password_login_blocked?: boolean;
      hardware_adapter?: string;
      hardware_adapter_configured?: string | null;
      hardware_tcp_listen?: string | null;
      time_foundation?: {
        workday_models: number;
        work_calendars: number;
        active_employees: number;
        employees_without_work_calendar: number;
        current_week_drafts_without_soll: number;
      };
    }>(apiUrl, '/api/v1/health').catch(() => null);
  }

  async function login() {
    error = '';
    success = '';
    try {
      const res = await api<LoginResponse>(apiUrl, '/api/v1/auth/login', {
        method: 'POST',
        body: JSON.stringify({ username, password }),
      });
      setToken(res.token);
      user = res.user;
      await refreshAll();
      await refreshDashboard();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function syncShiftWeekFromApi() {
    const w = await api<{ period_start: string }>(apiUrl, '/api/v1/time/calendar-week').catch(
      () => null,
    );
    const anchor = w?.period_start ? anchorFromPeriodStart(w.period_start) : null;
    if (anchor) shiftWeekAnchor = anchor;
  }

  async function refreshAll() {
    if (!getToken()) return;
    await syncShiftWeekFromApi();
    await Promise.all([
      refreshAccess(),
      refreshPersonnel(),
      refreshTime(),
      refreshAbsences(),
      refreshDashboard(),
      refreshAudit(),
    ]);
  }

  async function refreshAbsences() {
    const params = new URLSearchParams();
    if (absenceFilter !== 'all') params.set('status', absenceFilter);
    if (absenceEmployeeFilter) params.set('employee_id', absenceEmployeeFilter);
    const qs = params.toString();
    absences = await api(apiUrl, qs ? `/api/v1/absences?${qs}` : '/api/v1/absences');
  }

  async function refreshClockedIn() {
    if (!getToken()) return;
    clockedIn = await api<typeof clockedIn>(apiUrl, '/api/v1/time/clocked-in').catch(() => []);
  }

  async function refreshAccess() {
    zones = await api(apiUrl, '/api/v1/access/zones');
    doors = await api(apiUrl, '/api/v1/access/doors');
    const readerIds: string[] = [];
    for (const d of doors) {
      if (d.reader_in_id) readerIds.push(d.reader_in_id);
      if (d.reader_out_id) readerIds.push(d.reader_out_id);
    }
    if (readerIds.length > 0 && !readerIds.includes(simulateReader)) {
      simulateReader = readerIds[0];
    }
    badges = await api(apiUrl, '/api/v1/access/badges');
    let eventsPath = '/api/v1/access/events?limit=50';
    if (accessEventFilter !== 'all') {
      eventsPath += `&decision=${accessEventFilter}`;
    }
    if (accessEventEmployeeNo.trim()) {
      eventsPath += `&employee_no=${encodeURIComponent(accessEventEmployeeNo.trim())}`;
    }
    accessEvents = await api(apiUrl, eventsPath);
    accessRules = await api<typeof accessRules>(apiUrl, '/api/v1/access/rules').catch(() => []);
    zoneOccupancy = await api<typeof zoneOccupancy>(apiUrl, '/api/v1/access/occupancy').catch(() => []);
    if (zones.length && !newDoor.zone_id) newDoor.zone_id = zones[0].id;
    if (zones.length && !newAccessRule.zone_id) newAccessRule.zone_id = zones[0].id;
    if (employees.length && !newAccessRule.employee_id) {
      newAccessRule.employee_id = employees[0].id;
    }
  }

  async function refreshAudit() {
    const params = new URLSearchParams({ limit: String(Math.min(500, Math.max(1, auditLimit))) });
    if (auditObjectFilter) params.set('object_type', auditObjectFilter);
    if (auditActionFilter) params.set('action', auditActionFilter);
    if (auditActorFilter) params.set('actor_type', auditActorFilter);
    auditLog = await api<typeof auditLog>(apiUrl, `/api/v1/admin/audit?${params}`).catch(() => []);
  }

  async function refreshSystem() {
    await refreshAudit();
    sites = await api<typeof sites>(apiUrl, '/api/v1/admin/sites').catch(() => []);
    roles = await api<typeof roles>(apiUrl, '/api/v1/admin/roles').catch(() => []);
    await refreshHealth();
  }

  function startEditEmployee(e: { id: string; display_name: string; org_unit?: string | null }) {
    editingEmployeeId = e.id;
    editDisplayName = e.display_name;
    editOrgUnit = e.org_unit ?? '';
  }

  function cancelEditEmployee() {
    editingEmployeeId = '';
  }

  async function saveEmployeeEdit(id: string) {
    error = '';
    try {
      await api(apiUrl, `/api/v1/admin/employees/${id}`, {
        method: 'PATCH',
        body: JSON.stringify({
          display_name: editDisplayName.trim(),
          org_unit: editOrgUnit.trim() || '',
        }),
      });
      editingEmployeeId = '';
      await refreshPersonnel();
      success = 'Mitarbeiter aktualisiert';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function deactivateEmployee(id: string) {
    error = '';
    try {
      await api(apiUrl, `/api/v1/admin/employees/${id}/deactivate`, { method: 'POST' });
      await refreshPersonnel();
      await refreshAccess();
      await refreshDashboard();
      success = 'Mitarbeiter deaktiviert (Badges gesperrt)';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function disableUser(id: string) {
    error = '';
    try {
      await api(apiUrl, `/api/v1/admin/users/${id}/disable`, { method: 'POST' });
      await refreshPersonnel();
      success = 'Benutzer deaktiviert';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function enableUser(id: string) {
    error = '';
    try {
      await api(apiUrl, `/api/v1/admin/users/${id}/enable`, { method: 'POST' });
      await refreshPersonnel();
      success = 'Benutzer reaktiviert';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function resetUserPassword() {
    if (!resetPasswordUserId || resetPasswordValue.length < 6) {
      error = 'Benutzer und Passwort (min. 6) wählen';
      return;
    }
    error = '';
    try {
      await api(apiUrl, `/api/v1/admin/users/${resetPasswordUserId}/reset-password`, {
        method: 'POST',
        body: JSON.stringify({ new_password: resetPasswordValue }),
      });
      resetPasswordValue = '';
      success = 'Passwort zurückgesetzt (Benutzer muss sich neu anmelden)';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function reactivateEmployee(id: string) {
    error = '';
    try {
      await api(apiUrl, `/api/v1/admin/employees/${id}/reactivate`, { method: 'POST' });
      await refreshPersonnel();
      await refreshDashboard();
      success = 'Mitarbeiter reaktiviert';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function suggestBadgeUid() {
    const emp = employees.find((e) => e.id === newBadge.employee_id);
    if (emp) newBadge = { ...newBadge, credential_uid: `DEMO-${emp.employee_no}` };
  }

  function employeeHasActiveBadge(employeeId: string) {
    return badges.some((b) => b.employee_id === employeeId && b.status === 'active');
  }

  function employeeHasZoneAllow(employeeId: string) {
    return accessRules.some(
      (r) => r.principal_id === employeeId && r.mode === 'allow',
    );
  }

  async function grantZoneAccessForEmployee(id: string) {
    error = '';
    try {
      await api(apiUrl, `/api/v1/admin/employees/${id}/grant-zone-access`, {
        method: 'POST',
      });
      await refreshAccess();
      await refreshDashboard();
      success = 'Zutritt Büro (Allow-Regel) angelegt';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function setupEmployeeAccess(e: { id: string; employee_no: string }) {
    error = '';
    try {
      if (!employeeHasActiveBadge(e.id)) {
        await api(apiUrl, '/api/v1/access/badges', {
          method: 'POST',
          body: JSON.stringify({
            employee_id: e.id,
            credential_uid: `DEMO-${e.employee_no}`,
          }),
        });
      }
      if (!employeeHasZoneAllow(e.id)) {
        await api(apiUrl, `/api/v1/admin/employees/${e.id}/grant-zone-access`, {
          method: 'POST',
        });
      }
      await refreshAccess();
      await refreshPersonnel();
      await refreshDashboard();
      success = `Badge + Zutritt für ${e.employee_no} eingerichtet`;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  function copyText(text: string) {
    navigator.clipboard.writeText(text).then(() => {
      success = 'Kopiert';
    });
  }

  async function issueBadgeForEmployee(e: { id: string; employee_no: string }) {
    error = '';
    try {
      await api(apiUrl, '/api/v1/access/badges', {
        method: 'POST',
        body: JSON.stringify({
          employee_id: e.id,
          credential_uid: `DEMO-${e.employee_no}`,
        }),
      });
      await refreshAccess();
      success = `Badge DEMO-${e.employee_no} ausgestellt`;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  function copyApiUrl(url: string) {
    navigator.clipboard.writeText(url).then(() => {
      success = 'URL kopiert';
    });
  }

  async function refreshPersonnel() {
    const q = personnelSearch.trim();
    const qParam = q ? `&q=${encodeURIComponent(q)}` : '';
    const userPath = includeInactiveUsers
      ? `/api/v1/admin/users?include_inactive=true${qParam}`
      : `/api/v1/admin/users${q ? `?q=${encodeURIComponent(q)}` : ''}`;
    users = await api(apiUrl, userPath);
    const empPath = includeInactiveEmployees
      ? `/api/v1/admin/employees?include_inactive=true${qParam}`
      : `/api/v1/admin/employees${q ? `?q=${encodeURIComponent(q)}` : ''}`;
    employees = await api(apiUrl, empPath);
    roles = await api(apiUrl, '/api/v1/admin/roles');
    if (employees.length) {
      newBadge.employee_id = employees[0].id;
      if (!newAbsence.employee_id) newAbsence.employee_id = employees[0].id;
    }
  }

  function refreshShiftAndTimesheetPanels() {
    void shiftWeekCard?.refresh();
    void timesheetsCard?.refresh();
  }

  async function refreshTime() {
    await Promise.all([
      workCalendarCard?.refresh(),
      shiftWeekCard?.refresh(),
      timesheetsCard?.refresh(),
    ]);
  }

  async function cancelAbsence(id: string) {
    error = '';
    try {
      await api(apiUrl, `/api/v1/absences/${id}/cancel`, { method: 'POST' });
      await refreshAbsences();
      await refreshDashboard();
      success = 'Antrag storniert';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function exportAccessLog(fmt: 'csv' | 'html') {
    error = '';
    try {
      let path = `/api/v1/reports/access/export?format=${fmt}`;
      if (accessExportFrom) {
        path += `&from=${encodeURIComponent(fromLocalDatetimeInputValue(accessExportFrom))}`;
      }
      if (accessExportTo) {
        path += `&to=${encodeURIComponent(fromLocalDatetimeInputValue(accessExportTo))}`;
      }
      const fromTag = accessExportFrom
        ? fromLocalDatetimeInputValue(accessExportFrom).slice(0, 10)
        : 'alle';
      const toTag = accessExportTo
        ? fromLocalDatetimeInputValue(accessExportTo).slice(0, 10)
        : 'alle';
      const base = `zutritt_protokoll_${fromTag}_${toTag}`;
      if (fmt === 'csv') {
        await downloadFile(apiUrl, path, `${base}.csv`);
      } else {
        await openHtmlExport(apiUrl, path);
      }
      success = 'Zutritt-Export gestartet';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function doorStatusLabel(s: string) {
    const map: Record<string, string> = {
      closed: 'Geschlossen',
      open: 'Auf',
      forced_open: 'Dauerauf',
      alarm: 'Alarm',
    };
    return map[s] ?? s;
  }

  async function setDoorStatus(doorId: string, status: string) {
    await api(apiUrl, `/api/v1/access/doors/${doorId}/status`, {
      method: 'POST',
      body: JSON.stringify({ status }),
    });
    await refreshAccess();
    await refreshDashboard();
    success = 'Türstatus aktualisiert';
  }

  async function refreshDashboard() {
    if (!getToken()) return;
    dashboard = await api(apiUrl, '/api/v1/admin/dashboard');
    zoneOccupancy = await api<typeof zoneOccupancy>(apiUrl, '/api/v1/access/occupancy').catch(() => []);
    await refreshClockedIn();
  }

  async function foundationFix() {
    error = '';
    try {
      const res = await api<{
        calendars_assigned: number;
        timesheets_updated: number;
        warnings?: string[];
      }>(apiUrl, '/api/v1/admin/foundation-fix', { method: 'POST' });
      await refreshDashboard();
      await refreshPersonnel();
      await refreshTime();
      let msg = `${res.calendars_assigned} Kalender zugewiesen, ${res.timesheets_updated} Stundenzettel neu berechnet`;
      if (res.warnings?.length) msg += ` (${res.warnings.length} Hinweise)`;
      success = msg;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function grantWorkCalendarForEmployee(employeeId: string) {
    error = '';
    try {
      await api(apiUrl, `/api/v1/admin/employees/${employeeId}/grant-work-calendar`, {
        method: 'POST',
      });
      await refreshPersonnel();
      await refreshTime();
      success = 'Arbeitskalender zugewiesen';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function fillSimulateUidFromEmployee() {
    const emp = employees.find((e) => e.id === simulateEmployeeId);
    if (emp) simulateUid = `DEMO-${emp.employee_no}`;
  }

  async function simulateScan() {
    error = '';
    try {
      const res = await api<{ decision: string; reason_code: string }>(
        apiUrl,
        '/api/v1/access/simulate-scan',
        {
          method: 'POST',
          body: JSON.stringify({
            credential_uid: simulateUid,
            reader_id: simulateReader,
          }),
        },
      );
      await refreshAccess();
      await refreshDashboard();
      success = `Scan: ${accessDecisionLabel(res.decision)} (${reasonLabel(res.reason_code)})`;
      if (res.decision === 'grant' || res.decision === 'allow') {
        simulateReader =
          simulateReader === 'sim.reader.main' ? 'sim.reader.main.out' : 'sim.reader.main';
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function fetchAccessEventsSince(since: string) {
    let path = `/api/v1/access/events?limit=20&since=${encodeURIComponent(since)}`;
    if (accessEventFilter !== 'all') {
      path += `&decision=${accessEventFilter}`;
    }
    return api<typeof accessEvents>(apiUrl, path);
  }

  async function hardwarePresentScan() {
    error = '';
    try {
      const since = new Date().toISOString();
      await api<{ queued: boolean }>(apiUrl, '/api/v1/access/hardware-present', {
        method: 'POST',
        body: JSON.stringify({
          credential_uid: simulateUid,
          reader_id: simulateReader,
        }),
      });
      let latest: (typeof accessEvents)[0] | undefined;
      for (let i = 0; i < 25; i++) {
        const batch = await fetchAccessEventsSince(since);
        if (batch.length > 0) {
          latest = batch[0];
          break;
        }
        await new Promise((r) => setTimeout(r, 200));
      }
      await refreshAccess();
      await refreshDashboard();
      success = latest
        ? `Hardware-Kanal: ${accessDecisionLabel(latest.decision)} (${reasonLabel(latest.reason_code ?? '')})`
        : 'Hardware-Kanal: Timeout — Ereignis nicht sichtbar (Worker/TCP prüfen)';
      if (latest?.decision === 'grant' || latest?.decision === 'allow') {
        simulateReader =
          simulateReader === 'sim.reader.main' ? 'sim.reader.main.out' : 'sim.reader.main';
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function deleteAccessRule(id: string) {
    error = '';
    try {
      await api(apiUrl, `/api/v1/access/rules/${id}`, { method: 'DELETE' });
      await refreshAccess();
      await refreshDashboard();
      success = 'Zutrittsregel entfernt';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function updateAccessRuleValidTo(id: string, localValue: string) {
    error = '';
    try {
      const valid_to = localValue ? fromLocalDatetimeInputValue(localValue) : null;
      await api(apiUrl, `/api/v1/access/rules/${id}`, {
        method: 'PATCH',
        body: JSON.stringify({ valid_to }),
      });
      await refreshAccess();
      await refreshDashboard();
      success = valid_to ? 'Gültig-bis-Datum gespeichert' : 'Ablauf entfernt (unbegrenzt)';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function updateAccessRuleValidFrom(id: string, localValue: string) {
    error = '';
    if (!localValue) {
      error = 'Gültig-ab-Datum erforderlich';
      return;
    }
    try {
      await api(apiUrl, `/api/v1/access/rules/${id}`, {
        method: 'PATCH',
        body: JSON.stringify({ valid_from: fromLocalDatetimeInputValue(localValue) }),
      });
      await refreshAccess();
      await refreshDashboard();
      success = 'Gültig-ab-Datum gespeichert';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function clearAccessRuleValidTo(id: string) {
    await updateAccessRuleValidTo(id, '');
  }

  async function duplicateAccessRule(r: {
    principal_id: string;
    zone_id?: string | null;
    schedule_json?: string | null;
    valid_to?: string | null;
    valid_from: string;
  }) {
    error = '';
    if (!r.zone_id) {
      error = 'Zone fehlt';
      return;
    }
    try {
      const body: Record<string, unknown> = {
        employee_id: r.principal_id,
        zone_id: r.zone_id,
        schedule_json: r.schedule_json ?? null,
      };
      if (r.valid_to) body.valid_to = r.valid_to;
      await api(apiUrl, '/api/v1/access/rules', {
        method: 'POST',
        body: JSON.stringify(body),
      });
      await refreshAccess();
      await refreshDashboard();
      success = 'Zutrittsregel dupliziert';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function toggleAccessRuleSchedule(rule: {
    id: string;
    schedule_json?: string | null;
  }) {
    error = '';
    try {
      const schedule_json = rule.schedule_json
        ? null
        : JSON.stringify({
            timezone: 'Europe/Berlin',
            weekdays: [1, 2, 3, 4, 5],
            start: '08:00',
            end: '18:00',
          });
      await api(apiUrl, `/api/v1/access/rules/${rule.id}`, {
        method: 'PATCH',
        body: JSON.stringify({ schedule_json }),
      });
      await refreshAccess();
      await refreshDashboard();
      success = schedule_json ? 'Zeitplan Mo–Fr 08:00–18:00 gesetzt' : 'Zeitplan entfernt';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function createAccessRule() {
    error = '';
    try {
      const schedule_json = newAccessRule.use_schedule
        ? JSON.stringify({
            timezone: 'Europe/Berlin',
            weekdays: [1, 2, 3, 4, 5],
            start: newAccessRule.start,
            end: newAccessRule.end,
          })
        : null;
      const body: Record<string, unknown> = {
        employee_id: newAccessRule.employee_id,
        zone_id: newAccessRule.zone_id,
        schedule_json,
      };
      if (newAccessRule.valid_from_local) {
        body.valid_from = fromLocalDatetimeInputValue(newAccessRule.valid_from_local);
      }
      if (newAccessRule.valid_to_local) {
        body.valid_to = fromLocalDatetimeInputValue(newAccessRule.valid_to_local);
      }
      await api(apiUrl, '/api/v1/access/rules', {
        method: 'POST',
        body: JSON.stringify(body),
      });
      await refreshAccess();
      await refreshDashboard();
      newAccessRule.valid_to_local = '';
      success = 'Zutrittsregel angelegt';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function revokeBadge(id: string) {
    error = '';
    try {
      await api(apiUrl, `/api/v1/access/badges/${id}/revoke`, { method: 'POST' });
      await refreshAccess();
      success = 'Badge gesperrt';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function reasonLabel(code: string) {
    const map: Record<string, string> = {
      ok: 'OK',
      unknown_badge: 'Unbekannte Karte',
      unknown_door: 'Unbekannte Tür',
      no_permission: 'Keine Berechtigung',
      antipassback: 'Anti-Passback',
      unassigned_badge: 'Badge nicht zugewiesen',
      schedule_restricted: 'Außerhalb Zeitplan',
    };
    return map[code] ?? code;
  }

  function accessDecisionLabel(decision: string) {
    const map: Record<string, string> = {
      grant: 'Zutritt',
      deny: 'Abgelehnt',
      allow: 'Zutritt',
    };
    return map[decision] ?? decision;
  }

  async function createUser() {
    error = '';
    try {
      await api(apiUrl, '/api/v1/admin/users', {
        method: 'POST',
        body: JSON.stringify(newUser),
      });
      newUser = { username: '', password: '', display_name: '', role_name: 'employee' };
      await refreshPersonnel();
      success = 'Benutzer angelegt';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function createEmployee() {
    error = '';
    try {
      const res = await api<{ employee_no: string; id: string }>(apiUrl, '/api/v1/admin/employees', {
        method: 'POST',
        body: JSON.stringify({
          display_name: newEmployee.display_name,
          employee_no: newEmployee.employee_no.trim() || null,
          org_unit: newEmployee.org_unit.trim() || null,
          issue_badge: newEmployee.issue_badge,
          grant_zone_access: newEmployee.grant_zone_access,
          grant_work_calendar: newEmployee.grant_work_calendar,
        }),
      });
      newEmployee = {
        display_name: '',
        employee_no: '',
        org_unit: '',
        issue_badge: true,
        grant_zone_access: true,
        grant_work_calendar: true,
      };
      await refreshPersonnel();
      await refreshAccess();
      await refreshDashboard();
      success = `Mitarbeiter ${res.employee_no} angelegt`;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function linkEmployeeUser(employeeId: string) {
    const userId = linkUserByEmployee[employeeId];
    if (!userId) {
      error = 'Bitte Benutzer wählen';
      return;
    }
    error = '';
    try {
      await api(apiUrl, `/api/v1/admin/employees/${employeeId}`, {
        method: 'PATCH',
        body: JSON.stringify({ user_id: userId }),
      });
      await refreshPersonnel();
      success = 'Benutzer verknüpft';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function unlinkEmployeeUser(employeeId: string) {
    error = '';
    try {
      await api(apiUrl, `/api/v1/admin/employees/${employeeId}`, {
        method: 'PATCH',
        body: JSON.stringify({ user_id: '' }),
      });
      await refreshPersonnel();
      success = 'Verknüpfung entfernt';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  const usersWithoutEmployee = $derived(
    users.filter((u) => !employees.some((e) => e.user_id === u.id)),
  );

  async function checkAbsenceConflict() {
    if (!getToken()) return;
    try {
      const starts = fromLocalDatetimeInputValue(newAbsence.starts_local);
      const ends = fromLocalDatetimeInputValue(newAbsence.ends_local);
      let url = `/api/v1/absences/conflicts?starts_at=${encodeURIComponent(starts)}&ends_at=${encodeURIComponent(ends)}`;
      if (canApprove && newAbsence.employee_id) {
        url += `&employee_id=${encodeURIComponent(newAbsence.employee_id)}`;
      }
      const r = await api<{ has_conflict: boolean; message?: string }>(apiUrl, url);
      absenceConflict = r.has_conflict ? (r.message ?? 'Zeitraum kollidiert') : null;
    } catch {
      absenceConflict = null;
    }
  }

  async function createZone() {
    error = '';
    try {
      await api(apiUrl, '/api/v1/access/zones', {
        method: 'POST',
        body: JSON.stringify(newZone),
      });
      newZone = { name: '' };
      await refreshAccess();
      success = 'Zone angelegt';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function createDoor() {
    error = '';
    try {
      await api(apiUrl, '/api/v1/access/doors', {
        method: 'POST',
        body: JSON.stringify(newDoor),
      });
      await refreshAccess();
      success = 'Tür angelegt';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function createBadge() {
    error = '';
    try {
      await api(apiUrl, '/api/v1/access/badges', {
        method: 'POST',
        body: JSON.stringify({ ...newBadge, credential_type: 'card' }),
      });
      newBadge = { ...newBadge, credential_uid: '' };
      await refreshAccess();
      success = 'Badge angelegt';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function createAbsence() {
    error = '';
    try {
      const body: Record<string, unknown> = {
        absence_type: newAbsence.absence_type,
        starts_at: fromLocalDatetimeInputValue(newAbsence.starts_local),
        ends_at: fromLocalDatetimeInputValue(newAbsence.ends_local),
        reason: newAbsence.reason || null,
      };
      if (canApprove && newAbsence.employee_id) {
        body.employee_id = newAbsence.employee_id;
      }
      await api(apiUrl, '/api/v1/absences', {
        method: 'POST',
        body: JSON.stringify(body),
      });
      newAbsence.reason = '';
      await refreshAbsences();
      await refreshDashboard();
      success = 'Abwesenheitsantrag erstellt';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function absenceAction(id: string, action: 'approve' | 'reject') {
    error = '';
    try {
      await api(apiUrl, `/api/v1/absences/${id}/${action}`, {
        method: 'POST',
        body: JSON.stringify({ note: rejectReason || undefined }),
      });
      await refreshAbsences();
      await refreshDashboard();
      success = 'Antrag aktualisiert';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function changePassword() {
    error = '';
    try {
      await api(apiUrl, '/api/v1/auth/change-password', {
        method: 'POST',
        body: JSON.stringify({
          current_password: currentPassword,
          new_password: newPassword,
        }),
      });
      currentPassword = '';
      newPassword = '';
      success = 'Passwort geändert';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function logout() {
    try {
      await api(apiUrl, '/api/v1/auth/logout', { method: 'POST' });
    } catch {
      /* ignore */
    }
    setToken(null);
    user = null;
    tab = 'overview';
    success = 'Abgemeldet';
  }

  async function approveAllPendingAbsences() {
    error = '';
    try {
      const res = await api<{ approved: number }>(
        apiUrl,
        '/api/v1/absences/approve-pending',
        { method: 'POST' },
      );
      await refreshAbsences();
      await refreshDashboard();
      success = `${res.approved} Abwesenheiten freigegeben`;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

</script>

<div class="layout">
  <aside class="sidebar">
    <h1>TimeShards Server</h1>
    {#if user}
      <p class="muted" style="font-size: 0.85rem;">
        {user.display_name}
        {#if user.employee_no}
          <br />PN {user.employee_no}
        {/if}
      </p>
      <button class="secondary" style="margin-bottom: 0.75rem; width: 100%;" onclick={logout}>
        Abmelden
      </button>
    {/if}
    {#if user}
      <button class="secondary" style="width: 100%; margin-bottom: 0.5rem;" onclick={refreshAll}>
        Alles aktualisieren
      </button>
    {/if}
    <nav class="nav">
      <button class:active={tab === 'overview'} onclick={() => (tab = 'overview')}>Übersicht</button>
      <button class:active={tab === 'personnel'} onclick={() => (tab = 'personnel')}>Personal</button>
      <button class:active={tab === 'time'} onclick={() => (tab = 'time')}>
        Zeit
        {#if dashboard && dashboard.pending_timesheets > 0}
          <span class="nav-badge">{dashboard.pending_timesheets}</span>
        {/if}
        {#if dashboard && dashboard.draft_timesheets > 0}
          <span class="nav-badge draft">{dashboard.draft_timesheets}</span>
        {/if}
        {#if dashboard && dashboard.planned_shifts_this_week > 0}
          <span class="nav-badge planned" title="Geplante Schichten diese Woche"
            >{dashboard.planned_shifts_this_week}</span
          >
        {/if}
      </button>
      <button class:active={tab === 'absence'} onclick={() => (tab = 'absence')}>
        Abwesenheit
        {#if dashboard && dashboard.pending_absences > 0}
          <span class="nav-badge">{dashboard.pending_absences}</span>
        {/if}
      </button>
      <button class:active={tab === 'access'} onclick={() => (tab = 'access')}>
        Zutritt
        {#if dashboard && dashboard.doors_alarm > 0}
          <span class="nav-badge alert">{dashboard.doors_alarm}</span>
        {/if}
      </button>
      <button class:active={tab === 'system'} onclick={() => (tab = 'system')}>System</button>
    </nav>
  </aside>

  <main class="content">
    {#if !user && tab !== 'overview'}
      <p class="error">Bitte zuerst unter Übersicht anmelden.</p>
    {:else if tab === 'overview'}
      <h2>Server-Übersicht</h2>
      {#if user}
        <button class="secondary" style="margin-top: 0.5rem;" onclick={refreshAll}>
          Aktualisieren
        </button>
      {/if}
      <div class="card" style="margin-top: 1rem;">
        <p><strong>API bindet auf:</strong> {apiBind || '…'}</p>
        {#if apiHealth}
          <p class="muted">
            API {apiHealth.service} v{apiHealth.version} - {apiHealth.status}
            (DB: {apiHealth.database})
          </p>
          {#if apiHealth.status !== 'ok'}
            <p class="error" style="margin-top: 0.35rem;">API degradiert — Details prüfen</p>
          {/if}
          {#if apiHealth.demo_seeding_enabled || apiHealth.default_password_login_blocked}
            <p class="error" style="margin-top: 0.35rem; font-size: 0.85rem;">
              {#if apiHealth.default_password_login_blocked && !apiHealth.demo_seeding_enabled}
                Produktionsmodus — Standardpasswörter gesperrt.
              {:else if apiHealth.default_password_login_blocked}
                Staging: Standardpasswörter gesperrt (<code>TIMESHARDS_BLOCK_DEFAULT_PASSWORDS</code>).
              {:else}
                Demo-Seeding aktiv — für Produktion <code>TIMESHARDS_DISABLE_DEMO=1</code>
              {/if}
            </p>
          {/if}
          {#if apiHealth.time_foundation}
            {@const tf = apiHealth.time_foundation}
            <p class="muted" style="margin-top: 0.35rem; font-size: 0.85rem;">
              Zeitbasis: {tf.workday_models} Tagesmodelle, {tf.work_calendars} Kalender, {tf.active_employees}{' '}
              aktive MA
              {#if tf.employees_without_work_calendar > 0 || tf.current_week_drafts_without_soll > 0}
                — <span style="color: #b8860b;">
                  {#if tf.employees_without_work_calendar > 0}
                    {tf.employees_without_work_calendar} ohne Kalender
                  {/if}
                  {#if tf.current_week_drafts_without_soll > 0}
                    {#if tf.employees_without_work_calendar > 0}; {/if}
                    {tf.current_week_drafts_without_soll} KW ohne Soll
                  {/if}
                </span>
              {:else}
                — OK
              {/if}
            </p>
          {/if}
        {/if}
        <button class="secondary" style="margin-top: 0.35rem;" onclick={refreshHealth}>
          API-Status prüfen
        </button>
        <p><strong>Client-URLs (im LAN):</strong></p>
        <ul>
          {#each apiUrls as u}
            <li>
              <code>{u}</code>
              <button class="secondary" onclick={() => copyApiUrl(u)}>Kopieren</button>
            </li>
          {/each}
        </ul>
        <p class="muted">
          Firewall: als Administrator <code>scripts/open-firewall.ps1</code> ausführen.
        </p>
        <p><strong>Datenbank:</strong> {dbPath || '…'}</p>
        {#if dashboard?.hardware_adapter}
          <p class="muted" style="font-size: 0.85rem; margin-top: 0.35rem;">
            System: HW <code>{dashboard.hardware_adapter}</code>
            {#if dashboard.default_password_login_blocked}
              · Standardpasswörter gesperrt
            {/if}
            {#if dashboard.demo_seeding_enabled}
              · Demo-Seeding
            {/if}
          </p>
        {/if}
      </div>
      <div class="card" style="margin-top: 1rem;">
        <h3>Anmeldung</h3>
        {#if user}
          <p class="success">Angemeldet als {user.display_name} ({user.roles.join(', ')})</p>
          {#if dashboard}
            <div class="stat-grid">
              <button
                type="button"
                class="stat-card stat-card-btn"
                onclick={() => {
                  tab = 'time';
                }}
              >
                <span class="muted">Eingestempelt</span>
                <strong>{dashboard.clocked_in_employees}</strong>
                <span class="muted">/ {dashboard.employees_total} MA</span>
              </button>
              <button
                type="button"
                class="stat-card stat-card-btn"
                onclick={() => {
                  tab = 'time';
                  timesheetFilter = 'pending';
                  void refreshTime();
                }}
              >
                <span class="muted">Stundenzettel offen</span>
                <strong>{dashboard.pending_timesheets}</strong>
              </button>
              {#if dashboard.draft_timesheets > 0}
                <button
                  type="button"
                  class="stat-card stat-card-btn"
                  onclick={() => {
                    tab = 'time';
                    timesheetFilter = 'draft';
                    void refreshTime();
                  }}
                >
                  <span class="muted">Entwürfe / abgelehnt</span>
                  <strong>{dashboard.draft_timesheets}</strong>
                </button>
              {/if}
              <button
                type="button"
                class="stat-card stat-card-btn"
                onclick={() => {
                  tab = 'absence';
                  absenceFilter = 'pending';
                  void refreshAbsences();
                }}
              >
                <span class="muted">Abwesenheit offen</span>
                <strong>{dashboard.pending_absences}</strong>
              </button>
              <button
                type="button"
                class="stat-card stat-card-btn"
                onclick={() => {
                  tab = 'time';
                  shiftFilter = 'planned';
                  void refreshTime();
                }}
              >
                <span class="muted">Schichten diese Woche</span>
                <strong>{dashboard.shifts_this_week}</strong>
                {#if dashboard.planned_shifts_this_week > 0}
                  <span class="muted"> ({dashboard.planned_shifts_this_week} geplant)</span>
                {/if}
              </button>
              <button
                type="button"
                class="stat-card stat-card-btn"
                onclick={() => {
                  tab = 'access';
                }}
              >
                <span class="muted">Im Gebäude (Zonen)</span>
                <strong>{dashboard.people_in_building}</strong>
              </button>
              {#if dashboard.doors_alarm + dashboard.doors_forced_open + dashboard.doors_open > 0}
                <button
                  type="button"
                  class="stat-card stat-card-btn"
                  style="border-color: #b00020;"
                  onclick={() => {
                    tab = 'access';
                  }}
                >
                  <span class="muted">Tür-Alerts</span>
                  <strong>{dashboard.doors_alarm + dashboard.doors_forced_open + dashboard.doors_open}</strong>
                </button>
              {/if}
              {#if (dashboard.employees_without_work_calendar ?? 0) > 0}
                <button
                  type="button"
                  class="stat-card stat-card-btn"
                  style="border-color: #b8860b;"
                  onclick={() => {
                    tab = 'time';
                    void refreshTime();
                  }}
                >
                  <span class="muted">MA ohne Arbeitskalender</span>
                  <strong>{dashboard.employees_without_work_calendar}</strong>
                </button>
              {/if}
              {#if (dashboard.timesheets_current_week_no_soll ?? 0) > 0}
                <button
                  type="button"
                  class="stat-card stat-card-btn"
                  style="border-color: #b8860b;"
                  onclick={() => {
                    tab = 'time';
                    timesheetFilter = 'draft';
                    void refreshTime();
                  }}
                >
                  <span class="muted">KW ohne Soll (Entwurf)</span>
                  <strong>{dashboard.timesheets_current_week_no_soll}</strong>
                </button>
              {/if}
            </div>
            {#if (dashboard.employees_without_work_calendar ?? 0) > 0 || (dashboard.timesheets_current_week_no_soll ?? 0) > 0}
              <div class="btn-row" style="margin-top: 0.75rem;">
                <button type="button" onclick={foundationFix}>Zeitbasis reparieren</button>
              </div>
              <p class="muted" style="margin-top: 0.5rem; font-size: 0.85rem;">
                {#if (dashboard.employees_without_work_calendar ?? 0) > 0}
                  {dashboard.employees_without_work_calendar} aktive Mitarbeiter ohne Arbeitskalender —
                  Zuweisung + Neuberechnung der aktuellen KW.
                {/if}
                {#if (dashboard.timesheets_current_week_no_soll ?? 0) > 0}
                  {#if (dashboard.employees_without_work_calendar ?? 0) > 0}<br />{/if}
                  {dashboard.timesheets_current_week_no_soll} Entwürfe diese KW ohne Soll.
                {/if}
              </p>
            {/if}
            {#if zoneOccupancy.some((z) => z.inside_count > 0)}
              <div style="margin-top: 0.75rem;">
                <p class="muted">Im Gebäude (Zonen):</p>
                <ul class="compact-list">
                  {#each zoneOccupancy.filter((z) => z.inside_count > 0) as z}
                    <li>
                      <strong>{z.zone_name}</strong>: {z.inside_count}
                      {#if z.occupants.length}
                        — {z.occupants.map((o) => o.display_name).join(', ')}
                      {/if}
                    </li>
                  {/each}
                </ul>
              </div>
            {/if}
            {#if clockedIn.length}
              <div style="margin-top: 0.75rem;">
                <p class="muted">Eingestempelt jetzt:</p>
                <ul class="compact-list">
                  {#each clockedIn as c}
                    <li>
                      {c.employee_no} {c.display_name} — {statusLabel(c.last_kind)}
                      {#if c.is_on_break}<span class="muted"> (Pause)</span>{/if}
                      <span class="muted">
                        seit {formatIsoLocalShort(c.last_at)}
                      </span>
                    </li>
                  {/each}
                </ul>
              </div>
            {/if}
            {#if dashboard.door_alerts.length}
              <ul class="compact-list" style="margin-top: 0.75rem;">
                {#each dashboard.door_alerts as d}
                  <li>
                    {d.name} — <strong>{doorStatusLabel(d.status)}</strong>
                    <button class="secondary" onclick={() => { tab = 'access'; setDoorStatus(d.id, 'closed'); }}>
                      Zurücksetzen
                    </button>
                  </li>
                {/each}
              </ul>
            {/if}
          {/if}
        {:else}
          <div style="display: grid; gap: 0.5rem; max-width: 320px;">
            <input
              bind:value={username}
              placeholder="Benutzername"
              onkeydown={(e) => e.key === 'Enter' && login()}
            />
            <input
              type="password"
              bind:value={password}
              placeholder="Passwort"
              onkeydown={(e) => e.key === 'Enter' && login()}
            />
            <div class="btn-row">
              <button class="secondary" onclick={() => { username = 'admin'; password = 'admin'; }}>
                admin
              </button>
              <button class="secondary" onclick={() => { username = 'demo'; password = 'demo'; }}>
                demo
              </button>
              <button class="secondary" onclick={() => { username = 'manager'; password = 'demo'; }}>
                manager
              </button>
            </div>
            <button onclick={login}>Anmelden</button>
          </div>
        {/if}
      </div>
    {:else if tab === 'personnel'}
      <h2>Personal</h2>
      <div class="grid-form" style="margin-top: 0.75rem; max-width: 480px;">
        <input
          bind:value={personnelSearch}
          placeholder="Suche Name, PN, Benutzername…"
          onkeydown={(e) => e.key === 'Enter' && refreshPersonnel()}
        />
        <button class="secondary" onclick={refreshPersonnel}>Suchen</button>
      </div>
      <div class="card" style="margin-top: 1rem;">
        <h3>Neuer Benutzer (+ Mitarbeiter)</h3>
        <div class="grid-form">
          <input bind:value={newUser.username} placeholder="Benutzername" />
          <input bind:value={newUser.display_name} placeholder="Anzeigename" />
          <input type="password" bind:value={newUser.password} placeholder="Passwort (min. 6)" />
          <select bind:value={newUser.role_name}>
            {#each roles as r}<option value={r.name}>{r.name}</option>{/each}
          </select>
          <button onclick={createUser}>Anlegen</button>
        </div>
      </div>
      <div class="card" style="margin-top: 1rem;">
        <h3>Benutzer</h3>
        <label class="muted">
          <input
            type="checkbox"
            bind:checked={includeInactiveUsers}
            onchange={() => refreshPersonnel()}
          />
          Inaktive Benutzer anzeigen
        </label>
        <div class="grid-form" style="margin: 0.75rem 0;">
          <select bind:value={resetPasswordUserId}>
            <option value="">Passwort zurücksetzen für…</option>
            {#each users as u}
              <option value={u.id}>{u.username} — {u.display_name}</option>
            {/each}
          </select>
          <input type="password" bind:value={resetPasswordValue} placeholder="Neues Passwort (min. 6)" />
          <button class="secondary" onclick={resetUserPassword}>Passwort setzen</button>
        </div>
        <ul class="compact-list">
          {#each users as u}
            <li class="row-card">
              {u.display_name} — <code>{u.username}</code> ({u.roles.join(', ')})
              {#if u.status !== 'active'}<span class="muted"> [inaktiv]</span>{/if}
              {#if u.id !== user?.id}
                {#if u.status === 'active'}
                  <button class="secondary" onclick={() => disableUser(u.id)}>Deaktivieren</button>
                {:else}
                  <button class="secondary" onclick={() => enableUser(u.id)}>Reaktivieren</button>
                {/if}
              {/if}
            </li>
          {/each}
        </ul>
        <h3>Mitarbeiter (ohne Login)</h3>
        <label class="muted">
          <input
            type="checkbox"
            bind:checked={includeInactiveEmployees}
            onchange={() => refreshPersonnel()}
          />
          Inaktive anzeigen
        </label>
        <label class="muted">
          <input type="checkbox" bind:checked={personnelShowSetupOpen} />
          Nur ohne Badge oder Zutritt
        </label>
        <label class="muted">
          <input type="checkbox" bind:checked={personnelShowNoCalendar} />
          Nur ohne Arbeitskalender
        </label>
        {#if personnelShowSetupOpen || personnelShowNoCalendar}
          <span class="muted">({displayedEmployees.length} Treffer)</span>
        {/if}
        <div class="grid-form">
          <input bind:value={newEmployee.display_name} placeholder="Anzeigename" />
          <input bind:value={newEmployee.employee_no} placeholder="Personalnr. (leer = auto)" />
          <input bind:value={newEmployee.org_unit} placeholder="Organisation (optional)" />
          <label class="muted">
            <input type="checkbox" bind:checked={newEmployee.issue_badge} />
            Demo-Badge ausstellen (DEMO-PN)
          </label>
          <label class="muted">
            <input type="checkbox" bind:checked={newEmployee.grant_zone_access} />
            Zutritt Büro (Allow-Regel)
          </label>
          <label class="muted">
            <input type="checkbox" bind:checked={newEmployee.grant_work_calendar} />
            Arbeitskalender (Sollzeit, Standard)
          </label>
          <button onclick={createEmployee}>Mitarbeiter anlegen</button>
        </div>
        <ul class="compact-list" style="margin-top: 0.75rem;">
          {#each displayedEmployees as e}
            <li class="row-card">
              {#if editingEmployeeId === e.id}
                <div class="grid-form">
                  <input bind:value={editDisplayName} placeholder="Anzeigename" />
                  <input bind:value={editOrgUnit} placeholder="Organisation" />
                  <button onclick={() => saveEmployeeEdit(e.id)}>Speichern</button>
                  <button class="secondary" onclick={cancelEditEmployee}>Abbrechen</button>
                </div>
              {:else}
              {e.employee_no} — {e.display_name}
              {#if e.org_unit}<span class="muted"> ({e.org_unit})</span>{/if}
              {#if e.active !== false && e.work_calendar_assigned === false}
                <span class="error" style="font-size: 0.85rem;"> · Kein Arbeitskalender</span>
              {/if}
              <button class="secondary" onclick={() => startEditEmployee(e)}>Bearbeiten</button>
              {#if e.active !== false && e.work_calendar_assigned === false}
                <button class="secondary" onclick={() => grantWorkCalendarForEmployee(e.id)}>
                  Arbeitskalender
                </button>
              {/if}
              {#if e.active !== false && !employeeHasActiveBadge(e.id)}
                <button class="secondary" onclick={() => issueBadgeForEmployee(e)}>
                  Badge ausstellen
                </button>
              {/if}
              {#if e.active !== false && !employeeHasZoneAllow(e.id)}
                <button class="secondary" onclick={() => grantZoneAccessForEmployee(e.id)}>
                  Zutritt Büro
                </button>
              {/if}
              {#if e.active !== false && (!employeeHasActiveBadge(e.id) || !employeeHasZoneAllow(e.id))}
                <button class="secondary" onclick={() => setupEmployeeAccess(e)}>
                  Badge + Zutritt
                </button>
              {/if}
              {#if e.active === false}
                <span class="muted"> (inaktiv)</span>
                <button class="secondary" onclick={() => reactivateEmployee(e.id)}>Reaktivieren</button>
              {:else if e.username}
                <span class="muted"> — Login: <code>{e.username}</code></span>
                <button class="secondary" onclick={() => unlinkEmployeeUser(e.id)}>Login trennen</button>
                <button class="secondary" onclick={() => deactivateEmployee(e.id)}>Deaktivieren</button>
              {:else}
                <select
                  bind:value={linkUserByEmployee[e.id]}
                  onchange={(ev) => {
                    linkUserByEmployee = { ...linkUserByEmployee, [e.id]: ev.currentTarget.value };
                  }}
                >
                  <option value="">Benutzer wählen…</option>
                  {#each usersWithoutEmployee as u}
                    <option value={u.id}>{u.username} — {u.display_name}</option>
                  {/each}
                </select>
                <button class="secondary" onclick={() => linkEmployeeUser(e.id)}>Verknüpfen</button>
                <button class="secondary" onclick={() => deactivateEmployee(e.id)}>Deaktivieren</button>
              {/if}
              {/if}
            </li>
          {:else}
            <li class="muted">
              {#if personnelShowSetupOpen}
                Keine Mitarbeiter ohne Badge oder Zutritt — alles eingerichtet.
              {:else}
                Keine Mitarbeiter in der Liste.
              {/if}
            </li>
          {/each}
        </ul>
      </div>
    {:else if tab === 'time'}
      <h2>Zeit & Schichten</h2>
      <WorkCalendarCard
        bind:this={workCalendarCard}
        {apiUrl}
        bind:shiftWeekAnchor
        {employees}
        active={tab === 'time'}
        onMessage={handleUiMessage}
        onWeekChange={refreshShiftAndTimesheetPanels}
      />
      <ShiftWeekCard
        bind:this={shiftWeekCard}
        {apiUrl}
        {employees}
        bind:shiftWeekAnchor
        bind:shiftFilter
        active={tab === 'time'}
        {canApprove}
        onMessage={handleUiMessage}
        onDashboardChange={refreshDashboard}
      />
      <TimesheetsCard
        bind:this={timesheetsCard}
        {apiUrl}
        {employees}
        {user}
        bind:shiftWeekAnchor
        bind:timesheetFilter
        active={tab === 'time'}
        {canApprove}
        {canCorrectTime}
        pendingTimesheets={dashboard?.pending_timesheets ?? 0}
        onMessage={handleUiMessage}
        onDashboardChange={refreshDashboard}
      />
    {:else if tab === 'absence'}
      <h2>Abwesenheit</h2>
      <div class="card" style="margin-top: 1rem;">
        <h3>Neuer Antrag</h3>
        <div class="grid-form">
          {#if canApprove}
            <select bind:value={newAbsence.employee_id}>
              {#each employees as e}
                <option value={e.id}>{e.employee_no} — {e.display_name}</option>
              {/each}
            </select>
          {/if}
          <select bind:value={newAbsence.absence_type}>
            <option value="urlaub">Urlaub</option>
            <option value="krank">Krank</option>
            <option value="sonder">Sonderurlaub</option>
            <option value="unbezahlt">Unbezahlt</option>
          </select>
          <label class="muted">Von</label>
          <input type="datetime-local" bind:value={newAbsence.starts_local} />
          <label class="muted">Bis</label>
          <input type="datetime-local" bind:value={newAbsence.ends_local} />
          <input bind:value={newAbsence.reason} placeholder="Begründung (optional)" />
          <button onclick={createAbsence} disabled={!!absenceConflict}>Antrag stellen</button>
          {#if absenceConflict}<p class="error">{absenceConflict}</p>{/if}
        </div>
      </div>
      <div class="card" style="margin-top: 1rem;">
        <div class="btn-row" style="margin-bottom: 0.5rem;">
          <select bind:value={absenceFilter} onchange={() => refreshAbsences()}>
            <option value="all">Alle Anträge</option>
            <option value="pending">Offen</option>
            <option value="approved">Freigegeben</option>
            <option value="rejected">Abgelehnt</option>
          </select>
          {#if canApprove}
            <select bind:value={absenceEmployeeFilter} onchange={() => refreshAbsences()}>
              <option value="">Alle Mitarbeiter</option>
              {#each employees.filter((e) => e.active !== false) as e}
                <option value={e.id}>{e.employee_no} — {e.display_name}</option>
              {/each}
            </select>
          {/if}
          {#if canApprove && dashboard && dashboard.pending_absences > 0}
            <button class="secondary" onclick={approveAllPendingAbsences}>
              Alle {dashboard.pending_absences} freigeben
            </button>
          {/if}
        </div>
        <h3>Anträge</h3>
        {#each absences as a}
          <div class="row-card">
            <p>
              <strong>{a.employee_no} {a.employee_name}</strong> — {statusLabel(a.absence_type)} —
              {formatIsoLocalShort(a.starts_at)} → {formatIsoLocalShort(a.ends_at)} — <em>{statusLabel(a.status)}</em>
            </p>
            {#if a.reason}<p class="muted">{a.reason}</p>{/if}
            {#if canApprove && a.status === 'pending'}
              <div class="btn-row">
                <button onclick={() => absenceAction(a.id, 'approve')}>Freigeben</button>
                <button class="secondary" onclick={() => absenceAction(a.id, 'reject')}>
                  Ablehnen
                </button>
                <button class="secondary" onclick={() => cancelAbsence(a.id)}>Stornieren</button>
              </div>
            {:else if a.status === 'pending' || a.status === 'approved'}
              <button class="secondary" onclick={() => cancelAbsence(a.id)}>Stornieren</button>
            {/if}
          </div>
        {:else}
          <p class="muted">Keine Anträge</p>
        {/each}
      </div>
    {:else if tab === 'access'}
      <h2>Zutritt</h2>
      <div class="card" style="margin-top: 1rem;">
        <h3>Neue Zone</h3>
        <div class="grid-form">
          <input bind:value={newZone.name} placeholder="Zonenname" />
          <button onclick={createZone}>Zone anlegen</button>
        </div>
        <h3 style="margin-top: 1rem;">Neue Tür</h3>
        <div class="grid-form">
          <input bind:value={newDoor.name} placeholder="Türname" />
          <select bind:value={newDoor.zone_id}>
            {#each zones as z}<option value={z.id}>{z.name}</option>{/each}
          </select>
          <input bind:value={newDoor.reader_id} placeholder="Reader-ID" />
          <button onclick={createDoor}>Tür anlegen</button>
        </div>
        <h3 style="margin-top: 1rem;">Badge ausgeben</h3>
        <div class="grid-form">
          <select bind:value={newBadge.employee_id}>
            {#each employees as e}
              <option value={e.id}>{e.employee_no} — {e.display_name}</option>
            {/each}
          </select>
          <input bind:value={newBadge.credential_uid} placeholder="Credential UID" />
          <button class="secondary" onclick={suggestBadgeUid}>DEMO-PN vorschlagen</button>
          <button onclick={createBadge}>Badge anlegen</button>
        </div>
      </div>
      <div class="card" style="margin-top: 1rem;">
        <h3>Zutrittsregel</h3>
        <div class="grid-form">
          <select bind:value={newAccessRule.employee_id}>
            {#each employees as e}
              <option value={e.id}>{e.employee_no} — {e.display_name}</option>
            {/each}
          </select>
          <select bind:value={newAccessRule.zone_id}>
            {#each zones as z}<option value={z.id}>{z.name}</option>{/each}
          </select>
          <label class="muted">
            <input type="checkbox" bind:checked={newAccessRule.use_schedule} />
            Mo–Fr Zeitfenster
          </label>
          {#if newAccessRule.use_schedule}
            <input bind:value={newAccessRule.start} placeholder="08:00" />
            <input bind:value={newAccessRule.end} placeholder="18:00" />
          {/if}
          <label class="muted">Gültig ab (optional, sonst jetzt)</label>
          <input type="datetime-local" bind:value={newAccessRule.valid_from_local} />
          <label class="muted">Gültig bis (optional)</label>
          <input type="datetime-local" bind:value={newAccessRule.valid_to_local} />
          <button onclick={createAccessRule}>Regel (Allow) anlegen</button>
        </div>
        <div class="btn-row" style="margin-top: 0.5rem;">
          <select bind:value={accessRuleEmployeeFilter}>
            <option value="">Alle Mitarbeiter</option>
            {#each employees.filter((e) => e.active !== false) as e}
              <option value={e.id}>{e.employee_no} — {e.display_name}</option>
            {/each}
          </select>
          <select bind:value={accessRuleZoneFilter}>
            <option value="">Alle Zonen</option>
            {#each zones as z}
              <option value={z.id}>{z.name}</option>
            {/each}
          </select>
        </div>
        <ul class="compact-list" style="margin-top: 0.75rem;">
          {#each displayedAccessRules as r}
            <li class="row-card">
              {r.employee_name ?? '—'} → {r.zone_name ?? 'Zone'} ({r.mode})
              {#if r.schedule_json}<span class="muted"> — Zeitplan aktiv</span>{/if}
              <span class="muted">
                — ab {formatIsoLocalShort(r.valid_from)}
                {#if r.valid_to}
                  bis {formatIsoLocalShort(r.valid_to)}
                {:else}
                  (unbegrenzt)
                {/if}
              </span>
              <div class="grid-form" style="margin-top: 0.35rem;">
                <label class="muted">Gültig ab</label>
                <input
                  type="datetime-local"
                  value={isoToLocalDatetimeInput(r.valid_from)}
                  onchange={(ev) => updateAccessRuleValidFrom(r.id, ev.currentTarget.value)}
                />
                <label class="muted">Gültig bis</label>
                <input
                  type="datetime-local"
                  value={isoToLocalDatetimeInput(r.valid_to)}
                  onchange={(ev) => updateAccessRuleValidTo(r.id, ev.currentTarget.value)}
                />
                {#if r.valid_to}
                  <button class="secondary" onclick={() => clearAccessRuleValidTo(r.id)}>
                    Unbegrenzt
                  </button>
                {/if}
              </div>
              <button class="secondary" onclick={() => toggleAccessRuleSchedule(r)}>
                {r.schedule_json ? 'Zeitplan aus' : 'Mo–Fr 08–18'}
              </button>
              <button class="secondary" onclick={() => duplicateAccessRule(r)}>Duplizieren</button>
              <button class="secondary" onclick={() => deleteAccessRule(r.id)}>Entfernen</button>
            </li>
          {:else}
            <li class="muted">Keine Regeln für diesen Filter.</li>
          {/each}
        </ul>
        <h3 style="margin-top: 1rem;">Belegung</h3>
        {#each zoneOccupancy as z}
          <p>
            <strong>{z.zone_name}</strong>: {z.inside_count} im Gebäude
            {#if z.occupants.length}
              — {z.occupants.map((o) => o.display_name).join(', ')}
            {/if}
          </p>
        {/each}
      </div>
      <div class="card" style="margin-top: 1rem;">
        <h3>Türen</h3>
        <ul class="compact-list">
          {#each doors as d}
            <li>
              {d.name} — <em>{doorStatusLabel(d.status)}</em>
              {#if d.reader_in_id || d.reader_out_id}
                <p class="muted" style="font-size: 0.85rem; margin: 0.25rem 0;">
                  {#if d.reader_in_id}Eingang: <code>{d.reader_in_id}</code>{/if}
                  {#if d.reader_in_id && d.reader_out_id}
                    ·
                  {/if}
                  {#if d.reader_out_id}Ausgang: <code>{d.reader_out_id}</code>{/if}
                </p>
              {/if}
              <div class="btn-row">
                <button class="secondary" onclick={() => setDoorStatus(d.id, 'closed')}>
                  Zu
                </button>
                <button class="secondary" onclick={() => setDoorStatus(d.id, 'open')}>
                  Auf
                </button>
                <button class="secondary" onclick={() => setDoorStatus(d.id, 'forced_open')}>
                  Offen
                </button>
                <button class="secondary" onclick={() => setDoorStatus(d.id, 'alarm')}>
                  Alarm
                </button>
              </div>
            </li>
          {/each}
        </ul>
      </div>
      <div class="card" style="margin-top: 1rem;">
        <h3>Simulator</h3>
        <div class="grid-form" style="margin-bottom: 0.5rem;">
          <label class="muted">Export von</label>
          <input type="datetime-local" bind:value={accessExportFrom} />
          <label class="muted">Export bis</label>
          <input type="datetime-local" bind:value={accessExportTo} />
        </div>
        <div class="btn-row" style="margin-bottom: 0.5rem;">
          <button class="secondary" onclick={() => exportAccessLog('csv')}>Zutritt CSV</button>
          <button class="secondary" onclick={() => exportAccessLog('html')}>Zutritt HTML/PDF</button>
        </div>
        <p class="muted" style="font-size: 0.85rem; margin: 0 0 0.75rem;">
          Export enthält alle sichtbaren Ereignisse (Admin/HR/Manager/Sicherheit). Optional Zeitraum oben
          einschränken.
        </p>
        <select
          bind:value={simulateEmployeeId}
          onchange={fillSimulateUidFromEmployee}
          style="margin-bottom: 0.5rem;"
        >
          <option value="">Mitarbeiter für DEMO-UID…</option>
          {#each employees.filter((e) => e.active !== false) as e}
            <option value={e.id}>{e.employee_no} — {e.display_name}</option>
          {/each}
        </select>
        <input bind:value={simulateUid} placeholder="Credential UID" />
        <div class="btn-row" style="margin-top: 0.5rem;">
          <button
            class="secondary"
            onclick={() => {
              simulateUid = 'DEMO-ADMIN-001';
            }}
          >
            DEMO-ADMIN-001
          </button>
          <button
            class="secondary"
            onclick={() => {
              simulateUid = 'DEMO-0002';
            }}
          >
            DEMO-0002
          </button>
          <button
            class="secondary"
            onclick={() => {
              simulateUid = 'DEMO-0003';
            }}
          >
            DEMO-0003
          </button>
        </div>
        <select bind:value={simulateReader} style="margin-top: 0.5rem;">
          {#if doorReaders.length === 0}
            <option value="sim.reader.main">Eingang (sim.reader.main)</option>
            <option value="sim.reader.main.out">Ausgang (sim.reader.main.out)</option>
          {:else}
            {#each doorReaders as r}
              <option value={r.id}>{r.label} ({r.id})</option>
            {/each}
          {/if}
        </select>
        <div class="btn-row" style="margin-top: 0.5rem;">
          <button onclick={simulateScan}>Scan (REST)</button>
          <button class="secondary" onclick={hardwarePresentScan}>Scan (Hardware-Kanal)</button>
        </div>
        <p class="muted" style="margin-top: 0.5rem;">
          REST = sofortige Antwort. Hardware-Kanal = wie ein echter Leser (für
          <code>TIMESHARDS_HW_ADAPTER=external</code>). Zweiter Eingangs-Scan ohne Ausgang →
          Anti-Passback.
        </p>
        {#if apiHealth?.hardware_tcp_listen}
          <p class="muted" style="margin-top: 0.35rem; font-size: 0.9rem;">
            TCP-Ingest aktiv: <code>{apiHealth.hardware_tcp_listen}</code> (JSON / kompakte Zeilen,
            siehe docs/HARDWARE.md). Tür-Status-Updates erscheinen in der Türliste und auf der
            Übersicht.
          </p>
        {/if}
        <h3 style="margin-top: 1rem;">Badges</h3>
        <ul class="compact-list">
          {#each badges as b}
            <li>
              <code>{b.credential_uid}</code>
              <button class="secondary" onclick={() => copyText(b.credential_uid)}>Kopieren</button>
              {#if b.employee_name}
                — {b.employee_no} {b.employee_name}
              {/if}
              — {b.status}
              {#if b.status === 'active'}
                <button class="secondary" onclick={() => revokeBadge(b.id)}>Sperren</button>
              {/if}
            </li>
          {/each}
        </ul>
        <h3>Letzte Ereignisse</h3>
        <div class="btn-row" style="margin-bottom: 0.5rem;">
          <select
            bind:value={accessEventFilter}
            onchange={() => refreshAccess()}
          >
            <option value="all">Alle</option>
            <option value="grant">Nur Zutritt</option>
            <option value="deny">Nur Abgelehnt</option>
          </select>
          <input
            bind:value={accessEventEmployeeNo}
            placeholder="PN filtern (z.B. 0002)"
            onchange={() => refreshAccess()}
          />
        </div>
        <ul class="compact-list">
          {#each accessEvents as ev}
            <li>
              <span class="muted">{formatIsoLocalShort(ev.occurred_at)}</span>
              {ev.employee_name ?? '—'} ({ev.employee_no ?? '—'})
              — {ev.zone_name ?? '—'} / {ev.door_name ?? '—'}
              — <strong>{accessDecisionLabel(ev.decision)}</strong>
              ({reasonLabel(ev.reason_code ?? '')})
            </li>
          {/each}
        </ul>
      </div>
    {:else}
      <h2>System</h2>
      <div class="card" style="margin-bottom: 1rem;">
        <h3>API-Status</h3>
        {#if apiHealth}
          <p>
            {apiHealth.service} v{apiHealth.version} — <strong>{apiHealth.status}</strong>
            (Datenbank: {apiHealth.database})
          </p>
          {#if apiHealth.demo_seeding_enabled || apiHealth.default_password_login_blocked}
            <p class="error" style="margin-top: 0.5rem; font-size: 0.9rem;">
              {#if apiHealth.default_password_login_blocked}
                Standardpasswörter sind an der API gesperrt. Passwort unter „Passwort ändern“ unten
                setzen oder Admin-Zurücksetzen nutzen.
              {/if}
              {#if apiHealth.demo_seeding_enabled}
                Demo-Seeding aktiv (`demo`/`manager`). Produktion:
                <code>TIMESHARDS_DISABLE_DEMO=1</code>.
              {/if}
            </p>
          {/if}
        {:else}
          <p class="muted">Nicht erreichbar</p>
        {/if}
        <p class="muted" style="margin-top: 0.5rem;">Bind: {apiBind || '…'}</p>
        <p class="muted">DB: {dbPath || '…'}</p>
        {#if apiHealth?.hardware_adapter}
          <p class="muted">
            Hardware-Adapter: <code>{apiHealth.hardware_adapter}</code>
            {#if apiHealth.hardware_adapter_configured}
              <span class="muted">
                (konfiguriert: <code>{apiHealth.hardware_adapter_configured}</code> — Fallback aktiv)
              </span>
            {/if}
            {#if apiHealth.hardware_tcp_listen}
              — TCP <code>{apiHealth.hardware_tcp_listen}</code>
            {/if}
          </p>
        {/if}
        <button class="secondary" style="margin-top: 0.5rem;" onclick={refreshHealth}>
          Erneut prüfen
        </button>
        <p class="muted" style="margin-top: 0.75rem; font-size: 0.9rem;">
          OpenAPI:
          <a href="{apiUrl}/api/v1/openapi.json" target="_blank" rel="noopener noreferrer">
            {apiUrl}/api/v1/openapi.json
          </a>
          (Swagger UI: Import URL)
        </p>
      </div>
      <div class="card" style="margin-bottom: 1rem;">
        <h3>Demo-Zugänge</h3>
        <p class="muted" style="font-size: 0.9rem;">
          Client: <code>demo</code> / <code>demo</code> (PN 0002, Badge DEMO-0002) —
          <code>manager</code> / <code>demo</code> (Freigaben, DEMO-0003). Server: <code>admin</code> /
          <code>admin</code> (DEMO-ADMIN-001).
        </p>
      </div>
      {#if roles.length}
        <div class="card" style="margin-bottom: 1rem;">
          <h3>Rollen</h3>
          <ul class="compact-list">
            {#each roles as r}
              <li>{r.name}</li>
            {/each}
          </ul>
        </div>
      {/if}
      {#if sites.length}
        <div class="card" style="margin-bottom: 1rem;">
          <h3>Standorte</h3>
          <ul class="compact-list">
            {#each sites as s}
              <li>{s.name} — <span class="muted">{s.timezone}</span></li>
            {/each}
          </ul>
        </div>
      {/if}
      <div class="card" style="margin-bottom: 1rem;">
        <h3>Audit-Log</h3>
        <div class="grid-form" style="margin-bottom: 0.5rem;">
          <input bind:value={auditActorFilter} placeholder="Akteur (z.B. user, hardware)" />
          <input bind:value={auditObjectFilter} placeholder="Objekt (z.B. door, employee)" />
          <input bind:value={auditActionFilter} placeholder="Aktion (z.B. update, reader_offline)" />
          <input
            type="number"
            min="1"
            max="500"
            bind:value={auditLimit}
            placeholder="Limit"
            style="max-width: 6rem;"
          />
          <button class="secondary" onclick={refreshAudit}>Filtern</button>
        </div>
        <ul class="compact-list">
          {#each auditLog as a}
            <li>
              {formatIsoLocalShort(a.occurred_at)} — <span class="muted">{a.actor_type}</span>
              {a.action} {a.object_type}
              {#if a.object_id}<span class="muted"> #{a.object_id.slice(0, 8)}</span>{/if}
              {#if a.reason}<span class="muted"> ({a.reason})</span>{/if}
            </li>
          {:else}
            <li class="muted">Keine Einträge</li>
          {/each}
        </ul>
      </div>
      <div class="card">
        <h3>Passwort ändern</h3>
        <input type="password" bind:value={currentPassword} placeholder="Aktuelles Passwort" />
        <input type="password" bind:value={newPassword} placeholder="Neues Passwort (min. 6)" />
        <button style="margin-top: 0.5rem;" onclick={changePassword}>Speichern</button>
      </div>
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
    {#if success}<p class="success">{success}</p>{/if}
  </main>
</div>
