<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { api, setToken, getToken, type LoginResponse } from './lib/api';
  import type { ServerHealth } from './lib/serverHealth';
  import ClientAbsencePillar from './components/ClientAbsencePillar.svelte';
  import ClientAccessPillar from './components/ClientAccessPillar.svelte';
  import ClientAccountPillar from './components/ClientAccountPillar.svelte';
  import ClientApprovalsPillar from './components/ClientApprovalsPillar.svelte';
  import ClientTimePillar from './components/ClientTimePillar.svelte';
  import ClientAppShell from './components/ClientAppShell.svelte';
  import ClientLoginView from './components/ClientLoginView.svelte';
  import ClientSettingsView from './components/ClientSettingsView.svelte';

  type View = 'settings' | 'login' | 'app';
  type Pillar = 'time' | 'absence' | 'approvals' | 'access' | 'account';

  let view = $state<View>('settings');
  let pillar = $state<Pillar>('time');
  let serverUrl = $state('http://127.0.0.1:47821');
  let serverHealth = $state<ServerHealth | null>(null);
  let username = $state('demo');
  let password = $state('demo');
  let user = $state<LoginResponse['user'] | null>(null);
  let error = $state('');
  let success = $state('');

  let timePillar: ClientTimePillar | undefined = $state();
  let approvalsPillar: ClientApprovalsPillar | undefined = $state();
  let absencePillar: ClientAbsencePillar | undefined = $state();
  let accessPillar: ClientAccessPillar | undefined = $state();
  let workSummary = $state<{
    pending_timesheets: number | null;
    pending_absences: number | null;
    my_pending_absences: number | null;
    draft_timesheets: number | null;
    team_draft_timesheets: number | null;
    employee_id: string | null;
    employee_no: string | null;
    is_clocked_in: boolean;
    is_on_break: boolean;
    flex_balance_minutes?: number | null;
    work_calendar_assigned?: boolean | null;
    current_week?: {
      period_start: string;
      status: string;
      worked_minutes: number;
      expected_minutes: number;
      balance_minutes: number;
      work_calendar_name?: string | null;
    } | null;
  } | null>(null);
  const canApprove = $derived(
    user?.roles.some((r) => ['system_admin', 'hr_admin', 'manager'].includes(r)) ?? false,
  );
  const canExportAccess = $derived(
    user?.permissions.includes('report:export') ?? false,
  );

  function handleClientMessage(msg: { type: 'error' | 'success'; text: string }) {
    if (msg.type === 'error') error = msg.text;
    else success = msg.text;
  }

  $effect(() => {
    loadSettings();
  });

  $effect(() => {
    if (view === 'login' || view === 'settings') void checkServerHealth();
  });

  async function checkServerHealth() {
    serverHealth = await api<ServerHealth>(serverUrl, '/api/v1/health').catch(() => null);
  }

  $effect(() => {
    if (view !== 'app' || !user) return;
    const p = pillar;
    if (p === 'time' || p === 'absence' || p === 'access' || p === 'account') void refreshAll();
    else if (p === 'approvals' && canApprove) void refreshAll();
  });

  async function loadSettings() {
    try {
      const s = await invoke<{ server_url: string }>('get_client_settings');
      serverUrl = s.server_url;
      if (getToken()) {
        view = 'app';
        await refreshAll();
      }
    } catch {
      /* dev in browser */
    }
  }

  async function saveSettings() {
    await invoke('save_client_settings', { settings: { server_url: serverUrl } });
    view = 'login';
  }

  async function login() {
    error = '';
    success = '';
    try {
      const res = await api<LoginResponse>(serverUrl, '/api/v1/auth/login', {
        method: 'POST',
        body: JSON.stringify({ username, password }),
      });
      setToken(res.token);
      user = res.user;
      view = 'app';
      await refreshAll();
      const queue =
        (workSummary?.pending_timesheets ?? 0) +
        (workSummary?.pending_absences ?? 0) +
        (workSummary?.team_draft_timesheets ?? 0);
      pillar = canApprove && queue > 0 ? 'approvals' : 'time';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function refreshAll() {
    workSummary = await api<typeof workSummary>(serverUrl, '/api/v1/me/work-summary').catch(
      () => null,
    );
    await Promise.all([
      timePillar?.refresh(),
      approvalsPillar?.refresh(),
      absencePillar?.refresh(),
      accessPillar?.refresh(),
    ]);
  }

  const approvalQueueCount = $derived(
    (workSummary?.pending_timesheets ?? 0) +
      (workSummary?.pending_absences ?? 0) +
      (canApprove ? (workSummary?.team_draft_timesheets ?? 0) : 0),
  );

  const ownDraftCount = $derived(workSummary?.draft_timesheets ?? 0);
  const ownPendingAbsences = $derived(workSummary?.my_pending_absences ?? 0);

  async function logout() {
    error = '';
    try {
      if (getToken()) {
        await api(serverUrl, '/api/v1/auth/logout', { method: 'POST' });
      }
    } catch {
      /* offline or session already gone */
    }
    setToken(null);
    user = null;
    view = 'login';
  }
</script>

{#if view === 'settings'}
  <ClientSettingsView
    bind:serverUrl
    {serverHealth}
    onCheckHealth={checkServerHealth}
    onSave={saveSettings}
  />
{:else if view === 'login'}
  <ClientLoginView
    {serverUrl}
    {serverHealth}
    bind:username
    bind:password
    {error}
    onLogin={login}
    onOpenSettings={() => (view = 'settings')}
  />
{:else}
  <ClientAppShell
    {user}
    {workSummary}
    bind:pillar
    {canApprove}
    {approvalQueueCount}
    {ownDraftCount}
    {ownPendingAbsences}
    onRefresh={refreshAll}
    onLogout={logout}
  >
    {#if pillar === 'approvals' && canApprove}
      <ClientApprovalsPillar
        bind:this={approvalsPillar}
        {serverUrl}
        active={pillar === 'approvals'}
        myEmployeeId={workSummary?.employee_id ?? null}
        teamDraftCount={workSummary?.team_draft_timesheets ?? 0}
        onMessage={handleClientMessage}
        onRefreshParent={refreshAll}
      />
    {:else if pillar === 'time'}
      <ClientTimePillar
        bind:this={timePillar}
        {serverUrl}
        active={pillar === 'time'}
        {workSummary}
        {canApprove}
        {ownPendingAbsences}
        onMessage={handleClientMessage}
        onNavigate={(p) => (pillar = p)}
        onRefreshParent={refreshAll}
      />
    {:else if pillar === 'absence'}
      <ClientAbsencePillar
        bind:this={absencePillar}
        {serverUrl}
        active={pillar === 'absence'}
        onMessage={handleClientMessage}
        onRefreshParent={refreshAll}
      />
    {:else if pillar === 'access'}
      <ClientAccessPillar
        bind:this={accessPillar}
        {serverUrl}
        active={pillar === 'access'}
        {canExportAccess}
        onMessage={handleClientMessage}
        onRefreshParent={refreshAll}
      />
    {:else if pillar === 'account'}
      <ClientAccountPillar
        bind:serverUrl
        {user}
        {workSummary}
        {canApprove}
        {ownPendingAbsences}
        {serverHealth}
        onMessage={handleClientMessage}
        onLogout={logout}
        onServerUrlSaved={() => (view = 'login')}
      />
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
    {#if success}<p class="success">{success}</p>{/if}
  </ClientAppShell>
{/if}
