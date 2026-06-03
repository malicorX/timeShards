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
  type PeriodPanel = 'jahres' | 'tages' | 'feiertage' | 'umschalt' | 'zuordnungen';
  type RotationPlan = {
    id: string;
    name: string;
    cycle_days: number;
    anchor_date: string;
    slots: { slot_index: number; workday_model_id: string; model_name: string }[];
  };

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

  let panel = $state<PeriodPanel>('jahres');
  let workdayModels = $state<{ id: string; name: string; config: WorkdayModelConfig }[]>([]);
  let editModelId = $state('wm-std-8h');
  let editModelName = $state('');
  let editExpectedMinutes = $state(480);
  let editLabel = $state('');
  let editFlexEarliest = $state('06:00');
  let editFlexLatest = $state('20:00');
  let newModelName = $state('');
  let savingModel = $state(false);
  let creatingModel = $state(false);

  let workCalendars = $state<
    {
      id: string;
      name: string;
      holiday_calendar_id?: string | null;
      rotation_plan_id?: string | null;
    }[]
  >([]);
  let holidayCalendars = $state<
    { id: string; name: string; region_code?: string | null; year_from: number; year_to: number }[]
  >([]);
  let holidayDays = $state<
    {
      date: string;
      day_kind: string;
      name?: string | null;
      model_name?: string | null;
    }[]
  >([]);
  let linkedHolidayCalendarId = $state('');
  let holidayViewYear = $state(new Date().getFullYear());
  let newCalendarName = $state('');
  let creatingCalendar = $state(false);
  let rotationPlans = $state<RotationPlan[]>([]);
  let selectedRotationPlanId = $state('');
  let rotationSlotEdits = $state<{ slot_index: number; workday_model_id: string }[]>([]);
  let savingRotation = $state(false);
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
  let selectedDayDate = $state('');
  let calendarGenYear = $state(new Date().getFullYear());
  let calendarRotationPlanId = $state('');
  let workAssignmentEmployeeFilter = $state('');
  let newWorkAssignment = $state({
    employee_id: '',
    work_calendar_id: 'wc-default-standard',
    valid_from: `${new Date().getFullYear()}-01-01`,
    part_time_percent: 100,
  });
  let dayEditModelId = $state('wm-std-8h');

  function toYmd(d: Date): string {
    const pad = (n: number) => String(n).padStart(2, '0');
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  }

  function notify(type: 'error' | 'success', text: string) {
    onMessage?.({ type, text });
  }

  function syncLinkedHolidayFromCalendar() {
    const cal = workCalendars.find((c) => c.id === selectedWorkCalendarId);
    linkedHolidayCalendarId = cal?.holiday_calendar_id ?? '';
    calendarRotationPlanId = cal?.rotation_plan_id ?? '';
  }

  function syncRotationSlotEdits() {
    const plan = rotationPlans.find((p) => p.id === selectedRotationPlanId);
    if (!plan) {
      rotationSlotEdits = [];
      return;
    }
    const byIdx = new Map(plan.slots.map((s) => [s.slot_index, s.workday_model_id]));
    rotationSlotEdits = Array.from({ length: plan.cycle_days }, (_, i) => ({
      slot_index: i,
      workday_model_id: byIdx.get(i) ?? workdayModels[0]?.id ?? 'wm-std-8h',
    }));
  }

  async function loadHolidayDays() {
    if (!linkedHolidayCalendarId) {
      holidayDays = [];
      return;
    }
    const from = `${holidayViewYear}-01-01`;
    const to = `${holidayViewYear}-12-31`;
    holidayDays = await api<typeof holidayDays>(
      apiUrl,
      `/api/v1/time/holiday-calendars/${linkedHolidayCalendarId}/days?from=${from}&to=${to}`,
    ).catch(() => []);
  }

  async function saveLinkedHoliday() {
    if (!selectedWorkCalendarId) {
      notify('error', 'Jahresperiode wählen (Tab Jahresperioden)');
      return;
    }
    try {
      await api(apiUrl, `/api/v1/time/work-calendars/${selectedWorkCalendarId}`, {
        method: 'PUT',
        body: JSON.stringify({
          holiday_calendar_id: linkedHolidayCalendarId || null,
        }),
      });
      await refresh();
      onWeekChange?.();
      notify('success', 'Feiertagskalender verknüpft — Stundenzettel werden neu berechnet');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function saveRotationSlots() {
    if (!selectedRotationPlanId) {
      notify('error', 'Umschaltplan wählen');
      return;
    }
    savingRotation = true;
    try {
      await api(apiUrl, `/api/v1/time/work-rotation-plans/${selectedRotationPlanId}/slots`, {
        method: 'PUT',
        body: JSON.stringify({ slots: rotationSlotEdits }),
      });
      await refresh();
      onWeekChange?.();
      notify('success', 'Umschaltplan gespeichert');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    } finally {
      savingRotation = false;
    }
  }

  function syncEditFromModel(id: string) {
    const m = workdayModels.find((x) => x.id === id);
    if (!m) return;
    editModelName = m.name;
    editExpectedMinutes = m.config.expected_minutes ?? 0;
    editLabel = m.config.label ?? '';
    editFlexEarliest = m.config.flex_band?.earliest_start ?? '06:00';
    editFlexLatest = m.config.flex_band?.latest_end ?? '20:00';
  }

  function selectModel(id: string) {
    editModelId = id;
    syncEditFromModel(id);
    panel = 'tages';
  }

  function selectCalendarDay(d: { date: string; workday_model_id: string }) {
    selectedDayDate = d.date;
    dayEditModelId = d.workday_model_id;
    calendarDayOverride = { date: d.date, workday_model_id: d.workday_model_id };
  }

  let calendarDayOverride = $state({
    date: '',
    workday_model_id: 'wm-std-8h',
  });

  async function saveWorkdayModel() {
    const m = workdayModels.find((x) => x.id === editModelId);
    if (!m) {
      notify('error', 'Tagesperiode wählen');
      return;
    }
    const name = editModelName.trim();
    if (!name) {
      notify('error', 'Name erforderlich');
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
        body: JSON.stringify({ name, config }),
      });
      await refresh();
      onWeekChange?.();
      notify('success', 'Tagesperiode gespeichert — betroffene Stundenzettel werden neu berechnet');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    } finally {
      savingModel = false;
    }
  }

  async function createWorkdayModel() {
    const name = newModelName.trim();
    if (!name) {
      notify('error', 'Name für neue Tagesperiode eingeben');
      return;
    }
    creatingModel = true;
    try {
      const created = await api<{ id: string; name: string; config: WorkdayModelConfig }>(
        apiUrl,
        '/api/v1/time/workday-models',
        {
          method: 'POST',
          body: JSON.stringify({
            name,
            config: {
              expected_minutes: 480,
              label: null,
              flex_band: { earliest_start: '06:00', latest_end: '20:00' },
            },
          }),
        },
      );
      newModelName = '';
      await refresh();
      selectModel(created.id);
      notify('success', `Tagesperiode „${created.name}“ angelegt`);
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    } finally {
      creatingModel = false;
    }
  }

  async function createWorkCalendar() {
    const name = newCalendarName.trim();
    if (!name) {
      notify('error', 'Name für Jahresperiode eingeben');
      return;
    }
    creatingCalendar = true;
    try {
      const created = await api<{ id: string; name: string }>(apiUrl, '/api/v1/time/work-calendars', {
        method: 'POST',
        body: JSON.stringify({ name }),
      });
      newCalendarName = '';
      await refresh();
      selectedWorkCalendarId = created.id;
      panel = 'jahres';
      notify('success', `Jahresperiode „${created.name}“ angelegt — Jahr befüllen`);
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    } finally {
      creatingCalendar = false;
    }
  }

  $effect(() => {
    if (workdayModels.length > 0) {
      if (!workdayModels.some((m) => m.id === editModelId)) {
        editModelId = workdayModels[0].id;
      }
      syncEditFromModel(editModelId);
      if (!workdayModels.some((m) => m.id === dayEditModelId)) {
        dayEditModelId = workdayModels[0].id;
      }
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
    if (
      selectedDayDate &&
      !workCalendarDays.some((d) => d.date === selectedDayDate)
    ) {
      selectedDayDate = workCalendarDays[0]?.date ?? '';
    }
    if (!selectedDayDate && workCalendarDays[0]) {
      selectedDayDate = workCalendarDays[0].date;
      dayEditModelId = workCalendarDays[0].workday_model_id;
    }
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
    holidayCalendars = await api<typeof holidayCalendars>(
      apiUrl,
      '/api/v1/time/holiday-calendars',
    ).catch(() => []);
    rotationPlans = await api<RotationPlan[]>(
      apiUrl,
      '/api/v1/time/work-rotation-plans',
    ).catch(() => []);
    if (workCalendars.length && !workCalendars.some((c) => c.id === selectedWorkCalendarId)) {
      selectedWorkCalendarId = workCalendars[0].id;
    }
    syncLinkedHolidayFromCalendar();
    if (rotationPlans.length && !rotationPlans.some((p) => p.id === selectedRotationPlanId)) {
      selectedRotationPlanId = rotationPlans[0].id;
    }
    syncRotationSlotEdits();
    if (panel === 'feiertage') await loadHolidayDays();
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

  async function generateWorkCalendarYearFor(year: number) {
    const res = await api<{ inserted: number }>(
      apiUrl,
      `/api/v1/time/work-calendars/${selectedWorkCalendarId}/generate-year`,
      { method: 'POST', body: JSON.stringify({ year }) },
    );
    return res.inserted;
  }

  async function generateWorkCalendarYear() {
    if (!selectedWorkCalendarId) {
      notify('error', 'Jahresperiode wählen');
      return;
    }
    try {
      const inserted = await generateWorkCalendarYearFor(calendarGenYear);
      await refresh();
      onWeekChange?.();
      notify('success', `${inserted} Kalendertage für ${calendarGenYear} ergänzt`);
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function generateWorkCalendarTwoYears() {
    if (!selectedWorkCalendarId) {
      notify('error', 'Jahresperiode wählen');
      return;
    }
    try {
      const y = calendarGenYear;
      const a = await generateWorkCalendarYearFor(y);
      const b = await generateWorkCalendarYearFor(y + 1);
      await refresh();
      onWeekChange?.();
      notify('success', `${a + b} Kalendertage für ${y} und ${y + 1} ergänzt`);
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function saveSelectedDay() {
    if (!selectedWorkCalendarId || !selectedDayDate) {
      notify('error', 'Tag und Jahresperiode wählen');
      return;
    }
    try {
      await api(
        apiUrl,
        `/api/v1/time/work-calendars/${selectedWorkCalendarId}/days/${selectedDayDate}`,
        {
          method: 'PUT',
          body: JSON.stringify({ workday_model_id: dayEditModelId }),
        },
      );
      await refreshWorkCalendarWeekDays();
      onWeekChange?.();
      notify('success', `Tag ${selectedDayDate} gespeichert`);
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
      notify('success', 'Zuordnung gespeichert');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function setCalendarRotation() {
    if (!selectedWorkCalendarId) {
      notify('error', 'Jahresperiode wählen');
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
          ? 'Umschaltplan aktiv'
          : 'Umschaltplan entfernt',
      );
      await refresh();
      onWeekChange?.();
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function copyWorkCalendarWeek() {
    if (!selectedWorkCalendarId) {
      notify('error', 'Jahresperiode wählen');
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
  const selectedDay = $derived(
    workCalendarDays.find((d) => d.date === selectedDayDate) ?? null,
  );
</script>

<section class="period-shell">
  <header class="period-header">
    <div>
      <h3>Perioden & Sollzeit</h3>
      <p class="muted period-lead">
        <strong>Tagesperioden</strong> definieren Soll, Gleitzeit und Pausen.
        <strong>Jahresperioden</strong> ordnen jedem Tag eine Tagesperiode zu.
        Schichtvorlagen sind nur Planung — Soll kommt hierher.
      </p>
    </div>
  </header>

  <nav class="period-tabs" aria-label="Perioden-Bereiche">
    <button type="button" class:active={panel === 'tages'} onclick={() => (panel = 'tages')}>
      Tagesperioden
      <span class="tab-count">{workdayModels.length}</span>
    </button>
    <button type="button" class:active={panel === 'jahres'} onclick={() => (panel = 'jahres')}>
      Jahresperioden
      <span class="tab-count">{workCalendars.length}</span>
    </button>
    <button
      type="button"
      class:active={panel === 'feiertage'}
      onclick={() => {
        panel = 'feiertage';
        void loadHolidayDays();
      }}
    >
      Feiertage
      <span class="tab-count">{holidayCalendars.length}</span>
    </button>
    <button
      type="button"
      class:active={panel === 'umschalt'}
      onclick={() => (panel = 'umschalt')}
    >
      Umschaltplan
      <span class="tab-count">{rotationPlans.length}</span>
    </button>
    <button
      type="button"
      class:active={panel === 'zuordnungen'}
      onclick={() => (panel = 'zuordnungen')}
    >
      MA-Zuordnung
      <span class="tab-count">{workAssignments.length}</span>
    </button>
  </nav>

  {#if panel === 'tages'}
    <div class="period-split">
      <div class="period-list-pane card-inner">
        <h4>Tagesperioden</h4>
        <ul class="pick-list" role="listbox">
          {#each workdayModels as m}
            <li>
              <button
                type="button"
                class="pick-item"
                class:selected={editModelId === m.id}
                onclick={() => selectModel(m.id)}
              >
                <span class="pick-title">{m.name}</span>
                <span class="pick-meta muted">
                  {formatMinutes(m.config.expected_minutes ?? 0)} Soll
                  {#if m.config.flex_band}
                    · {m.config.flex_band.earliest_start}–{m.config.flex_band.latest_end}
                  {/if}
                </span>
              </button>
            </li>
          {:else}
            <li class="muted">Keine Tagesperioden.</li>
          {/each}
        </ul>
        <div class="inline-form">
          <input bind:value={newModelName} placeholder="Neue Tagesperiode…" />
          <button type="button" disabled={creatingModel} onclick={createWorkdayModel}>
            {creatingModel ? '…' : '+ Anlegen'}
          </button>
        </div>
      </div>

      <div class="period-editor-pane card-inner">
        {#if workdayModels.length > 0}
          <h4>Bearbeiten</h4>
          <div class="editor-grid">
            <label class="field">
              <span class="field-label">Name</span>
              <input bind:value={editModelName} />
            </label>
            <label class="field">
              <span class="field-label">Soll (Minuten)</span>
              <input
                type="number"
                bind:value={editExpectedMinutes}
                min="0"
                max="720"
                step="15"
              />
            </label>
            <label class="field">
              <span class="field-label">Bezeichnung (optional)</span>
              <input bind:value={editLabel} />
            </label>
            <label class="field">
              <span class="field-label">Gleitzeit von</span>
              <input bind:value={editFlexEarliest} placeholder="HH:MM" />
            </label>
            <label class="field">
              <span class="field-label">Gleitzeit bis</span>
              <input bind:value={editFlexLatest} placeholder="HH:MM" />
            </label>
          </div>
          <div class="btn-row">
            <button type="button" disabled={savingModel} onclick={saveWorkdayModel}>
              {savingModel ? 'Speichern…' : 'Speichern'}
            </button>
          </div>
          <p class="muted fine-print">
            Änderungen an Soll/Gleitzeit lösen Neuberechnung der Stundenzettel aus.
          </p>
        {:else}
          <p class="muted">Legen Sie zuerst eine Tagesperiode an.</p>
        {/if}
      </div>
    </div>
  {:else if panel === 'jahres'}
    <div class="card-inner">
      <div class="toolbar-row">
        <label class="field grow">
          <span class="field-label">Jahresperiode</span>
          <select
            bind:value={selectedWorkCalendarId}
            onchange={() => {
              selectedDayDate = '';
              void refreshWorkCalendarWeekDays();
            }}
          >
            {#each workCalendars as c}
              <option value={c.id}>{c.name}</option>
            {/each}
          </select>
        </label>
        <div class="inline-form">
          <input bind:value={newCalendarName} placeholder="Neue Jahresperiode…" />
          <button
            type="button"
            class="secondary"
            disabled={creatingCalendar}
            onclick={createWorkCalendar}
          >
            {creatingCalendar ? '…' : '+ Anlegen'}
          </button>
        </div>
        <label class="field" style="margin-top: 0.75rem;">
          <span class="field-label">Feiertagskalender (für diese Jahresperiode)</span>
          <select bind:value={linkedHolidayCalendarId}>
            <option value="">Keiner</option>
            {#each holidayCalendars as h}
              <option value={h.id}>{h.name}</option>
            {/each}
          </select>
        </label>
        <button type="button" class="secondary" onclick={saveLinkedHoliday}>
          Feiertagskalender verknüpfen
        </button>
      </div>

      <div class="btn-row week-nav">
        <button class="secondary" type="button" onclick={() => shiftWeek(-1)}>← KW</button>
        <span class="muted">{weekLabelForAnchor(shiftWeekAnchor)}</span>
        <button class="secondary" type="button" onclick={() => shiftWeek(1)}>KW →</button>
        <button class="secondary" type="button" onclick={goToThisWeek}>Diese Woche</button>
      </div>

      <div class="week-calendar period-week">
        {#each workCalendarDays as d}
          <button
            type="button"
            class="day-col day-col-btn"
            class:selected={selectedDayDate === d.date}
            onclick={() => selectCalendarDay(d)}
          >
            <span class="day-num">{d.date.slice(8, 10)}.{d.date.slice(5, 7)}</span>
            <span class="shift-chip published">{d.model_name}</span>
          </button>
        {:else}
          <p class="muted span-all">
            Keine Tage in dieser Woche — Jahr befüllen oder andere KW wählen.
          </p>
        {/each}
      </div>

      {#if selectedDay}
        <div class="day-editor card-inner subtle">
          <h4>
            {selectedDay.date} — Tagesperiode zuweisen
          </h4>
          <div class="inline-form">
            <select bind:value={dayEditModelId}>
              {#each workdayModels as m}
                <option value={m.id}>{m.name}</option>
              {/each}
            </select>
            <button type="button" onclick={saveSelectedDay}>Tag speichern</button>
            <button
              type="button"
              class="secondary"
              onclick={() => selectModel(selectedDay.workday_model_id)}
            >
              Tagesperiode bearbeiten →
            </button>
          </div>
        </div>
      {/if}

      <details class="period-advanced">
        <summary>Jahr befüllen, kopieren, Umschaltplan</summary>
        <div class="toolbar-row" style="margin-top: 0.75rem;">
          <input type="number" bind:value={calendarGenYear} min="2020" max="2035" />
          <button class="secondary" type="button" onclick={generateWorkCalendarYear}>
            Jahr befüllen (Mo–Fr)
          </button>
          <button class="secondary" type="button" onclick={generateWorkCalendarTwoYears}>
            {calendarGenYear} + {calendarGenYear + 1}
          </button>
          <button class="secondary" type="button" onclick={copyWorkCalendarWeek}>
            KW → KW+1 kopieren
          </button>
        </div>
        <div class="inline-form" style="margin-top: 0.5rem;">
          <select bind:value={calendarRotationPlanId}>
            <option value="">Kein Umschaltplan</option>
            {#each rotationPlans as p}
              <option value={p.id}>{p.name} ({p.cycle_days} Tage)</option>
            {/each}
          </select>
          <button class="secondary" type="button" onclick={setCalendarRotation}>
            Umschaltplan zuweisen
          </button>
        </div>
      </details>
    </div>
  {:else if panel === 'feiertage'}
    <div class="card-inner">
      <p class="ts-lead" style="margin-bottom: 0.75rem;">
        Feiertage überschreiben den Jahreskalender. Verknüpfung pro Jahresperiode unter Tab
        <button type="button" class="linkish" onclick={() => (panel = 'jahres')}>Jahresperioden</button>.
      </p>
      <div class="toolbar-row">
        <label class="field grow">
          <span class="field-label">Feiertagskalender anzeigen</span>
          <select
            bind:value={linkedHolidayCalendarId}
            onchange={() => loadHolidayDays()}
          >
            <option value="">— wählen —</option>
            {#each holidayCalendars as h}
              <option value={h.id}>{h.name} ({h.year_from}–{h.year_to})</option>
            {/each}
          </select>
        </label>
        <input type="number" bind:value={holidayViewYear} min="2020" max="2035" />
        <button class="secondary" type="button" onclick={() => loadHolidayDays()}>Laden</button>
      </div>
      <ul class="pick-list" style="max-height: 360px; margin-top: 0.75rem;">
        {#each holidayDays as h}
          <li class="pick-item" style="cursor: default;">
            <span class="pick-title">{h.date}</span>
            <span class="pick-meta muted">
              {h.name ?? h.day_kind}
              {#if h.model_name} · {h.model_name}{/if}
            </span>
          </li>
        {:else}
          <li class="muted">
            {#if linkedHolidayCalendarId}
              Keine Feiertage in {holidayViewYear} — Seed prüfen oder Jahr wechseln.
            {:else}
              Kalender wählen oder unter Jahresperioden verknüpfen.
            {/if}
          </li>
        {/each}
      </ul>
    </div>
  {:else if panel === 'umschalt'}
    <div class="card-inner">
      <p class="ts-lead" style="margin-bottom: 0.75rem;">
        Zyklische Tagesperioden (z. B. Schichtwechsel). Zuweisung an eine Jahresperiode unter
        <button type="button" class="linkish" onclick={() => (panel = 'jahres')}>Jahresperioden</button>
        → Erweitert.
      </p>
      <label class="field">
        <span class="field-label">Plan</span>
        <select
          bind:value={selectedRotationPlanId}
          onchange={() => syncRotationSlotEdits()}
        >
          {#each rotationPlans as p}
            <option value={p.id}>{p.name} ({p.cycle_days} Tage, ab {p.anchor_date})</option>
          {/each}
        </select>
      </label>
      {#if selectedRotationPlanId}
        {@const plan = rotationPlans.find((p) => p.id === selectedRotationPlanId)}
        {#if plan}
          <div class="editor-grid" style="margin-top: 0.75rem;">
            {#each Array(plan.cycle_days) as _, i}
              {@const idx = i}
              {@const slot = rotationSlotEdits.find((s) => s.slot_index === idx)}
              <label class="field">
                <span class="field-label">Tag {idx + 1} im Zyklus</span>
                <select
                  value={slot?.workday_model_id ?? 'wm-std-8h'}
                  onchange={(e) => {
                    const v = (e.currentTarget as HTMLSelectElement).value;
                    const rest = rotationSlotEdits.filter((s) => s.slot_index !== idx);
                    rotationSlotEdits = [...rest, { slot_index: idx, workday_model_id: v }];
                  }}
                >
                  {#each workdayModels as m}
                    <option value={m.id}>{m.name}</option>
                  {/each}
                </select>
              </label>
            {/each}
          </div>
          <div class="btn-row">
            <button type="button" disabled={savingRotation} onclick={saveRotationSlots}>
              {savingRotation ? 'Speichern…' : 'Slots speichern'}
            </button>
          </div>
        {/if}
      {/if}
    </div>
  {:else}
    <div class="card-inner">
      <p class="muted">
        Welche Jahresperiode gilt für welchen Mitarbeiter ab welchem Datum (inkl. Teilzeit %).
      </p>
      <div class="editor-grid">
        <label class="field">
          <span class="field-label">Filter</span>
          <select bind:value={workAssignmentEmployeeFilter} onchange={() => refresh()}>
            <option value="">Alle Mitarbeiter</option>
            {#each activeEmployees as e}
              <option value={e.id}>{e.employee_no} — {e.display_name}</option>
            {/each}
          </select>
        </label>
        <label class="field">
          <span class="field-label">Mitarbeiter</span>
          <select bind:value={newWorkAssignment.employee_id}>
            <option value="">Wählen…</option>
            {#each activeEmployees as e}
              <option value={e.id}>{e.employee_no} — {e.display_name}</option>
            {/each}
          </select>
        </label>
        <label class="field">
          <span class="field-label">Jahresperiode</span>
          <select bind:value={newWorkAssignment.work_calendar_id}>
            {#each workCalendars as c}
              <option value={c.id}>{c.name}</option>
            {/each}
          </select>
        </label>
        <label class="field">
          <span class="field-label">Gültig ab</span>
          <input type="date" bind:value={newWorkAssignment.valid_from} />
        </label>
        <label class="field">
          <span class="field-label">Teilzeit %</span>
          <input
            type="number"
            bind:value={newWorkAssignment.part_time_percent}
            min="1"
            max="100"
          />
        </label>
      </div>
      <div class="btn-row">
        <button type="button" onclick={createWorkAssignment}>Zuordnung speichern</button>
      </div>
      <ul class="assignment-list">
        {#each workAssignments as a}
          <li class="assignment-row">
            <button
              type="button"
              class="assignment-row-btn"
              onclick={() => {
                newWorkAssignment.employee_id = a.employee_id;
                newWorkAssignment.work_calendar_id = a.work_calendar_id;
                newWorkAssignment.valid_from = a.valid_from.slice(0, 10);
                newWorkAssignment.part_time_percent = a.part_time_percent;
              }}
            >
              <strong>{a.employee_no} {a.employee_name}</strong>
              <span class="muted">→ {a.work_calendar_name}</span>
              <span class="muted">ab {a.valid_from.slice(0, 10)}, {a.part_time_percent}%</span>
            </button>
          </li>
        {:else}
          <li class="muted">Noch keine Zuordnungen.</li>
        {/each}
      </ul>
    </div>
  {/if}
</section>
