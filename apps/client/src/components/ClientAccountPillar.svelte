<script lang="ts">
  import { api, setToken } from '../lib/api';
  import { formatMinutes } from '../lib/formatMinutes';
  import type { LoginResponse } from '../lib/api';
  import type { ServerHealth } from '../lib/serverHealth';

  type WorkSummary = {
    pending_timesheets: number | null;
    pending_absences: number | null;
    my_pending_absences: number | null;
    draft_timesheets: number | null;
    team_draft_timesheets: number | null;
    is_clocked_in: boolean;
    is_on_break: boolean;
    flex_balance_minutes?: number | null;
    work_calendar_assigned?: boolean | null;
    current_week?: {
      worked_minutes: number;
      expected_minutes: number;
      balance_minutes: number;
    } | null;
  };

  type UiMessage = { type: 'error' | 'success'; text: string };

  let {
    serverUrl = $bindable(''),
    user,
    workSummary = null,
    canApprove = false,
    ownPendingAbsences = 0,
    serverHealth = null,
    onMessage,
    onLogout,
    onServerUrlSaved,
  }: {
    serverUrl?: string;
    user: LoginResponse['user'] | null;
    workSummary?: WorkSummary | null;
    canApprove?: boolean;
    ownPendingAbsences?: number;
    serverHealth?: ServerHealth | null;
    onMessage?: (msg: UiMessage) => void;
    onLogout?: () => void | Promise<void>;
    onServerUrlSaved?: () => void;
  } = $props();

  let currentPassword = $state('');
  let newPassword = $state('');

  const productionHint = $derived(
    serverHealth?.demo_seeding_enabled !== false &&
      !serverHealth?.default_password_login_blocked,
  );

  function notify(type: 'error' | 'success', text: string) {
    onMessage?.({ type, text });
  }

  async function saveServerUrl() {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('save_client_settings', { settings: { server_url: serverUrl } });
    onServerUrlSaved?.();
    notify('success', 'Server-URL gespeichert — bitte neu anmelden');
  }

  async function changePassword() {
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
      notify('success', 'Passwort geändert — bitte erneut anmelden');
      setToken(null);
      await onLogout?.();
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }
</script>

<h2>Konto</h2>
{#if productionHint}
  <p class="muted" style="font-size: 0.85rem; margin-bottom: 0.75rem;">
    Demo-Modus aktiv — für Produktion Server mit <code>TIMESHARDS_DISABLE_DEMO=1</code> betreiben.
  </p>
{/if}
<div class="card">
  <p>
    {user?.display_name} (<code>{user?.username}</code>)
    {#if user?.employee_no}
      <br /><span class="muted">Personalnr. {user.employee_no}</span>
    {/if}
  </p>
  <h3>Server</h3>
  <input bind:value={serverUrl} placeholder="http://127.0.0.1:47821" />
  <button class="secondary" type="button" style="margin-top: 0.5rem;" onclick={saveServerUrl}>
    Server-URL speichern
  </button>
  <p class="muted" style="margin-top: 0.5rem;">Nach Änderung der URL bitte neu anmelden.</p>
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
      {#if workSummary.flex_balance_minutes != null}
        <br />Gleitzeit: {formatMinutes(workSummary.flex_balance_minutes)}
      {/if}
      {#if workSummary.work_calendar_assigned === false}
        <br /><span class="error">Kein Arbeitskalender — Admin kontaktieren</span>
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
    <button type="button" onclick={changePassword}>Speichern</button>
  </div>
  <p class="muted" style="margin-top: 0.75rem;">Nach dem Speichern werden Sie abgemeldet.</p>
</div>
