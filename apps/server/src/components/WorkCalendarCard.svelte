<script lang="ts">
  import { api } from '../lib/api';
  import { formatMinutes } from '../lib/formatMinutes';
  import { weekLabelForAnchor, weekRangeContaining } from '../lib/datetime';

  type Employee = {
    id: string;
    employee_no: string;
    display_name: string;
    active?: boolean;
  };

  type UiMessage = { type: 'error' | 'success'; text: string };

  let {
    apiUrl,
    shiftWeekAnchor = $bindable(new Date()),
    employees,
    active = false,
    onMessage,
    onWeekChange,
  }: {
    apiUrl: string;
    shiftWeekAnchor?: Date;
    employees: Employee[];
    active?: boolean;
    onMessage?: (msg: UiMessage) => void;
    onWeekChange?: () => void;
  } = $props();

  type WorkdayModelConfig = {
    expected_minutes: number;
    label?: string | null;
    flex_band?: { earliest_start: string; latest_end: string } | null;
    core_time?: { start: string; end: string } | null;
    break_expectation?: {
      required_after_minutes: number;
      required_minutes: number;
    } | null;
    auto_credit_expected?: boolean;
    worked_rounding_minutes?: number | null;
  };

  let workdayModels = $state<{ id: string; name: string; config: WorkdayModelConfig }[]>([]);
  let editModelId = $state('wm-std-8h');
  let editExpectedMinutes = $state(480);
  let editLabel = $state('');
  let editFlexEarliest = $state('06:00');
  let editFlexLatest = $state('20:00');
  let savingModel = $state(false);
  let workCalendars = $state<{ id: string; name: string }[]>([]);
  let rotationPlans = $state<
    { id: string; name: string; cycle_days: number; anchor_date: string }[]
  >([]);
  let workAssignments = $state<
    {
      id: string;
      employee_id: string;
      employee_no: string;
      employee_name: string;
      work_calendar_id: string;
      work_calendar_name: string;
      valid_from: string;
      part_time_percent: number;
    }[]
  >([]);
  let workCalendarDays = $state<
    { date: string; workday_model_id: string; model_name: string }[]
  >([]);
  let selectedWorkCalendarId = $state('wc-default-standard');
  let calendarGenYear = $state(new Date().getFullYear());
  let calendarRotationPlanId = $state('');
  let workAssignmentEmployeeFilter = $state('');
  let newWorkAssignment = $state({
    employee_id: '',
    work_calendar_id: 'wc-default-standard',
    valid_from: `${new Date().getFullYear()}-01-01`,
    part_time_percent: 100,
  });
  let calendarDayOverride = $state({
    date: '',
    workday_model_id: 'wm-std-8h',
  });

  function toYmd(d: Date): string {
    const pad = (n: number) => String(n).padStart(2, '0');
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  }

  function notify(type: 'error' | 'success', text: string) {
    onMessage?.({ type, text });
  }

  function syncEditFromModel(id: string) {
    const m = workdayModels.find((x) => x.id === id);
    if (!m) return;
    editExpectedMinutes = m.config.expected_minutes ?? 0;
    editLabel = m.config.label ?? '';
    editFlexEarliest = m.config.flex_band?.earliest_start ?? '06:00';
    editFlexLatest = m.config.flex_band?.latest_end ?? '20:00';
  }

  async function saveWorkdayModel() {
    const m = workdayModels.find((x) => x.id === editModelId);
    if (!m) {
      notify('error', 'Tagesmodell wählen');
      return;
    }
    savingModel = true;
    try {
      const config: WorkdayModelConfig = {
        ...m.config,
        expected_minutes: editExpectedMinutes,
        label: editLabel.trim() || null,
        flex_band: {
          earliest_start: editFlexEarliest,
          latest_end: editFlexLatest,
        },
      };
      await api(apiUrl, `/api/v1/time/workday-models/${editModelId}`, {
        method: 'PUT',
        body: JSON.stringify({ config }),
      });
      await refresh();
      onWeekChange?.();
      notify('success', 'Tagesmodell gespeichert — betroffene Stundenzettel werden neu berechnet');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    } finally {
      savingModel = false;
    }
  }

  $effect(() => {
    if (workdayModels.length > 0) {
      if (!workdayModels.some((m) => m.id === editModelId)) {
        editModelId = workdayModels[0].id;
      }
      syncEditFromModel(editModelId);
    }
  });

  async function refreshWorkCalendarWeekDays() {
    if (!selectedWorkCalendarId) {
      workCalendarDays = [];
      return;
    }
    const { days } = weekRangeContaining(shiftWeekAnchor);
    const from = toYmd(days[0]);
    const to = toYmd(days[6]);
    workCalendarDays = await api<typeof workCalendarDays>(
      apiUrl,
      `/api/v1/time/work-calendars/${selectedWorkCalendarId}/days?from=${from}&to=${to}`,
    ).catch(() => []);
  }

  export async function refresh() {
    workdayModels = await api<typeof workdayModels>(
      apiUrl,
      '/api/v1/time/workday-models',
    ).catch(() => []);
    workCalendars = await api<typeof workCalendars>(
      apiUrl,
      '/api/v1/time/work-calendars',
    ).catch(() => []);
    rotationPlans = await api<typeof rotationPlans>(
      apiUrl,
      '/api/v1/time/work-rotation-plans',
    ).catch(() => []);
    if (workCalendars.length && !workCalendars.some((c) => c.id === selectedWorkCalendarId)) {
      selectedWorkCalendarId = workCalendars[0].id;
    }
    const assignParams = new URLSearchParams();
    if (workAssignmentEmployeeFilter) {
      assignParams.set('employee_id', workAssignmentEmployeeFilter);
    }
    const assignQs = assignParams.toString();
    workAssignments = await api<typeof workAssignments>(
      apiUrl,
      assignQs
        ? `/api/v1/time/employee-work-assignments?${assignQs}`
        : '/api/v1/time/employee-work-assignments',
    ).catch(() => []);
    await refreshWorkCalendarWeekDays();
  }

  $effect(() => {
    if (active) {
      shiftWeekAnchor;
      void refresh();
    }
  });

  function shiftWeek(delta: number) {
    const d = new Date(shiftWeekAnchor);
    d.setDate(d.getDate() + delta * 7);
    shiftWeekAnchor = d;
    onWeekChange?.();
    void refreshWorkCalendarWeekDays();
  }

  function goToThisWeek() {
    shiftWeekAnchor = new Date();
    onWeekChange?.();
    void refreshWorkCalendarWeekDays();
  }

  async function generateWorkCalendarYear() {
    if (!selectedWorkCalendarId) {
      notify('error', 'Arbeitskalender wählen');
      return;
    }
    try {
      const res = await api<{ inserted: number }>(
        apiUrl,
        `/api/v1/time/work-calendars/${selectedWorkCalendarId}/generate-year`,
        { method: 'POST', body: JSON.stringify({ year: calendarGenYear }) },
      );
      await refresh();
      onWeekChange?.();
      notify('success', `${res.inserted} Kalendertage für ${calendarGenYear} ergänzt`);
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function setCalendarDay() {
    if (!selectedWorkCalendarId || !calendarDayOverride.date) {
      notify('error', 'Kalender und Datum wählen');
      return;
    }
    try {
      await api(
        apiUrl,
        `/api/v1/time/work-calendars/${selectedWorkCalendarId}/days/${calendarDayOverride.date}`,
        {
          method: 'PUT',
          body: JSON.stringify({ workday_model_id: calendarDayOverride.workday_model_id }),
        },
      );
      await refreshWorkCalendarWeekDays();
      onWeekChange?.();
      notify('success', `Kalendertag ${calendarDayOverride.date} gespeichert`);
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function createWorkAssignment() {
    if (!newWorkAssignment.employee_id) {
      notify('error', 'Mitarbeiter wählen');
      return;
    }
    try {
      await api(apiUrl, '/api/v1/time/employee-work-assignments', {
        method: 'POST',
        body: JSON.stringify(newWorkAssignment),
      });
      await refresh();
      onWeekChange?.();
      notify('success', 'Kalender-Zuordnung gespeichert');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function setCalendarRotation() {
    if (!selectedWorkCalendarId) {
      notify('error', 'Arbeitskalender wählen');
      return;
    }
    try {
      await api(apiUrl, `/api/v1/time/work-calendars/${selectedWorkCalendarId}/rotation`, {
        method: 'PUT',
        body: JSON.stringify({
          rotation_plan_id: calendarRotationPlanId || null,
        }),
      });
      notify(
        'success',
        calendarRotationPlanId
          ? 'Umschaltplan am Kalender aktiv'
          : 'Umschaltplan vom Kalender entfernt',
      );
      await refresh();
      onWeekChange?.();
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function copyWorkCalendarWeek() {
    if (!selectedWorkCalendarId) {
      notify('error', 'Arbeitskalender wählen');
      return;
    }
    const { days } = weekRangeContaining(shiftWeekAnchor);
    const from = toYmd(days[0]);
    const to = toYmd(days[6]);
    const next = new Date(days[0].getTime());
    next.setDate(next.getDate() + 7);
    const targetFrom = toYmd(next);
    try {
      const res = await api<{ copied: number }>(
        apiUrl,
        `/api/v1/time/work-calendars/${selectedWorkCalendarId}/copy-days`,
        {
          method: 'POST',
          body: JSON.stringify({ source_from: from, source_to: to, target_from: targetFrom }),
        },
      );
      await refreshWorkCalendarWeekDays();
      onWeekChange?.();
      notify('success', `${res.copied} Tage auf KW+1 kopiert`);
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  const activeEmployees = $derived(employees.filter((e) => e.active !== false));
</script>

<div class="card" style="margin-top: 1rem;">
  <h3>Arbeitskalender & Tagesmodelle</h3>
  <p class="muted" style="margin-bottom: 0.75rem;">
    Sollzeit und Feiertage kommen aus dem Arbeitskalender (PrimeWeb-Tages-/Jahresperiode).
    Schichtvorlagen unten erzeugen nur geplante Schichten.
  </p>
  <div class="btn-row">
    <button class="secondary" type="button" onclick={() => shiftWeek(-1)}>← KW</button>
    <span class="muted">{weekLabelForAnchor(shiftWeekAnchor)}</span>
    <button class="secondary" type="button" onclick={() => shiftWeek(1)}>KW →</button>
    <button class="secondary" type="button" onclick={goToThisWeek}>Diese Woche</button>
  </div>
  <div class="grid-form" style="margin-top: 0.75rem;">
    <select bind:value={selectedWorkCalendarId} onchange={() => refreshWorkCalendarWeekDays()}>
      {#each workCalendars as c}
        <option value={c.id}>{c.name}</option>
      {/each}
    </select>
    <input type="number" bind:value={calendarGenYear} min="2020" max="2035" />
    <button class="secondary" type="button" onclick={generateWorkCalendarYear}>
      Jahr befüllen (Mo–Fr)
    </button>
    <button class="secondary" type="button" onclick={copyWorkCalendarWeek}>
      Diese KW → nächste KW kopieren
    </button>
    <select bind:value={calendarRotationPlanId}>
      <option value="">Kein Umschaltplan</option>
      {#each rotationPlans as p}
        <option value={p.id}>{p.name} ({p.cycle_days} Tage)</option>
      {/each}
    </select>
    <button class="secondary" type="button" onclick={setCalendarRotation}>Umschaltplan zuweisen</button>
  </div>
  <div class="week-calendar" style="margin-top: 0.75rem;">
    {#each workCalendarDays as d}
      <div class="day-col">
        <h4>{d.date.slice(8, 10)}.{d.date.slice(5, 7)}</h4>
        <div class="shift-chip published" title={d.workday_model_id}>{d.model_name}</div>
      </div>
    {:else}
      <p class="muted">Keine Kalendertage — Jahr befüllen oder Server neu starten (Seed).</p>
    {/each}
  </div>
  <div class="grid-form" style="margin-top: 0.75rem;">
    <input type="date" bind:value={calendarDayOverride.date} />
    <select bind:value={calendarDayOverride.workday_model_id}>
      {#each workdayModels as m}
        <option value={m.id}>{m.name}</option>
      {/each}
    </select>
    <button class="secondary" type="button" onclick={setCalendarDay}>Einzelnen Tag setzen</button>
  </div>
  <h4 style="margin-top: 1rem;">Tagesmodelle (Soll)</h4>
  <ul class="compact-list">
    {#each workdayModels as m}
      <li>
        <strong>{m.name}</strong>
        <span class="muted">
          — {formatMinutes(m.config.expected_minutes ?? 0)} Soll
          {#if m.config.label} ({m.config.label}){/if}
          {#if m.config.flex_band}
            · Gleit {m.config.flex_band.earliest_start}–{m.config.flex_band.latest_end}
          {/if}
        </span>
      </li>
    {:else}
      <li class="muted">Keine Tagesmodelle geladen.</li>
    {/each}
  </ul>
  {#if workdayModels.length > 0}
    <div class="grid-form" style="margin-top: 0.75rem;">
      <select
        bind:value={editModelId}
        onchange={() => syncEditFromModel(editModelId)}
      >
        {#each workdayModels as m}
          <option value={m.id}>{m.name}</option>
        {/each}
      </select>
      <input
        type="number"
        bind:value={editExpectedMinutes}
        min="0"
        max="720"
        step="15"
        title="Soll Minuten pro Tag"
      />
      <input bind:value={editLabel} placeholder="Bezeichnung (optional)" />
      <input bind:value={editFlexEarliest} placeholder="Gleit ab HH:MM" />
      <input bind:value={editFlexLatest} placeholder="Gleit bis HH:MM" />
      <button type="button" disabled={savingModel} onclick={saveWorkdayModel}>
        {savingModel ? 'Speichern…' : 'Tagesmodell speichern'}
      </button>
    </div>
    <p class="muted" style="font-size: 0.8rem; margin-top: 0.35rem;">
      Änderung an Soll/Gleitzeit löst Neuberechnung aller Kalender mit diesem Modell aus.
    </p>
  {/if}
  <h4 style="margin-top: 1rem;">Mitarbeiter-Zuordnung</h4>
  <div class="grid-form">
    <select bind:value={workAssignmentEmployeeFilter} onchange={() => refresh()}>
      <option value="">Alle Mitarbeiter</option>
      {#each activeEmployees as e}
        <option value={e.id}>{e.employee_no} — {e.display_name}</option>
      {/each}
    </select>
    <select bind:value={newWorkAssignment.employee_id}>
      <option value="">Mitarbeiter…</option>
      {#each activeEmployees as e}
        <option value={e.id}>{e.employee_no} — {e.display_name}</option>
      {/each}
    </select>
    <select bind:value={newWorkAssignment.work_calendar_id}>
      {#each workCalendars as c}
        <option value={c.id}>{c.name}</option>
      {/each}
    </select>
    <input type="date" bind:value={newWorkAssignment.valid_from} />
    <input
      type="number"
      bind:value={newWorkAssignment.part_time_percent}
      min="1"
      max="100"
      title="Teilzeit %"
    />
    <button type="button" onclick={createWorkAssignment}>Zuordnung speichern</button>
  </div>
  <ul class="compact-list" style="margin-top: 0.5rem;">
    {#each workAssignments as a}
      <li class="row-card">
        {a.employee_no} {a.employee_name} → {a.work_calendar_name}
        <span class="muted">ab {a.valid_from.slice(0, 10)}, {a.part_time_percent}%</span>
      </li>
    {:else}
      <li class="muted">Keine Zuordnungen (Seed legt Standard-Kalender an).</li>
    {/each}
  </ul>
</div>
