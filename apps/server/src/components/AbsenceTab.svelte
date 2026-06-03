<script lang="ts">
  import { api, getToken } from '../lib/api';
  import { formatIsoLocalShort, fromLocalDatetimeInputValue, toLocalDatetimeInputValue } from '../lib/datetime';
  import { statusLabel } from '../lib/statusLabels';

  type Employee = {
    id: string;
    employee_no: string;
    display_name: string;
    active?: boolean;
  };

  type UiMessage = { type: 'error' | 'success'; text: string };

  let {
    apiUrl,
    employees,
    canApprove,
    pendingAbsences = 0,
    absenceFilter = $bindable<'all' | 'pending' | 'approved' | 'rejected'>('all'),
    active = false,
    onMessage,
    onDashboardChange,
  }: {
    apiUrl: string;
    employees: Employee[];
    canApprove: boolean;
    pendingAbsences?: number;
    absenceFilter?: 'all' | 'pending' | 'approved' | 'rejected';
    active?: boolean;
    onMessage?: (msg: UiMessage) => void;
    onDashboardChange?: () => void;
  } = $props();

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
  let absenceEmployeeFilter = $state('');
  let absenceConflict = $state<string | null>(null);
  let newAbsence = $state({
    employee_id: '',
    absence_type: 'urlaub',
    starts_local: toLocalDatetimeInputValue(new Date()),
    ends_local: toLocalDatetimeInputValue(new Date(Date.now() + 7 * 24 * 60 * 60 * 1000)),
    reason: '',
  });

  function notify(type: 'error' | 'success', text: string) {
    onMessage?.({ type, text });
  }

  export async function refresh() {
    let path = '/api/v1/absences';
    const params: string[] = [];
    if (absenceFilter !== 'all') params.push(`status=${absenceFilter}`);
    if (canApprove && absenceEmployeeFilter) params.push(`employee_id=${absenceEmployeeFilter}`);
    if (params.length) path += `?${params.join('&')}`;
    absences = await api(apiUrl, path);
  }

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

  $effect(() => {
    if (!active) return;
    void refresh();
  });

  $effect(() => {
    if (!active) return;
    newAbsence.employee_id;
    newAbsence.starts_local;
    newAbsence.ends_local;
    void checkAbsenceConflict();
  });

  $effect(() => {
    if (employees.length && !newAbsence.employee_id) {
      newAbsence.employee_id = employees[0].id;
    }
  });

  async function createAbsence() {
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
      await refresh();
      onDashboardChange?.();
      notify('success', 'Abwesenheitsantrag erstellt');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function absenceAction(id: string, action: 'approve' | 'reject') {
    try {
      await api(apiUrl, `/api/v1/absences/${id}/${action}`, {
        method: 'POST',
        body: JSON.stringify({}),
      });
      await refresh();
      onDashboardChange?.();
      notify('success', 'Antrag aktualisiert');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function cancelAbsence(id: string) {
    try {
      await api(apiUrl, `/api/v1/absences/${id}/cancel`, { method: 'POST' });
      await refresh();
      onDashboardChange?.();
      notify('success', 'Antrag storniert');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function approveAllPendingAbsences() {
    try {
      const res = await api<{ approved: number }>(apiUrl, '/api/v1/absences/approve-pending', {
        method: 'POST',
      });
      await refresh();
      onDashboardChange?.();
      notify('success', `${res.approved} Abwesenheiten freigegeben`);
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }
</script>

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
    <button type="button" onclick={createAbsence} disabled={!!absenceConflict}>Antrag stellen</button>
    {#if absenceConflict}<p class="error">{absenceConflict}</p>{/if}
  </div>
</div>
<div class="card" style="margin-top: 1rem;">
  <div class="btn-row" style="margin-bottom: 0.5rem;">
    <select bind:value={absenceFilter} onchange={() => refresh()}>
      <option value="all">Alle Anträge</option>
      <option value="pending">Offen</option>
      <option value="approved">Freigegeben</option>
      <option value="rejected">Abgelehnt</option>
    </select>
    {#if canApprove}
      <select bind:value={absenceEmployeeFilter} onchange={() => refresh()}>
        <option value="">Alle Mitarbeiter</option>
        {#each employees.filter((e) => e.active !== false) as e}
          <option value={e.id}>{e.employee_no} — {e.display_name}</option>
        {/each}
      </select>
    {/if}
    {#if canApprove && pendingAbsences > 0}
      <button class="secondary" type="button" onclick={approveAllPendingAbsences}>
        Alle {pendingAbsences} freigeben
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
          <button type="button" onclick={() => absenceAction(a.id, 'approve')}>Freigeben</button>
          <button class="secondary" type="button" onclick={() => absenceAction(a.id, 'reject')}>
            Ablehnen
          </button>
          <button class="secondary" type="button" onclick={() => cancelAbsence(a.id)}>Stornieren</button>
        </div>
      {:else if a.status === 'pending' || a.status === 'approved'}
        <button class="secondary" type="button" onclick={() => cancelAbsence(a.id)}>Stornieren</button>
      {/if}
    </div>
  {:else}
    <p class="muted">Keine Anträge</p>
  {/each}
</div>
