<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { api, downloadFile, setToken, getToken, type LoginResponse } from './lib/api';
  import {
    toLocalDatetimeInputValue,
    fromLocalDatetimeInputValue,
    formatIsoShort,
  } from './lib/datetime';
  import { formatMinutes } from './lib/formatMinutes';
  import { statusLabel } from './lib/statusLabels';
  import ClientApprovalsPillar from './components/ClientApprovalsPillar.svelte';
  import ClientTimePillar from './components/ClientTimePillar.svelte';

  type View = 'settings' | 'login' | 'app';
  type Pillar = 'time' | 'absence' | 'approvals' | 'access' | 'account';

  let view = $state<View>('settings');
  let pillar = $state<Pillar>('time');
  let serverUrl = $state('http://127.0.0.1:47821');
  let serverHealth = $state<{
    status: string;
    version: string;
    demo_seeding_enabled?: boolean;
    default_password_login_blocked?: boolean;
    hardware_adapter?: string;
    hardware_adapter_configured?: string | null;
    hardware_tcp_listen?: string | null;
  } | null>(null);
  let username = $state('demo');
  let password = $state('demo');
  let user = $state<LoginResponse['user'] | null>(null);
  let error = $state('');
  let success = $state('');

  let timePillar: ClientTimePillar | undefined = $state();
  let approvalsPillar: ClientApprovalsPillar | undefined = $state();
  let absences = $state<
    {
      id: string;
      employee_name: string;
      absence_type: string;
      starts_at: string;
      ends_at: string;
      status: string;
    }[]
  >([]);
  let accessSummary = $state<{
    badges: { credential_uid: string; status: string }[];
    recent_events: {
      decision: string;
      reason_code: string;
      zone_name?: string;
      occurred_at: string;
    }[];
    readers?: { id: string; label: string }[];
  } | null>(null);
  let clientSimReader = $state('sim.reader.main');
  let newAbsence = $state({
    absence_type: 'urlaub',
    starts_local: toLocalDatetimeInputValue(new Date()),
    ends_local: toLocalDatetimeInputValue(
      new Date(Date.now() + 7 * 24 * 60 * 60 * 1000),
    ),
    reason: '',
  });
  let absenceConflict = $state<string | null>(null);
  let absenceFilter = $state<'all' | 'pending' | 'approved' | 'rejected'>('all');
  let currentPassword = $state('');
  let newPassword = $state('');
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
    serverHealth = await api<{
      status: string;
      version: string;
      demo_seeding_enabled?: boolean;
      default_password_login_blocked?: boolean;
      hardware_adapter?: string;
      hardware_adapter_configured?: string | null;
      hardware_tcp_listen?: string | null;
    }>(serverUrl, '/api/v1/health').catch(() => null);
  }

  $effect(() => {
    if (view !== 'app' || !user) return;
    const p = pillar;
    if (p === 'time' || p === 'absence' || p === 'access' || p === 'account') void refreshAll();
    else if (p === 'approvals' && canApprove) void refreshAll();
  });

  $effect(() => {
    if (view !== 'app' || pillar !== 'absence') return;
    newAbsence.starts_local;
    newAbsence.ends_local;
    void checkAbsenceConflict();
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
    let absPath = '/api/v1/absences';
    if (absenceFilter !== 'all') absPath += `?status=${absenceFilter}`;
    absences = await api<typeof absences>(serverUrl, absPath).catch(() => []);
    accessSummary = await api<typeof accessSummary>(serverUrl, '/api/v1/access/me').catch(
      () => null,
    );
    workSummary = await api<typeof workSummary>(serverUrl, '/api/v1/me/work-summary').catch(
      () => null,
    );
    await Promise.all([timePillar?.refresh(), approvalsPillar?.refresh()]);
  }

  const approvalQueueCount = $derived(
    (workSummary?.pending_timesheets ?? 0) +
      (workSummary?.pending_absences ?? 0) +
      (canApprove ? (workSummary?.team_draft_timesheets ?? 0) : 0),
  );

  const ownDraftCount = $derived(workSummary?.draft_timesheets ?? 0);
  const ownPendingAbsences = $derived(workSummary?.my_pending_absences ?? 0);

  async function checkAbsenceConflict() {
    try {
      const starts = fromLocalDatetimeInputValue(newAbsence.starts_local);
      const ends = fromLocalDatetimeInputValue(newAbsence.ends_local);
      const r = await api<{ has_conflict: boolean; message?: string }>(
        serverUrl,
        `/api/v1/absences/conflicts?starts_at=${encodeURIComponent(starts)}&ends_at=${encodeURIComponent(ends)}`,
      );
      absenceConflict = r.has_conflict ? (r.message ?? 'Zeitraum kollidiert') : null;
    } catch {
      absenceConflict = null;
    }
  }

  async function createAbsence() {
    await api(serverUrl, '/api/v1/absences', {
      method: 'POST',
      body: JSON.stringify({
        absence_type: newAbsence.absence_type,
        starts_at: fromLocalDatetimeInputValue(newAbsence.starts_local),
        ends_at: fromLocalDatetimeInputValue(newAbsence.ends_local),
        reason: newAbsence.reason || null,
      }),
    });
    newAbsence.reason = '';
    await refreshAll();
    success = 'Abwesenheit beantragt';
  }

  async function cancelAbsence(id: string) {
    error = '';
    await api(serverUrl, `/api/v1/absences/${id}/cancel`, { method: 'POST' });
    await refreshAll();
    success = 'Antrag storniert';
  }

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

  async function changePassword() {
    error = '';
    success = '';
    try {
      await api(serverUrl, '/api/v1/auth/change-password', {
        method: 'POST',
        body: JSON.stringify({
          current_password: currentPassword,
          new_password: newPassword,
        }),
      });
      currentPassword = '';
      newPassword = '';
      success = 'Passwort geändert — bitte erneut anmelden';
      await logout();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function exportAccessLog() {
    error = '';
    try {
      await downloadFile(serverUrl, '/api/v1/reports/access/export?format=csv&limit=500', 'zutritt.csv');
      success = 'Zutritt-Protokoll exportiert';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function simulateMyBadge() {
    const uid = accessSummary?.badges.find((b) => b.status === 'active')?.credential_uid;
    if (!uid) {
      error = 'Kein aktives Badge';
      return;
    }
    error = '';
    try {
      const res = await api<{ decision: string; reason_code: string }>(
        serverUrl,
        '/api/v1/access/me/simulate-scan',
        {
          method: 'POST',
          body: JSON.stringify({ credential_uid: uid, reader_id: clientSimReader }),
        },
      );
      await refreshAll();
      success = `${accessDecisionLabel(res.decision)} (${accessReasonLabel(res.reason_code)})`;
      if (res.decision === 'grant' || res.decision === 'allow') {
        clientSimReader =
          clientSimReader === 'sim.reader.main' ? 'sim.reader.main.out' : 'sim.reader.main';
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function accessReasonLabel(code: string) {
    const map: Record<string, string> = {
      ok: 'OK',
      no_permission: 'Keine Berechtigung',
      antipassback: 'Anti-Passback',
      schedule_restricted: 'Außerhalb Zeitplan',
      unknown_badge: 'Unbekannte Karte',
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

</script>

{#if view === 'settings'}
  <div class="content" style="max-width: 420px; margin: 4rem auto;">
    <h2>Server-Verbindung</h2>
    <input bind:value={serverUrl} onchange={() => checkServerHealth()} />
    {#if serverHealth}
      <p class="muted" style="margin-top: 0.5rem;">
        API v{serverHealth.version} — {serverHealth.status === 'ok' ? 'erreichbar' : serverHealth.status}
      </p>
      {#if serverHealth.default_password_login_blocked}
        <p class="muted" style="margin-top: 0.5rem; font-size: 0.85rem;">
          Standardpasswörter werden abgelehnt — nur geänderte Zugangsdaten nutzen.
        </p>
      {/if}
      {#if serverHealth.hardware_adapter}
        <p class="muted" style="margin-top: 0.25rem; font-size: 0.85rem;">
          Hardware-Adapter: <code>{serverHealth.hardware_adapter}</code>
          {#if serverHealth.hardware_adapter_configured}
            (konfiguriert: <code>{serverHealth.hardware_adapter_configured}</code>)
          {/if}
          {#if serverHealth.hardware_tcp_listen}
            — TCP <code>{serverHealth.hardware_tcp_listen}</code>
          {/if}
        </p>
      {/if}
    {:else}
      <p class="error" style="margin-top: 0.5rem;">Server nicht erreichbar</p>
    {/if}
    <button style="margin-top: 1rem;" onclick={saveSettings}>Speichern & weiter</button>
  </div>
{:else if view === 'login'}
  <div class="content" style="max-width: 420px; margin: 4rem auto;">
    <h2>Anmelden</h2>
    <p class="muted">{serverUrl}</p>
    {#if serverHealth}
      <p class="muted" style="font-size: 0.85rem;">
        API {serverHealth.status === 'ok' ? 'bereit' : serverHealth.status} (v{serverHealth.version})
      </p>
      {#if serverHealth.default_password_login_blocked}
        <p class="muted" style="font-size: 0.85rem; margin-top: 0.35rem;">
          Standardpasswörter gesperrt — nur geänderte Zugangsdaten.
        </p>
      {/if}
    {:else}
      <p class="error" style="font-size: 0.85rem;">API nicht erreichbar - URL prüfen</p>
    {/if}
    <input
      bind:value={username}
      placeholder="Benutzername"
      onkeydown={(e) => e.key === 'Enter' && login()}
    />
    <input
      type="password"
      bind:value={password}
      placeholder="Passwort"
      style="margin-top: 0.5rem;"
      onkeydown={(e) => e.key === 'Enter' && login()}
    />
    {#if serverHealth?.demo_seeding_enabled !== false && !serverHealth?.default_password_login_blocked}
      <div class="btn-row" style="margin-top: 0.75rem;">
        <button class="secondary" onclick={() => { username = 'demo'; password = 'demo'; }}>demo</button>
        <button class="secondary" onclick={() => { username = 'manager'; password = 'demo'; }}>manager</button>
        <button class="secondary" onclick={() => { username = 'admin'; password = 'admin'; }}>admin</button>
      </div>
      <p class="muted" style="margin-top: 0.75rem; font-size: 0.85rem;">
        Demo: <code>demo</code>/<code>demo</code> (Zeit, DEMO-0002) ·
        <code>manager</code>/<code>demo</code> (Freigaben, Zutritt DEMO-0003)
      </p>
    {/if}
    <button style="margin-top: 1rem;" onclick={login}>Anmelden</button>
    <button class="secondary" style="margin-top: 0.5rem;" onclick={() => (view = 'settings')}>
      Server ändern
    </button>
    {#if error}<p class="error">{error}</p>{/if}
  </div>
{:else}
  <div class="layout">
    <aside class="sidebar">
      <h1>TimeShards</h1>
      <p class="muted" style="font-size: 0.8rem;">
        {user?.display_name}
        {#if user?.employee_no}
          <br />PN {user.employee_no}
        {/if}
      </p>
      {#if workSummary}
        <p class="muted" style="font-size: 0.75rem; margin-top: 0.35rem;">
          {#if workSummary.is_on_break}
            Pause
          {:else if workSummary.is_clocked_in}
            Eingestempelt
          {:else}
            Ausgestempelt
          {/if}
          {#if workSummary.flex_balance_minutes != null}
            <br />Gleitzeit: {formatMinutes(workSummary.flex_balance_minutes)}
          {/if}
          {#if workSummary.work_calendar_assigned === false}
            <br /><span class="error">Kein Arbeitskalender</span>
          {:else if workSummary.current_week && workSummary.current_week.expected_minutes > 0}
            <br />KW: Ist {formatMinutes(workSummary.current_week.worked_minutes)} · Soll{' '}
            {formatMinutes(workSummary.current_week.expected_minutes)} · Saldo{' '}
            {formatMinutes(workSummary.current_week.balance_minutes)}
          {/if}
        </p>
      {/if}
      <nav class="nav">
        {#if canApprove}
          <button class:active={pillar === 'approvals'} onclick={() => (pillar = 'approvals')}>
            Freigaben
            {#if approvalQueueCount > 0}
              <span class="nav-badge">{approvalQueueCount}</span>
            {/if}
          </button>
        {/if}
        <button class:active={pillar === 'time'} onclick={() => (pillar = 'time')}>
          Zeit
          {#if ownDraftCount > 0}
            <span class="nav-badge draft">{ownDraftCount}</span>
          {/if}
        </button>
        <button class:active={pillar === 'absence'} onclick={() => (pillar = 'absence')}>
          Abwesenheit
          {#if ownPendingAbsences > 0}
            <span class="nav-badge">{ownPendingAbsences}</span>
          {/if}
        </button>
        <button class:active={pillar === 'access'} onclick={() => (pillar = 'access')}>
          Zutritt
        </button>
        <button class:active={pillar === 'account'} onclick={() => (pillar = 'account')}>
          Konto
        </button>
      </nav>
      <button class="secondary" style="margin-top: 1rem;" onclick={refreshAll}>Aktualisieren</button>
      <button class="secondary" style="margin-top: 0.5rem;" onclick={logout}>Abmelden</button>
    </aside>

    <main class="content">
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
        <h2>Abwesenheit</h2>
        <div class="card">
          <div class="grid-form">
            <select bind:value={newAbsence.absence_type}>
              <option value="urlaub">Urlaub</option>
              <option value="krank">Krank</option>
              <option value="sonder">Sonderurlaub</option>
              <option value="unbezahlt">Unbezahlt</option>
            </select>
            <input type="datetime-local" bind:value={newAbsence.starts_local} />
            <input type="datetime-local" bind:value={newAbsence.ends_local} />
            <input bind:value={newAbsence.reason} placeholder="Grund (optional)" />
            <button onclick={createAbsence} disabled={!!absenceConflict}>Beantragen</button>
            {#if absenceConflict}<p class="error">{absenceConflict}</p>{/if}
          </div>
        </div>
        <div class="card" style="margin-top: 1rem;">
          <select
            bind:value={absenceFilter}
            onchange={() => refreshAll()}
            style="margin-bottom: 0.5rem; max-width: 240px;"
          >
            <option value="all">Alle Anträge</option>
            <option value="pending">Offen</option>
            <option value="approved">Freigegeben</option>
            <option value="rejected">Abgelehnt</option>
          </select>
          <ul>
            {#each absences as a}
              <li class="row-card">
                {statusLabel(a.absence_type)} · {formatIsoShort(a.starts_at)} → {formatIsoShort(a.ends_at)} ·
                {statusLabel(a.status)}
                {#if a.status === 'pending' || a.status === 'approved'}
                  <button class="secondary" onclick={() => cancelAbsence(a.id)}>Stornieren</button>
                {/if}
              </li>
            {/each}
          </ul>
        </div>
      {:else if pillar === 'account'}
        <h2>Konto</h2>
        <div class="card">
          <p>
            {user?.display_name} (<code>{user?.username}</code>)
            {#if user?.employee_no}
              <br /><span class="muted">Personalnr. {user.employee_no}</span>
            {/if}
          </p>
          <h3>Server</h3>
          <input bind:value={serverUrl} placeholder="http://127.0.0.1:47821" />
          <button class="secondary" style="margin-top: 0.5rem;" onclick={saveSettings}>
            Server-URL speichern
          </button>
          <p class="muted" style="margin-top: 0.5rem;">
            Nach Änderung der URL bitte neu anmelden.
          </p>
        </div>
        {#if workSummary}
          <div class="card" style="margin-top: 1rem;">
            <h3>Status</h3>
            <p class="muted">
              {#if workSummary.is_on_break}
                Pause aktiv
              {:else if workSummary.is_clocked_in}
                Eingestempelt
              {:else}
                Ausgestempelt
              {/if}
            </p>
            <ul class="compact-list muted">
              {#if (workSummary.draft_timesheets ?? 0) > 0}
                <li>{workSummary.draft_timesheets} Stundenzettel-Entwurf(e)</li>
              {/if}
              {#if canApprove && (workSummary.pending_timesheets ?? 0) > 0}
                <li>{workSummary.pending_timesheets} Stundenzettel zur Freigabe</li>
              {/if}
              {#if canApprove && (workSummary.pending_absences ?? 0) > 0}
                <li>{workSummary.pending_absences} Abwesenheit(en) zur Freigabe</li>
              {/if}
              {#if canApprove && (workSummary.team_draft_timesheets ?? 0) > 0}
                <li>{workSummary.team_draft_timesheets} Team-Entwürfe</li>
              {/if}
              {#if ownPendingAbsences > 0}
                <li>{ownPendingAbsences} eigene Abwesenheit(en) offen</li>
              {/if}
            </ul>
          </div>
        {/if}
        <div class="card" style="margin-top: 1rem;">
          <h3>Passwort ändern</h3>
          <div class="grid-form">
            <input type="password" bind:value={currentPassword} placeholder="Aktuelles Passwort" />
            <input type="password" bind:value={newPassword} placeholder="Neues Passwort (min. 6)" />
            <button onclick={changePassword}>Speichern</button>
          </div>
          <p class="muted" style="margin-top: 0.75rem;">
            Nach dem Speichern werden Sie abgemeldet.
          </p>
        </div>
      {:else}
        <h2>Zutritt</h2>
        {#if accessSummary}
          <div class="card">
            <h3>Meine Karte</h3>
            {#each accessSummary.badges as b}
              <p>
                <code>{b.credential_uid}</code>
                <button
                  class="secondary"
                  onclick={() =>
                    navigator.clipboard.writeText(b.credential_uid).then(() => {
                      success = 'UID kopiert';
                    })}
                >
                  Kopieren
                </button>
                — {b.status}
              </p>
            {:else}
              <p class="muted">Kein Badge zugewiesen</p>
            {/each}
            {#if accessSummary.badges.some((b) => b.status === 'active')}
              <p class="muted" style="margin-top: 0.5rem;">
                Demo: zweiter Scan am Eingang ohne Ausgang löst Anti-Passback aus. Nach erfolgreichem Scan
                wechselt der Reader automatisch (Eingang ↔ Ausgang).
              </p>
              <select bind:value={clientSimReader} style="margin-top: 0.5rem;">
                {#if accessSummary.readers?.length}
                  {#each accessSummary.readers as r}
                    <option value={r.id}>{r.label}</option>
                  {/each}
                {:else}
                  <option value="sim.reader.main">Eingang</option>
                  <option value="sim.reader.main.out">Ausgang</option>
                {/if}
              </select>
              <button style="margin-top: 0.5rem;" onclick={simulateMyBadge}>
                Scan simulieren
              </button>
            {/if}
            {#if canExportAccess}
              <button class="secondary" style="margin-top: 0.5rem;" onclick={exportAccessLog}>
                Zutritt-Protokoll (CSV)
              </button>
            {/if}
          </div>
          <div class="card" style="margin-top: 1rem;">
            <h3>Letzte Zutritte</h3>
            <ul>
              {#each accessSummary.recent_events as ev}
                <li>
                  {accessDecisionLabel(ev.decision)} — {accessReasonLabel(ev.reason_code)}
                  {#if ev.zone_name}<span class="muted"> ({ev.zone_name})</span>{/if}
                  — {formatIsoShort(ev.occurred_at)}
                </li>
              {:else}
                <li class="muted">Noch keine Ereignisse</li>
              {/each}
            </ul>
          </div>
        {/if}
      {/if}
      {#if error}<p class="error">{error}</p>{/if}
      {#if success}<p class="success">{success}</p>{/if}
    </main>
  </div>
{/if}
