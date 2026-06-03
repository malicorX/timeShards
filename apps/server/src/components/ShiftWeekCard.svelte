<script lang="ts">
  import { api } from '../lib/api';
  import { statusLabel } from '../lib/statusLabels';
  import TsEmptyState from '@timeshards/shared/ui/TsEmptyState.svelte';
  import {
    toLocalDatetimeInputValue,
    fromLocalDatetimeInputValue,
    formatIsoLocalShort,
    formatDayLabel,
    weekLabelForAnchor,
    weekRangeContaining,
    defaultShiftRange,
    shiftOnDay,
  } from '../lib/datetime';

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
    shiftWeekAnchor = $bindable(new Date()),
    shiftFilter = $bindable<'all' | 'planned' | 'published' | 'cancelled'>('all'),
    active = false,
    canApprove = false,
    onMessage,
    onDashboardChange,
  }: {
    apiUrl: string;
    employees: Employee[];
    shiftWeekAnchor?: Date;
    shiftFilter?: 'all' | 'planned' | 'published' | 'cancelled';
    active?: boolean;
    canApprove?: boolean;
    onMessage?: (msg: UiMessage) => void;
    onDashboardChange?: () => void | Promise<void>;
  } = $props();

  const weekdayLabels = ['', 'Mo', 'Di', 'Mi', 'Do', 'Fr', 'Sa', 'So'];
  const defaultRange = defaultShiftRange();

  let shifts = $state<
    {
      id: string;
      employee_id: string;
      employee_no: string;
      employee_name: string;
      starts_at: string;
      ends_at: string;
      status: string;
    }[]
  >([]);
  let shiftTemplates = $state<
    {
      id: string;
      employee_id: string;
      employee_name: string;
      employee_no: string;
      name: string;
      weekday: number;
      starts_time: string;
      ends_time: string;
    }[]
  >([]);
  let shiftTemplateEmployeeFilter = $state('');
  let shiftEmployeeFilter = $state('');

  const shiftWeek = $derived(weekRangeContaining(shiftWeekAnchor));
  const plannedShiftsInView = $derived(shifts.filter((s) => s.status === 'planned').length);
  let shiftConflict = $state<string | null>(null);
  let newTemplate = $state({
    employee_id: '',
    name: 'Standard',
    weekday: 1,
    starts_time: '08:00',
    ends_time: '16:00',
  });
  let newShift = $state({
    employee_id: '',
    starts_local: toLocalDatetimeInputValue(new Date(new Date(defaultRange.start).getTime())),
    ends_local: toLocalDatetimeInputValue(new Date(new Date(defaultRange.end).getTime())),
  });

  function notify(type: 'error' | 'success', text: string) {
    onMessage?.({ type, text });
  }

  function employeeLabel(employeeId: string) {
    const e = employees.find((x) => x.id === employeeId);
    return e ? `${e.employee_no} ${e.display_name}` : employeeId.slice(0, 8);
  }

  function shiftsForDay(day: Date) {
    return shifts.filter((s) => shiftOnDay(s.starts_at, day));
  }

  export async function refresh() {
    const { from, to } = weekRangeContaining(shiftWeekAnchor);
    const shiftParams = new URLSearchParams({ from, to });
    if (shiftFilter !== 'all') shiftParams.set('status', shiftFilter);
    if (shiftEmployeeFilter) shiftParams.set('employee_id', shiftEmployeeFilter);
    shifts = await api<typeof shifts>(
      apiUrl,
      `/api/v1/time/shifts?${shiftParams}`,
    ).catch(() => []);
    await refreshShiftTemplates();
  }

  async function refreshShiftTemplates() {
    const params = new URLSearchParams();
    if (shiftTemplateEmployeeFilter) params.set('employee_id', shiftTemplateEmployeeFilter);
    const qs = params.toString();
    shiftTemplates = await api<typeof shiftTemplates>(
      apiUrl,
      qs ? `/api/v1/time/shift-templates?${qs}` : '/api/v1/time/shift-templates',
    ).catch(() => []);
  }

  async function checkShiftConflict() {
    if (!newShift.employee_id) return;
    try {
      const starts = fromLocalDatetimeInputValue(newShift.starts_local);
      const ends = fromLocalDatetimeInputValue(newShift.ends_local);
      const r = await api<{ has_conflict: boolean; message?: string }>(
        apiUrl,
        `/api/v1/time/shifts/conflicts?employee_id=${encodeURIComponent(newShift.employee_id)}&starts_at=${encodeURIComponent(starts)}&ends_at=${encodeURIComponent(ends)}`,
      );
      shiftConflict = r.has_conflict ? (r.message ?? 'Zeitraum kollidiert') : null;
    } catch {
      shiftConflict = null;
    }
  }

  async function createShiftTemplate() {
    try {
      await api(apiUrl, '/api/v1/time/shift-templates', {
        method: 'POST',
        body: JSON.stringify(newTemplate),
      });
      await refresh();
      await onDashboardChange?.();
      notify('success', 'Schichtvorlage gespeichert');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function applyShiftTemplates() {
    const { from } = weekRangeContaining(shiftWeekAnchor);
    try {
      const params = new URLSearchParams({ week_start: from });
      if (shiftTemplateEmployeeFilter) params.set('employee_id', shiftTemplateEmployeeFilter);
      const res = await api<{ created: number; skipped: number }>(
        apiUrl,
        `/api/v1/time/shift-templates/apply-week?${params}`,
        { method: 'POST' },
      );
      await refresh();
      await onDashboardChange?.();
      notify('success', `${res.created} Schichten erzeugt (${res.skipped} übersprungen)`);
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function publishPlannedShifts() {
    try {
      const { from } = weekRangeContaining(shiftWeekAnchor);
      const res = await api<{ published: number }>(
        apiUrl,
        `/api/v1/time/shifts/publish-planned?week_start=${encodeURIComponent(from)}`,
        { method: 'POST' },
      );
      await refresh();
      await onDashboardChange?.();
      notify('success', `${res.published} geplante Schichten veröffentlicht`);
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function deactivateShiftTemplate(id: string) {
    await api(apiUrl, `/api/v1/time/shift-templates/${id}/deactivate`, { method: 'POST' });
    await refresh();
    await onDashboardChange?.();
    notify('success', 'Vorlage deaktiviert');
  }

  async function duplicateShiftTemplate(t: {
    employee_id: string;
    name: string;
    weekday: number;
    starts_time: string;
    ends_time: string;
  }) {
    try {
      await api(apiUrl, '/api/v1/time/shift-templates', {
        method: 'POST',
        body: JSON.stringify({
          employee_id: t.employee_id,
          name: `${t.name} (Kopie)`,
          weekday: t.weekday,
          starts_time: t.starts_time,
          ends_time: t.ends_time,
        }),
      });
      await refreshShiftTemplates();
      notify('success', 'Vorlage dupliziert');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function addWeekdayTemplates() {
    if (!newTemplate.employee_id) {
      notify('error', 'Mitarbeiter wählen');
      return;
    }
    try {
      let n = 0;
      for (let wd = 1; wd <= 5; wd++) {
        await api(apiUrl, '/api/v1/time/shift-templates', {
          method: 'POST',
          body: JSON.stringify({
            employee_id: newTemplate.employee_id,
            name: newTemplate.name || 'Standard',
            weekday: wd,
            starts_time: newTemplate.starts_time,
            ends_time: newTemplate.ends_time,
          }),
        });
        n += 1;
      }
      await refreshShiftTemplates();
      await onDashboardChange?.();
      notify('success', `${n} Vorlagen (Mo–Fr) angelegt`);
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function createShift() {
    try {
      await api(apiUrl, '/api/v1/time/shifts', {
        method: 'POST',
        body: JSON.stringify({
          employee_id: newShift.employee_id,
          starts_at: fromLocalDatetimeInputValue(newShift.starts_local),
          ends_at: fromLocalDatetimeInputValue(newShift.ends_local),
        }),
      });
      await refresh();
      await onDashboardChange?.();
      notify('success', 'Schicht geplant');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function publishShift(id: string) {
    await api(apiUrl, `/api/v1/time/shifts/${id}/publish`, { method: 'POST' });
    await refresh();
    await onDashboardChange?.();
    notify('success', 'Schicht veröffentlicht');
  }

  async function cancelShift(id: string) {
    await api(apiUrl, `/api/v1/time/shifts/${id}/cancel`, { method: 'POST' });
    await refresh();
    await onDashboardChange?.();
    notify('success', 'Schicht storniert');
  }

  function shiftWeekPrev() {
    const d = new Date(shiftWeekAnchor);
    d.setDate(d.getDate() - 7);
    shiftWeekAnchor = d;
    void refresh();
  }

  function shiftWeekNext() {
    const d = new Date(shiftWeekAnchor);
    d.setDate(d.getDate() + 7);
    shiftWeekAnchor = d;
    void refresh();
  }

  function goToThisWeek() {
    shiftWeekAnchor = new Date();
    void refresh();
  }

  $effect(() => {
    if (employees.length && !newTemplate.employee_id) {
      newTemplate.employee_id = employees[0].id;
    }
    if (employees.length && !newShift.employee_id) {
      newShift.employee_id = employees[0].id;
    }
  });

  $effect(() => {
    if (!active) return;
    newShift.employee_id;
    newShift.starts_local;
    newShift.ends_local;
    void checkShiftConflict();
  });

  $effect(() => {
    if (active) {
      shiftWeekAnchor;
      void refresh();
    }
  });
</script>

<div class="ts-section-intro">
  <h3>Schichtplanung</h3>
  <p class="ts-lead">
    Schichten sind Planung — Sollzeit kommt aus Perioden. Vorlagen erzeugen Wochen-Schichten; veröffentlichen für
    Mitarbeiteransicht.
  </p>
</div>

<div class="card" style="margin-top: 1rem;">
  <h3>Wochenvorlagen (geplante Schichten)</h3>
  <div class="grid-form">
    <select bind:value={newTemplate.employee_id}>
      {#each employees as e}
        <option value={e.id}>{e.employee_no} — {e.display_name}</option>
      {/each}
    </select>
    <select bind:value={shiftTemplateEmployeeFilter} onchange={() => refreshShiftTemplates()}>
      <option value="">Alle Mitarbeiter (Liste & Anwenden)</option>
      {#each employees.filter((e) => e.active !== false) as e}
        <option value={e.id}>{e.employee_no} — {e.display_name}</option>
      {/each}
    </select>
    <input bind:value={newTemplate.name} placeholder="Name der Vorlage" />
    <select bind:value={newTemplate.weekday}>
      {#each [1, 2, 3, 4, 5, 6, 7] as d}
        <option value={d}>{weekdayLabels[d]}</option>
      {/each}
    </select>
    <input bind:value={newTemplate.starts_time} placeholder="08:00" />
    <input bind:value={newTemplate.ends_time} placeholder="16:00" />
    <button type="button" onclick={createShiftTemplate}>Vorlage speichern</button>
    <button class="secondary" type="button" onclick={addWeekdayTemplates}>Mo–Fr aus Feldern</button>
    <button class="secondary" type="button" onclick={applyShiftTemplates}>
      {shiftTemplateEmployeeFilter ? 'Vorlagen → KW (1 MA)' : 'Vorlagen → Kalenderwoche'}
    </button>
    <button
      class="secondary"
      type="button"
      onclick={publishPlannedShifts}
      disabled={plannedShiftsInView === 0}
    >
      {plannedShiftsInView > 0
        ? `${plannedShiftsInView} geplante veröffentlichen`
        : 'Geplante veröffentlichen'}
    </button>
  </div>
  <ul class="compact-list" style="margin-top: 0.5rem;">
    {#each shiftTemplates as t}
      <li class="row-card">
        {t.employee_no} {weekdayLabels[t.weekday]} {t.starts_time}–{t.ends_time} ({t.name})
        <button class="secondary" type="button" onclick={() => duplicateShiftTemplate(t)}>
          Duplizieren
        </button>
        <button class="secondary" type="button" onclick={() => deactivateShiftTemplate(t.id)}>
          Entfernen
        </button>
      </li>
    {:else}
      <li class="muted">Keine Vorlagen für diesen Filter.</li>
    {/each}
  </ul>
</div>

<div class="card" style="margin-top: 1rem;">
  <h3>Schicht planen</h3>
  <div class="grid-form">
    <select bind:value={newShift.employee_id}>
      {#each employees as e}
        <option value={e.id}>{e.employee_no} — {e.display_name}</option>
      {/each}
    </select>
    <label class="muted" for="new-shift-start">Beginn</label>
    <input id="new-shift-start" type="datetime-local" bind:value={newShift.starts_local} />
    <label class="muted" for="new-shift-end">Ende</label>
    <input id="new-shift-end" type="datetime-local" bind:value={newShift.ends_local} />
    <button type="button" onclick={createShift} disabled={!!shiftConflict}>Schicht speichern</button>
    {#if shiftConflict}<p class="error">{shiftConflict}</p>{/if}
  </div>
  <div class="btn-row" style="margin-top: 1rem;">
    <button class="secondary" type="button" onclick={shiftWeekPrev}>← Woche</button>
    <span class="muted">{weekLabelForAnchor(shiftWeekAnchor)}</span>
    <button class="secondary" type="button" onclick={shiftWeekNext}>Woche →</button>
    <button class="secondary" type="button" onclick={goToThisWeek}>Heute</button>
  </div>
  <div class="week-calendar">
    {#each shiftWeek.days as day}
      <div class="day-col">
        <h4>{formatDayLabel(day)}</h4>
        {#each shiftsForDay(day) as s}
          <div
            class="shift-chip {s.status}"
            title="{statusLabel(s.status)} — {s.starts_at} – {s.ends_at}"
          >
            {s.employee_no}{' '}
            {new Date(s.starts_at).toLocaleTimeString('de-DE', {
              hour: '2-digit',
              minute: '2-digit',
            })}
          </div>
        {/each}
      </div>
    {/each}
  </div>
  <div class="btn-row" style="margin-top: 1rem;">
    <select bind:value={shiftFilter} onchange={() => refresh()}>
      <option value="all">Alle Schichten</option>
      <option value="planned">Geplant</option>
      <option value="published">Veröffentlicht</option>
      <option value="cancelled">Storniert</option>
    </select>
    {#if canApprove}
      <select bind:value={shiftEmployeeFilter} onchange={() => refresh()}>
        <option value="">Alle Mitarbeiter</option>
        {#each employees.filter((e) => e.active !== false) as e}
          <option value={e.id}>{e.employee_no} — {e.display_name}</option>
        {/each}
      </select>
    {/if}
  </div>
  <h3 style="margin-top: 0.5rem;">Schichten in dieser Woche</h3>
  {#if shifts.length === 0}
    <TsEmptyState message="Keine Schichten in dieser KW — Vorlage anwenden oder Einzelschicht anlegen." />
  {/if}
  {#each shifts as s}
    <div class="row-card">
      <p>
        {employeeLabel(s.employee_id)} — {formatIsoLocalShort(s.starts_at)} →
        {formatIsoLocalShort(s.ends_at)} — {statusLabel(s.status)}
      </p>
      <div class="btn-row">
        {#if s.status === 'planned'}
          <button type="button" onclick={() => publishShift(s.id)}>Veröffentlichen</button>
        {/if}
        {#if s.status !== 'cancelled'}
          <button class="secondary" type="button" onclick={() => cancelShift(s.id)}>
            Stornieren
          </button>
        {/if}
      </div>
    </div>
  {/each}
</div>
