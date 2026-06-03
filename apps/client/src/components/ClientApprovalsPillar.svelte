<script lang="ts">
  import { api } from '../lib/api';
  import { formatMinutes } from '../lib/formatMinutes';
  import { statusLabel } from '../lib/statusLabels';
  import { formatIsoShort, formatPeriodRange, calendarWeekLabel } from '../lib/datetime';
  import TsPageHeader from '@timeshards/shared/ui/TsPageHeader.svelte';
  import TsEmptyState from '@timeshards/shared/ui/TsEmptyState.svelte';

  type TimesheetRow = {
    id: string;
    employee_id?: string;
    employee_name?: string;
    worked_minutes: number;
    overtime_minutes: number;
    status: string;
    period_start?: string;
    period_end?: string;
    rejection_reason?: string;
  };

  type AbsenceRow = {
    id: string;
    employee_name: string;
    absence_type: string;
    starts_at: string;
    ends_at: string;
    status: string;
  };

  type UiMessage = { type: 'error' | 'success'; text: string };

  let {
    serverUrl,
    active = false,
    myEmployeeId = null,
    teamDraftCount = 0,
    onMessage,
    onRefreshParent,
  }: {
    serverUrl: string;
    active?: boolean;
    myEmployeeId?: string | null;
    teamDraftCount?: number;
    onMessage?: (msg: UiMessage) => void;
    onRefreshParent?: () => void | Promise<void>;
  } = $props();

  let decisionNote = $state('');
  let pendingTimesheets = $state<TimesheetRow[]>([]);
  let pendingAbsences = $state<AbsenceRow[]>([]);
  let teamDraftRows = $state<TimesheetRow[]>([]);

  const teamDraftQueueCount = $derived(
    teamDraftRows.length > 0 ? teamDraftRows.length : teamDraftCount,
  );

  function notify(type: 'error' | 'success', text: string) {
    onMessage?.({ type, text });
  }

  export async function refresh() {
    const [pendingTs, pendingAbs, drafts, rejected] = await Promise.all([
      api<TimesheetRow[]>(serverUrl, '/api/v1/time/timesheets?status=pending').catch(() => []),
      api<AbsenceRow[]>(serverUrl, '/api/v1/absences?status=pending').catch(() => []),
      api<TimesheetRow[]>(serverUrl, '/api/v1/time/timesheets?status=draft').catch(() => []),
      api<TimesheetRow[]>(serverUrl, '/api/v1/time/timesheets?status=rejected').catch(() => []),
    ]);
    pendingTimesheets = pendingTs;
    pendingAbsences = pendingAbs;
    const seen = new Set<string>();
    teamDraftRows = [...drafts, ...rejected].filter((t) => {
      if (myEmployeeId && t.employee_id === myEmployeeId) return false;
      if (seen.has(t.id)) return false;
      seen.add(t.id);
      return true;
    });
  }

  async function afterAction(msg: string) {
    await refresh();
    await onRefreshParent?.();
    notify('success', msg);
  }

  async function timesheetAction(id: string, action: 'approve' | 'reject') {
    try {
      if (action === 'reject') {
        await api(serverUrl, `/api/v1/time/timesheets/${id}/reject`, {
          method: 'POST',
          body: JSON.stringify({ reason: decisionNote || 'Korrektur nötig' }),
        });
      } else {
        await api(serverUrl, `/api/v1/time/timesheets/${id}/approve`, { method: 'POST' });
      }
      await afterAction('Erledigt');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function absenceAction(id: string, action: 'approve' | 'reject') {
    try {
      await api(serverUrl, `/api/v1/absences/${id}/${action}`, {
        method: 'POST',
        body: JSON.stringify({ note: decisionNote || undefined }),
      });
      await afterAction('Erledigt');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function approveAllPendingTimesheets() {
    try {
      const res = await api<{ approved: number }>(
        serverUrl,
        '/api/v1/time/timesheets/approve-pending',
        { method: 'POST' },
      );
      await afterAction(`${res.approved} Stundenzettel freigegeben`);
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function approveAllPendingAbsences() {
    try {
      const res = await api<{ approved: number }>(
        serverUrl,
        '/api/v1/absences/approve-pending',
        { method: 'POST' },
      );
      await afterAction(`${res.approved} Abwesenheiten freigegeben`);
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function submitTeamTimesheet(id: string) {
    try {
      await api(serverUrl, `/api/v1/time/timesheets/${id}/submit`, { method: 'POST' });
      await afterAction('Stundenzettel eingereicht');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function submitAllTeamDraftTimesheets() {
    try {
      const res = await api<{ submitted: number }>(
        serverUrl,
        '/api/v1/time/timesheets/submit-drafts',
        { method: 'POST' },
      );
      await afterAction(
        res.submitted > 0
          ? `${res.submitted} Team-Entwürfe eingereicht`
          : 'Keine Team-Entwürfe zum Einreichen',
      );
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  $effect(() => {
    if (active) void refresh();
  });
</script>

<TsPageHeader
  title="Freigaben"
  lead="Offene Stundenzettel und Abwesenheiten freigeben oder ablehnen. Team-Entwürfe vorher einreichen."
/>
<input bind:value={decisionNote} placeholder="Kommentar bei Ablehnung (optional)" />
{#if pendingTimesheets.length > 0 || pendingAbsences.length > 0 || teamDraftQueueCount > 0}
  <div class="btn-row" style="margin-top: 0.5rem;">
    {#if pendingTimesheets.length > 0}
      <button class="secondary" type="button" onclick={approveAllPendingTimesheets}>
        Alle {pendingTimesheets.length} Stundenzettel freigeben
      </button>
    {/if}
    {#if pendingAbsences.length > 0}
      <button class="secondary" type="button" onclick={approveAllPendingAbsences}>
        Alle {pendingAbsences.length} Abwesenheiten freigeben
      </button>
    {/if}
    {#if teamDraftQueueCount > 0}
      <button class="secondary" type="button" onclick={submitAllTeamDraftTimesheets}>
        Alle {teamDraftQueueCount} Team-Entwürfe einreichen
      </button>
    {/if}
  </div>
{/if}
<div class="card" style="margin-top: 1rem;">
  <h3>Stundenzettel ({pendingTimesheets.length})</h3>
  {#each pendingTimesheets as t}
    <div class="row-card">
      <p>
        <strong>{t.employee_name ?? 'MA'}</strong>
        {#if t.period_start}
          <span class="muted">
            · {calendarWeekLabel(t.period_start)} ({formatPeriodRange(t.period_start, t.period_end)})
          </span>
        {/if}
      </p>
      <p class="muted">
        {formatMinutes(t.worked_minutes)} Arbeit · ÜS {formatMinutes(t.overtime_minutes)}
      </p>
      <div class="btn-row">
        <button type="button" onclick={() => timesheetAction(t.id, 'approve')}>Freigeben</button>
        <button class="secondary" type="button" onclick={() => timesheetAction(t.id, 'reject')}>
          Ablehnen
        </button>
      </div>
    </div>
  {:else}
    <TsEmptyState message="Keine offenen Stundenzettel zur Freigabe." />
  {/each}
</div>
<div class="card" style="margin-top: 1rem;">
  <h3>Abwesenheit ({pendingAbsences.length})</h3>
  {#each pendingAbsences as a}
    <div class="row-card">
      <p>
        <strong>{a.employee_name}</strong> — {statusLabel(a.absence_type)}
      </p>
      <p class="muted">
        {formatIsoShort(a.starts_at)} → {formatIsoShort(a.ends_at)}
      </p>
      <div class="btn-row">
        <button type="button" onclick={() => absenceAction(a.id, 'approve')}>Freigeben</button>
        <button class="secondary" type="button" onclick={() => absenceAction(a.id, 'reject')}>
          Ablehnen
        </button>
      </div>
    </div>
  {:else}
    <TsEmptyState message="Keine offenen Abwesenheitsanträge." />
  {/each}
</div>
<div class="card" style="margin-top: 1rem;">
  <h3>Team-Entwürfe ({teamDraftRows.length})</h3>
  <p class="muted" style="margin-bottom: 0.5rem;">
    Entwürfe anderer Mitarbeiter vor Freigabe einreichen.
  </p>
  {#each teamDraftRows as t}
    <div class="row-card">
      <p>
        <strong>{t.employee_name ?? 'MA'}</strong>
        {#if t.period_start}
          <span class="muted">
            · {calendarWeekLabel(t.period_start)} ({formatPeriodRange(t.period_start, t.period_end)})
          </span>
        {/if}
      </p>
      <p class="muted">
        {formatMinutes(t.worked_minutes)} · {statusLabel(t.status)}
        {#if t.rejection_reason}
          · {t.rejection_reason}
        {/if}
      </p>
      <button class="secondary" type="button" onclick={() => submitTeamTimesheet(t.id)}>
        Einreichen
      </button>
    </div>
  {:else}
    <TsEmptyState message="Keine Team-Entwürfe — alle Stundenzettel eingereicht." />
  {/each}
</div>
