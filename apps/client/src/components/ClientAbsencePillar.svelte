<script lang="ts">
  import { api } from '../lib/api';
  import { toLocalDatetimeInputValue, fromLocalDatetimeInputValue, formatIsoShort } from '../lib/datetime';
  import { statusLabel } from '../lib/statusLabels';

  type UiMessage = { type: 'error' | 'success'; text: string };
  type AbsenceRow = {
    id: string;
    employee_name: string;
    absence_type: string;
    starts_at: string;
    ends_at: string;
    status: string;
  };

  let {
    serverUrl,
    absenceFilter = $bindable<'all' | 'pending' | 'approved' | 'rejected'>('all'),
    active = false,
    onMessage,
    onRefreshParent,
  }: {
    serverUrl: string;
    absenceFilter?: 'all' | 'pending' | 'approved' | 'rejected';
    active?: boolean;
    onMessage?: (msg: UiMessage) => void;
    onRefreshParent?: () => void | Promise<void>;
  } = $props();

  let absences = $state<AbsenceRow[]>([]);
  let absenceConflict = $state<string | null>(null);
  let newAbsence = $state({
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
    if (absenceFilter !== 'all') path += `?status=${absenceFilter}`;
    absences = await api<AbsenceRow[]>(serverUrl, path).catch(() => []);
  }

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

  $effect(() => {
    if (active) void refresh();
  });

  $effect(() => {
    if (!active) return;
    newAbsence.starts_local;
    newAbsence.ends_local;
    void checkAbsenceConflict();
  });

  async function createAbsence() {
    try {
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
      await refresh();
      await onRefreshParent?.();
      notify('success', 'Abwesenheit beantragt');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function cancelAbsence(id: string) {
    try {
      await api(serverUrl, `/api/v1/absences/${id}/cancel`, { method: 'POST' });
      await refresh();
      await onRefreshParent?.();
      notify('success', 'Antrag storniert');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }
</script>

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
    <button type="button" onclick={createAbsence} disabled={!!absenceConflict}>Beantragen</button>
    {#if absenceConflict}<p class="error">{absenceConflict}</p>{/if}
  </div>
</div>
<div class="card" style="margin-top: 1rem;">
  <select
    bind:value={absenceFilter}
    onchange={() => refresh()}
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
          <button class="secondary" type="button" onclick={() => cancelAbsence(a.id)}>Stornieren</button>
        {/if}
      </li>
    {:else}
      <li class="muted">Keine Anträge</li>
    {/each}
  </ul>
</div>
