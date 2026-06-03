<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { api, setToken, getToken, type LoginResponse } from './lib/api';
  import { anchorFromPeriodStart } from './lib/datetime';
  import { doorStatusLabel } from './lib/accessLabels';
  import WorkCalendarCard from './components/WorkCalendarCard.svelte';
  import ShiftWeekCard from './components/ShiftWeekCard.svelte';
  import TimesheetsCard from './components/TimesheetsCard.svelte';
  import OverviewTab, { type OverviewNavigate } from './components/OverviewTab.svelte';
  import PersonnelTab from './components/PersonnelTab.svelte';
  import AbsenceTab from './components/AbsenceTab.svelte';
  import AccessTab from './components/AccessTab.svelte';
  import SystemTab from './components/SystemTab.svelte';

  type Tab = 'overview' | 'personnel' | 'time' | 'absence' | 'access' | 'system';
  type TimeSection = 'stammdaten' | 'planung' | 'stundenzettel' | 'abschluss';

  function handleUiMessage(msg: { type: 'error' | 'success'; text: string }) {
    if (msg.type === 'error') error = msg.text;
    else success = msg.text;
  }
  let tab = $state<Tab>('overview');
  let timeSection = $state<TimeSection>('stammdaten');
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
    time_foundation?: {
      workday_models: number;
      work_calendars: number;
      active_employees: number;
      employees_without_work_calendar: number;
      current_week_drafts_without_soll: number;
    };
  } | null>(null);
  let username = $state('admin');
  let password = $state('admin');
  let user = $state<LoginResponse['user'] | null>(null);
  let error = $state('');
  let success = $state('');

  let absenceFilter = $state<'all' | 'pending' | 'approved' | 'rejected'>('all');
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
  let shiftWeekAnchor = $state(new Date());
  let timesheetFilter = $state<'all' | 'pending' | 'draft' | 'approved' | 'rejected'>('all');
  let shiftFilter = $state<'all' | 'planned' | 'published' | 'cancelled'>('all');
  let workCalendarCard: WorkCalendarCard | undefined = $state();
  let shiftWeekCard: ShiftWeekCard | undefined = $state();
  let timesheetsCard: TimesheetsCard | undefined = $state();
  let personnelTab: PersonnelTab | undefined = $state();
  let absenceTab: AbsenceTab | undefined = $state();
  let accessTab: AccessTab | undefined = $state();
  let systemTab: SystemTab | undefined = $state();
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
    time_access_mismatch_count?: number;
    time_access_mismatches?: {
      employee_id: string;
      employee_no: string;
      display_name: string;
      kind: 'clocked_not_inside' | 'inside_not_clocked';
    }[];
  } | null>(null);

  function timeAccessMismatchLabel(kind: string): string {
    if (kind === 'clocked_not_inside') return 'eingestempelt, nicht im Gebäude';
    if (kind === 'inside_not_clocked') return 'im Gebäude, nicht eingestempelt';
    return kind;
  }

  let zoneOccupancy = $state<
    { zone_name: string; inside_count: number; occupants: { display_name: string }[] }[]
  >([]);
  const canApprove = $derived(
    user?.roles.some((r) =>
      ['system_admin', 'hr_admin', 'manager'].includes(r),
    ) ?? false,
  );

  const canCorrectTime = $derived(
    user?.roles.some((r) => ['system_admin', 'hr_admin', 'manager'].includes(r)) ?? false,
  );

  $effect(() => {
    loadServerInfo();
  });

  $effect(() => {
    if (!user) return;
    const t = tab;
    error = '';
    if (t === 'overview') {
      void refreshDashboard();
      void refreshClockedIn();
    } else if (t === 'personnel') {
      void personnelTab?.refresh();
    } else if (t === 'time') void refreshTime();
    else if (t === 'absence') void absenceTab?.refresh();
    else if (t === 'access') {
      void accessTab?.refresh();
      void refreshDashboard();
    } else if (t === 'system') void systemTab?.refresh();
  });

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
      accessTab?.refresh() ?? Promise.resolve(),
      refreshPersonnel(),
      refreshTime(),
      refreshAbsences(),
      refreshDashboard(),
      systemTab?.refresh() ?? Promise.resolve(),
    ]);
  }

  async function refreshAbsences() {
    await absenceTab?.refresh();
  }

  function handleOverviewNavigate(target: OverviewNavigate) {
    tab = target.tab;
    if (target.timesheetFilter) timesheetFilter = target.timesheetFilter;
    if (target.absenceFilter) absenceFilter = target.absenceFilter;
    if (target.shiftFilter) shiftFilter = target.shiftFilter;
    if (target.timeSection) timeSection = target.timeSection;
    if (target.tab === 'time') void refreshTime();
    if (target.tab === 'absence') void absenceTab?.refresh();
    if (target.tab === 'personnel') {
      void refreshPersonnel();
      if (target.personnelNoCalendar) personnelTab?.focusNoCalendar();
    }
  }

  function onPersonnelData(emps: typeof employees) {
    employees = emps;
  }

  async function refreshClockedIn() {
    if (!getToken()) return;
    clockedIn = await api<typeof clockedIn>(apiUrl, '/api/v1/time/clocked-in').catch(() => []);
  }

  async function refreshAccess() {
    await accessTab?.refresh();
  }

  async function setDoorStatus(doorId: string, status: string) {
    await accessTab?.setDoorStatus(doorId, status);
  }

  function copyApiUrl(url: string) {
    navigator.clipboard.writeText(url).then(() => {
      success = 'URL kopiert';
    });
  }

  async function refreshPersonnel() {
    if (personnelTab) {
      await personnelTab.refresh();
      return;
    }
    employees = await api(apiUrl, '/api/v1/admin/employees');
  }

  function refreshShiftAndTimesheetPanels() {
    void shiftWeekCard?.refresh();
    void timesheetsCard?.refresh();
  }

  async function refreshTimeSection() {
    switch (timeSection) {
      case 'stammdaten':
        await workCalendarCard?.refresh();
        break;
      case 'planung':
        await shiftWeekCard?.refresh();
        break;
      case 'stundenzettel':
      case 'abschluss':
        await timesheetsCard?.refresh();
        break;
    }
  }

  function selectTimeSection(section: TimeSection) {
    timeSection = section;
    void refreshTimeSection();
  }

  async function refreshTime() {
    await Promise.all([
      workCalendarCard?.refresh(),
      shiftWeekCard?.refresh(),
      timesheetsCard?.refresh(),
    ]);
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
      <OverviewTab
        {apiBind}
        {apiUrls}
        {dbPath}
        {apiHealth}
        {dashboard}
        {zoneOccupancy}
        {clockedIn}
        {user}
        bind:username
        bind:password
        onRefreshAll={refreshAll}
        onRefreshHealth={refreshHealth}
        onCopyApiUrl={copyApiUrl}
        onLogin={login}
        onNavigate={handleOverviewNavigate}
        onFoundationFix={foundationFix}
        onSetDoorStatus={setDoorStatus}
        {timeAccessMismatchLabel}
      />
    {:else if tab === 'personnel'}
      <PersonnelTab
        bind:this={personnelTab}
        {apiUrl}
        {user}
        active={tab === 'personnel'}
        onMessage={handleUiMessage}
        onDataChange={onPersonnelData}
      />
    {:else if tab === 'time'}
      <h2>Zeit</h2>
      <p class="muted" style="margin-top: 0; margin-bottom: 0.75rem;">
        Sollzeit aus Arbeitskalender · Schichten nur Planung · Freigabe bucht auf Zeitkonten
      </p>
      <nav class="sub-nav" aria-label="Zeit-Bereiche">
        <button
          type="button"
          class:active={timeSection === 'stammdaten'}
          onclick={() => selectTimeSection('stammdaten')}>Arbeitskalender</button>
        <button
          type="button"
          class:active={timeSection === 'planung'}
          onclick={() => selectTimeSection('planung')}>Schichtplanung</button>
        <button
          type="button"
          class:active={timeSection === 'stundenzettel'}
          onclick={() => selectTimeSection('stundenzettel')}>
          Stundenzettel
          {#if dashboard && dashboard.pending_timesheets > 0}
            <span class="nav-badge">{dashboard.pending_timesheets}</span>
          {/if}
        </button>
        <button
          type="button"
          class:active={timeSection === 'abschluss'}
          onclick={() => selectTimeSection('abschluss')}>Abschluss & Export</button>
      </nav>
      {#if timeSection === 'stammdaten'}
        <WorkCalendarCard
          bind:this={workCalendarCard}
          {apiUrl}
          bind:shiftWeekAnchor
          {employees}
          active={tab === 'time'}
          onMessage={handleUiMessage}
          onWeekChange={refreshShiftAndTimesheetPanels}
        />
      {:else if timeSection === 'planung'}
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
      {:else}
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
          settlementOnly={timeSection === 'abschluss'}
        />
      {/if}
    {:else if tab === 'absence'}
      <AbsenceTab
        bind:this={absenceTab}
        {apiUrl}
        {employees}
        {canApprove}
        pendingAbsences={dashboard?.pending_absences ?? 0}
        bind:absenceFilter
        active={tab === 'absence'}
        onMessage={handleUiMessage}
        onDashboardChange={refreshDashboard}
      />
    {:else if tab === 'access'}
      <AccessTab
        bind:this={accessTab}
        {apiUrl}
        {employees}
        {apiHealth}
        active={tab === 'access'}
        onMessage={handleUiMessage}
        onDashboardChange={refreshDashboard}
        onOccupancyChange={(occ) => (zoneOccupancy = occ)}
      />
    {:else}
      <SystemTab
        bind:this={systemTab}
        {apiUrl}
        {apiBind}
        {dbPath}
        {apiHealth}
        active={tab === 'system'}
        onMessage={handleUiMessage}
        onRefreshHealth={refreshHealth}
      />
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
    {#if success}<p class="success">{success}</p>{/if}
  </main>
</div>
