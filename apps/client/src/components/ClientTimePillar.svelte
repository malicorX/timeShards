<script lang="ts">
  import { api, downloadFile } from '../lib/api';
  import { formatMinutes } from '../lib/formatMinutes';
  import { statusLabel } from '../lib/statusLabels';
  import TsPageHeader from '@timeshards/shared/ui/TsPageHeader.svelte';
  import TsCard from '@timeshards/shared/ui/TsCard.svelte';
  import {
    anchorFromPeriodStart,
    weekRangeContaining,
    formatDayLabel,
    formatIsoShort,
    formatPeriodRange,
    calendarWeekLabel,
    weekLabelForAnchor,
    shiftOnDay,
  } from '../lib/datetime';

  type WorkSummary = {
    pending_timesheets?: number | null;
    pending_absences?: number | null;
    draft_timesheets?: number | null;
    team_draft_timesheets?: number | null;
    my_pending_absences?: number | null;
    work_calendar_assigned?: boolean | null;
    current_week?: {
      period_start: string;
      status: string;
      worked_minutes: number;
      expected_minutes: number;
      balance_minutes: number;
      work_calendar_name?: string | null;
    } | null;
  };

  type TimesheetRow = {
    id: string;
    employee_id?: string;
    employee_name?: string;
    worked_minutes: number;
    expected_minutes?: number;
    balance_minutes?: number;
    overtime_minutes: number;
    status: string;
    period_start?: string;
    period_end?: string;
    rejection_reason?: string;
    evaluation?: {
      work_calendar_name: string;
      days: {
        date: string;
        model_name: string;
        expected_minutes: number;
        worked_minutes: number;
        balance_minutes: number;
        absence_label?: string;
      }[];
    };
  };

  type UiMessage = { type: 'error' | 'success'; text: string };

  let {
    serverUrl,
    active = false,
    workSummary = null,
    canApprove = false,
    ownPendingAbsences = 0,
    onMessage,
    onNavigate,
    onRefreshParent,
  }: {
    serverUrl: string;
    active?: boolean;
    workSummary?: WorkSummary | null;
    canApprove?: boolean;
    ownPendingAbsences?: number;
    onMessage?: (msg: UiMessage) => void;
    onNavigate?: (pillar: 'approvals' | 'absence') => void;
    onRefreshParent?: () => void | Promise<void>;
  } = $props();

  const weekdayLabels: Record<number, string> = {
    1: 'Mo',
    2: 'Di',
    3: 'Mi',
    4: 'Do',
    5: 'Fr',
    6: 'Sa',
    7: 'So',
  };

  let timeStatus = $state<{
    is_clocked_in: boolean;
    is_on_break: boolean;
    last_kind: string | null;
  } | null>(null);
  let timeEvents = $state<{ id: string; kind: string; occurred_at: string }[]>([]);
  let timesheets = $state<TimesheetRow[]>([]);
  let timesheetWeekOnly = $state(true);
  let timesheetFilter = $state<'all' | 'draft' | 'pending' | 'approved' | 'rejected'>('all');
  let shiftFilter = $state<'all' | 'planned' | 'published' | 'cancelled'>('all');
  let shiftWeekAnchor = $state(new Date());
  let myShifts = $state<{ id: string; starts_at: string; ends_at: string; status: string }[]>([]);
  let myShiftTemplates = $state<
    { weekday: number; starts_time: string; ends_time: string; name: string }[]
  >([]);
  let expandedTimesheetId = $state('');
  let rebuildWarnings = $state<string[]>([]);

  const shiftWeek = $derived(weekRangeContaining(shiftWeekAnchor));

  function notify(type: 'error' | 'success', text: string) {
    onMessage?.({ type, text });
  }

  function shiftsForDay(day: Date) {
    return myShifts.filter((s) => shiftOnDay(s.starts_at, day));
  }

  async function syncWeekAnchorFromApi() {
    const w = await api<{ period_start: string }>(serverUrl, '/api/v1/time/calendar-week').catch(
      () => null,
    );
    const anchor = w?.period_start ? anchorFromPeriodStart(w.period_start) : null;
    if (anchor) shiftWeekAnchor = anchor;
  }

  export async function refresh() {
    await syncWeekAnchorFromApi();
    timeStatus = await api<typeof timeStatus>(serverUrl, '/api/v1/time/status').catch(() => null);
    timeEvents = await api<typeof timeEvents>(serverUrl, '/api/v1/time/events?limit=30').catch(
      () => [],
    );
    const tsParams = new URLSearchParams();
    if (timesheetFilter !== 'all') tsParams.set('status', timesheetFilter);
    if (timesheetWeekOnly) {
      const { from } = weekRangeContaining(shiftWeekAnchor);
      tsParams.set('period_start', from);
    }
    const tsQs = tsParams.toString();
    timesheets = await api<TimesheetRow[]>(
      serverUrl,
      tsQs ? `/api/v1/time/timesheets?${tsQs}` : '/api/v1/time/timesheets',
    ).catch(() => []);
    const { from, to } = weekRangeContaining(shiftWeekAnchor);
    const shiftParams = new URLSearchParams({ from, to });
    if (shiftFilter !== 'all') shiftParams.set('status', shiftFilter);
    myShifts = await api<typeof myShifts>(serverUrl, `/api/v1/time/shifts?${shiftParams}`).catch(
      () => [],
    );
    myShiftTemplates = await api<typeof myShiftTemplates>(
      serverUrl,
      '/api/v1/time/shift-templates',
    ).catch(() => []);
  }

  function notifyPunchAdvisory(advisory?: string) {
    if (!advisory) return;
    if (advisory.includes('Arbeitskalender')) {
      notify('error', advisory);
    } else {
      notify('success', `Hinweis: ${advisory}`);
    }
  }

  async function afterTimeAction(advisory?: string) {
    await refresh();
    await onRefreshParent?.();
    notifyPunchAdvisory(advisory);
  }

  async function clockIn() {
    const res = await api<{ advisory?: string }>(serverUrl, '/api/v1/time/clock-in', {
      method: 'POST',
    });
    await afterTimeAction(res.advisory);
  }

  async function clockOut() {
    const res = await api<{ advisory?: string }>(serverUrl, '/api/v1/time/clock-out', {
      method: 'POST',
    });
    await afterTimeAction(res.advisory);
  }

  async function breakStart() {
    await api(serverUrl, '/api/v1/time/break-start', { method: 'POST' });
    await afterTimeAction();
  }

  async function breakEnd() {
    await api(serverUrl, '/api/v1/time/break-end', { method: 'POST' });
    await afterTimeAction();
  }

  async function submitTimesheet(id: string) {
    await api(serverUrl, `/api/v1/time/timesheets/${id}/submit`, { method: 'POST' });
    await afterTimeAction('Stundenzettel eingereicht');
  }

  async function submitAllDraftTimesheets() {
    try {
      const params = new URLSearchParams();
      if (timesheetWeekOnly) {
        const { from } = weekRangeContaining(shiftWeekAnchor);
        params.set('period_start', from);
      }
      const qs = params.toString();
      const path = qs
        ? `/api/v1/time/timesheets/submit-drafts?${qs}`
        : '/api/v1/time/timesheets/submit-drafts';
      const res = await api<{ submitted: number }>(serverUrl, path, { method: 'POST' });
      await afterTimeAction(
        res.submitted > 0
          ? `${res.submitted} Stundenzettel eingereicht`
          : 'Keine Entwürfe zum Einreichen',
      );
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function exportMyTimesheets() {
    try {
      const status = timesheetFilter === 'all' ? 'draft' : timesheetFilter;
      let path = `/api/v1/reports/timesheets/export?format=csv&status=${encodeURIComponent(status)}`;
      if (timesheetWeekOnly) {
        const { from } = weekRangeContaining(shiftWeekAnchor);
        path += `&period_start=${encodeURIComponent(from)}`;
      }
      await downloadFile(serverUrl, path, `stundenzettel_${status}.csv`);
      notify('success', 'CSV exportiert');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function rebuildMyTimesheets() {
    rebuildWarnings = [];
    try {
      const { from } = weekRangeContaining(shiftWeekAnchor);
      const res = await api<{ updated: number; warnings?: string[] }>(
        serverUrl,
        `/api/v1/time/timesheets/rebuild?week_start=${encodeURIComponent(from)}`,
        { method: 'POST' },
      );
      rebuildWarnings = res.warnings ?? [];
      const w = rebuildWarnings.length ? ` (${rebuildWarnings.length} ArbZG-Hinweise)` : '';
      await afterTimeAction(`${res.updated} Stundenzettel berechnet${w}`);
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
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
    if (active) void refresh();
  });
</script>

<TsPageHeader
  title="Zeiterfassung"
  lead="Stempeln, Kalenderwoche mit Soll/Ist und Stundenzettel. Sollzeit kommt aus Ihrer Jahresperiode (HR pflegt Perioden im Server)."
/>
{#if workSummary && ((workSummary.pending_timesheets ?? 0) + (workSummary.pending_absences ?? 0) + (workSummary.draft_timesheets ?? 0) + ownPendingAbsences) > 0}
  <p class="muted" style="margin-bottom: 0.75rem;">
    {#if canApprove && ((workSummary.pending_timesheets ?? 0) + (workSummary.pending_absences ?? 0)) > 0}
      Offene Freigaben:
      {#if workSummary.pending_timesheets}
        {workSummary.pending_timesheets} Stundenzettel
      {/if}
      {#if workSummary.pending_timesheets && workSummary.pending_absences}
        ·
      {/if}
      {#if workSummary.pending_absences}
        {workSummary.pending_absences} Abwesenheiten
      {/if}
      — Tab <button class="secondary" type="button" onclick={() => onNavigate?.('approvals')}>
        Freigaben
      </button>
    {/if}
    {#if canApprove && (workSummary.team_draft_timesheets ?? 0) > 0}
      {#if (workSummary.pending_timesheets ?? 0) + (workSummary.pending_absences ?? 0) > 0}
        <br />
      {/if}
      {workSummary.team_draft_timesheets} Team-Entwürfe —
      <button class="secondary" type="button" onclick={() => onNavigate?.('approvals')}>
        Freigaben
      </button>
    {/if}
    {#if (workSummary.draft_timesheets ?? 0) > 0}
      {#if canApprove && ((workSummary.pending_timesheets ?? 0) + (workSummary.pending_absences ?? 0) + (workSummary.team_draft_timesheets ?? 0)) > 0}
        <br />
      {/if}
      {workSummary.draft_timesheets} eigene Entwürfe —
      <button class="secondary" type="button" onclick={submitAllDraftTimesheets}>
        Alle einreichen
      </button>
    {/if}
    {#if ownPendingAbsences > 0}
      {#if canApprove && ((workSummary.pending_timesheets ?? 0) + (workSummary.pending_absences ?? 0) + (workSummary.team_draft_timesheets ?? 0) + (workSummary.draft_timesheets ?? 0)) > 0}
        <br />
      {/if}
      {ownPendingAbsences} Abwesenheit(en) offen —
      <button class="secondary" type="button" onclick={() => onNavigate?.('absence')}>
        Abwesenheit
      </button>
    {/if}
  </p>
{/if}
{#if timeStatus}
  <TsCard title="Stempeluhr" lead="Aktueller Status und Hauptaktionen">
    <div class="ts-punch-bar">
      <span
        class="status-pill"
        class:on={timeStatus.is_clocked_in && !timeStatus.is_on_break}
        class:break={timeStatus.is_on_break}
        class:off={!timeStatus.is_clocked_in && !timeStatus.is_on_break}
      >
        {#if timeStatus.is_on_break}
          Pause
        {:else if timeStatus.is_clocked_in}
          Eingestempelt
        {:else}
          Ausgestempelt
        {/if}
      </span>
      {#if !timeStatus.is_clocked_in}
        <button type="button" onclick={clockIn}>Kommen</button>
      {:else if timeStatus.is_on_break}
        <button type="button" onclick={breakEnd}>Weiterarbeiten</button>
        <button class="secondary" type="button" onclick={clockOut}>Gehen</button>
      {:else}
        <button class="secondary" type="button" onclick={breakStart}>Pause</button>
        <button type="button" onclick={clockOut}>Gehen</button>
      {/if}
    </div>
    {#if workSummary?.work_calendar_assigned === false}
      <p class="error" style="margin-top: 0.75rem; font-size: 0.9rem;">
        Kein Arbeitskalender — Sollzeit fehlt. Bitte HR kontaktieren.
      </p>
    {:else if workSummary?.current_week && workSummary.current_week.expected_minutes > 0}
      <div class="ts-status-hero" style="margin-top: 1rem;">
        <div class="stat">
          <span class="label">Ist (KW)</span>
          <span class="value">{formatMinutes(workSummary.current_week.worked_minutes)}</span>
        </div>
        <div class="stat">
          <span class="label">Soll</span>
          <span class="value">{formatMinutes(workSummary.current_week.expected_minutes)}</span>
        </div>
        <div class="stat">
          <span class="label">Saldo</span>
          <span
            class="value"
            class:warn={workSummary.current_week.balance_minutes < 0}
            class:ok={workSummary.current_week.balance_minutes >= 0}
          >
            {formatMinutes(workSummary.current_week.balance_minutes)}
          </span>
        </div>
        <div class="stat">
          <span class="label">Stundenzettel</span>
          <span class="value">{statusLabel(workSummary.current_week.status)}</span>
        </div>
      </div>
      {#if workSummary.current_week.work_calendar_name}
        <p class="muted fine-print">Kalender: {workSummary.current_week.work_calendar_name}</p>
      {/if}
    {/if}
  </TsCard>
{/if}
<TsCard title="Stundenzettel" lead="Filter, Tagesdetails und Einreichen">
  <div class="btn-row" style="margin-bottom: 0.5rem;">
    <select bind:value={timesheetFilter} onchange={() => refresh()}>
      <option value="all">Alle Status</option>
      <option value="draft">Entwurf</option>
      <option value="pending">Eingereicht</option>
      <option value="approved">Freigegeben</option>
      <option value="rejected">Abgelehnt</option>
    </select>
    <label class="muted">
      <input type="checkbox" bind:checked={timesheetWeekOnly} onchange={() => refresh()} />
      Nur Kalenderwoche
    </label>
    <button class="secondary" type="button" onclick={exportMyTimesheets}>CSV exportieren</button>
  </div>
  <p class="muted" style="margin: 0 0 0.5rem; font-size: 0.85rem;">
    Export nutzt den Status-Filter (bei „Alle“: Entwürfe).
  </p>
  {#each timesheets as t}
    <div class="row-card" style="margin-bottom: 0.5rem;">
      <p>
        {#if t.period_start}
          <span class="muted">
            {calendarWeekLabel(t.period_start)} {formatPeriodRange(t.period_start, t.period_end)} ·
          </span>
        {/if}
        Ist {formatMinutes(t.worked_minutes)}
        {#if t.expected_minutes != null && t.expected_minutes > 0}
          · Soll {formatMinutes(t.expected_minutes)}
          · Saldo {formatMinutes(t.balance_minutes ?? t.worked_minutes - t.expected_minutes)}
        {/if}
        · ÜS {formatMinutes(t.overtime_minutes)} · {statusLabel(t.status)}
      </p>
      {#if t.rejection_reason}
        <p class="muted">Ablehnung: {t.rejection_reason}</p>
      {/if}
      {#if t.evaluation?.days?.length}
        <button
          class="secondary"
          type="button"
          onclick={() => (expandedTimesheetId = expandedTimesheetId === t.id ? '' : t.id)}
        >
          {expandedTimesheetId === t.id ? 'Tagesdetails aus' : 'Tagesdetails'}
        </button>
      {/if}
      {#if expandedTimesheetId === t.id && t.evaluation?.days}
        <ul class="compact-list" style="margin-top: 0.35rem; font-size: 0.85rem;">
          {#each t.evaluation.days as d}
            <li>
              {d.date} · {d.model_name}
              {#if d.absence_label}<span class="muted"> ({d.absence_label})</span>{/if}
              — Soll {formatMinutes(d.expected_minutes)}, Ist {formatMinutes(d.worked_minutes)}
            </li>
          {/each}
        </ul>
      {/if}
      {#if t.status === 'draft' || t.status === 'rejected'}
        <button class="secondary" type="button" onclick={() => submitTimesheet(t.id)}>
          Einreichen
        </button>
      {/if}
    </div>
  {/each}
  {#if timesheets.some((t) => t.status === 'draft' || t.status === 'rejected')}
    <button class="secondary" type="button" style="margin-top: 0.5rem;" onclick={submitAllDraftTimesheets}>
      Alle Entwürfe einreichen
    </button>
  {/if}
  {#if timesheets.length === 0}
    <div class="ts-empty">
      <p>Keine Stundenzettel in dieser Ansicht — Filter oder Kalenderwoche prüfen.</p>
    </div>
  {/if}
  {#if timeEvents.length > 0 && (timesheets.length === 0 || timesheets.some((t) => (t.expected_minutes ?? 0) === 0 && (t.status === 'draft' || t.status === 'rejected')))}
    <button class="secondary" type="button" style="margin-top: 0.5rem;" onclick={rebuildMyTimesheets}>
      Manuell neu berechnen (Kalender/Soll)
    </button>
  {/if}
  {#if rebuildWarnings.length > 0}
    <div style="margin-top: 0.75rem;">
      <p class="muted">ArbZG-Hinweise:</p>
      <ul class="compact-list">
        {#each rebuildWarnings as w}<li>{w}</li>{/each}
      </ul>
    </div>
  {/if}
</TsCard>
{#if myShiftTemplates.length > 0}
  <div class="card" style="margin-top: 1rem;">
    <h3>Wochenvorlagen</h3>
    <ul class="compact-list">
      {#each myShiftTemplates as t}
        <li>
          {weekdayLabels[t.weekday] ?? t.weekday} {t.starts_time}–{t.ends_time}
          <span class="muted"> ({t.name})</span>
        </li>
      {/each}
    </ul>
    <p class="muted">Geplante Schichten — nicht Sollzeit (Arbeitskalender).</p>
  </div>
{/if}
<div class="card" style="margin-top: 1rem;">
  <h3>Meine Schichten (Woche)</h3>
  <select
    bind:value={shiftFilter}
    onchange={() => refresh()}
    style="margin-bottom: 0.5rem; max-width: 240px;"
  >
    <option value="all">Alle Status</option>
    <option value="planned">Geplant</option>
    <option value="published">Veröffentlicht</option>
    <option value="cancelled">Storniert</option>
  </select>
  <div class="btn-row" style="margin-bottom: 0.5rem;">
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
          <div class="shift-chip {s.status}" title={statusLabel(s.status)}>
            {new Date(s.starts_at).toLocaleTimeString('de-DE', {
              hour: '2-digit',
              minute: '2-digit',
            })}
            –
            {new Date(s.ends_at).toLocaleTimeString('de-DE', {
              hour: '2-digit',
              minute: '2-digit',
            })}
            <span class="muted"> ({statusLabel(s.status)})</span>
          </div>
        {/each}
      </div>
    {/each}
  </div>
</div>
<div class="card" style="margin-top: 1rem;">
  <h3>Stempelungen</h3>
  <ul>
    {#each timeEvents as ev}
      <li>{statusLabel(ev.kind)} — {formatIsoShort(ev.occurred_at)}</li>
    {/each}
  </ul>
</div>
