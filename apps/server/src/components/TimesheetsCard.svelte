<script lang="ts">
  import { api, downloadFile, openHtmlExport } from '../lib/api';
  import { formatMinutes } from '../lib/formatMinutes';
  import { statusLabel } from '../lib/statusLabels';
  import {
    toLocalDatetimeInputValue,
    fromLocalDatetimeInputValue,
    formatIsoLocalShort,
    calendarWeekLabel,
    formatPeriodRange,
    weekLabelForAnchor,
    weekRangeContaining,
  } from '../lib/datetime';
  import TimeSettlementCard from './TimeSettlementCard.svelte';

  type Employee = {
    id: string;
    employee_no: string;
    display_name: string;
    active?: boolean;
  };

  type TimesheetRow = {
    id: string;
    employee_id: string;
    employee_no: string;
    employee_name: string;
    worked_minutes: number;
    expected_minutes?: number;
    balance_minutes?: number;
    overtime_minutes: number;
    period_start: string;
    period_end?: string;
    status: string;
    rejection_reason?: string;
    evaluation?: {
      work_calendar_name: string;
      part_time_percent: number;
      days: {
        date: string;
        model_name: string;
        expected_minutes: number;
        worked_minutes: number;
        balance_minutes: number;
        absence_label?: string;
        warnings: string[];
      }[];
    };
  };

  type UiMessage = { type: 'error' | 'success'; text: string };

  let {
    apiUrl,
    employees,
    user,
    shiftWeekAnchor = $bindable(new Date()),
    timesheetFilter = $bindable<'all' | 'pending' | 'draft' | 'approved' | 'rejected'>('all'),
    active = false,
    canApprove = false,
    canCorrectTime = false,
    pendingTimesheets = 0,
    onMessage,
    onDashboardChange,
  }: {
    apiUrl: string;
    employees: Employee[];
    user: { employee_id?: string | null } | null;
    shiftWeekAnchor?: Date;
    timesheetFilter?: 'all' | 'pending' | 'draft' | 'approved' | 'rejected';
    active?: boolean;
    canApprove?: boolean;
    canCorrectTime?: boolean;
    pendingTimesheets?: number;
    onMessage?: (msg: UiMessage) => void;
    onDashboardChange?: () => void | Promise<void>;
  } = $props();

  let timesheets = $state<TimesheetRow[]>([]);
  let teamDraftRows = $state<TimesheetRow[]>([]);
  let expandedTimesheetId = $state('');
  let timesheetEmployeeFilter = $state('');
  let timesheetAllWeeks = $state(false);
  let exportTimesheetStatus = $state('approved');
  let rejectReason = $state('');
  let rebuildWarnings = $state<string[]>([]);
  let activePolicy = $state<{
    max_daily_minutes: number;
    max_weekly_minutes: number;
    min_break_minutes: number;
  } | null>(null);

  let correctionEmployeeId = $state('');
  let correctionKind = $state('clock_in');
  let correctionAtLocal = $state(toLocalDatetimeInputValue(new Date()));
  let correctionReason = $state('');
  let timeEvents = $state<
    { id: string; kind: string; occurred_at: string; notes?: string; source: string }[]
  >([]);

  function notify(type: 'error' | 'success', text: string) {
    onMessage?.({ type, text });
  }

  export async function refresh() {
    const tsParams = new URLSearchParams();
    if (!timesheetAllWeeks) {
      const { from: weekStart } = weekRangeContaining(shiftWeekAnchor);
      tsParams.set('period_start', weekStart);
    }
    if (timesheetFilter !== 'all') tsParams.set('status', timesheetFilter);
    if (timesheetEmployeeFilter) tsParams.set('employee_id', timesheetEmployeeFilter);
    const tsQs = tsParams.toString();
    timesheets = await api<TimesheetRow[]>(
      apiUrl,
      tsQs ? `/api/v1/time/timesheets?${tsQs}` : '/api/v1/time/timesheets',
    ).catch(() => []);
    if (canApprove) {
      const [drafts, rejected] = await Promise.all([
        api<TimesheetRow[]>(apiUrl, '/api/v1/time/timesheets?status=draft').catch(() => []),
        api<TimesheetRow[]>(apiUrl, '/api/v1/time/timesheets?status=rejected').catch(() => []),
      ]);
      const myEmpId = user?.employee_id;
      const seen = new Set<string>();
      teamDraftRows = [...drafts, ...rejected].filter((t) => {
        if (myEmpId && t.employee_id === myEmpId) return false;
        if (seen.has(t.id)) return false;
        seen.add(t.id);
        return true;
      });
    } else {
      teamDraftRows = [];
    }
    activePolicy = await api<typeof activePolicy>(apiUrl, '/api/v1/admin/policy').catch(() => null);
    await refreshTimeEvents();
  }

  async function refreshTimeEvents() {
    if (!correctionEmployeeId) return;
    timeEvents = await api<typeof timeEvents>(
      apiUrl,
      `/api/v1/time/events?employee_id=${encodeURIComponent(correctionEmployeeId)}&limit=30`,
    ).catch(() => []);
  }

  async function submitAllDraftTimesheets() {
    try {
      const params = new URLSearchParams();
      if (!timesheetAllWeeks) {
        const { from } = weekRangeContaining(shiftWeekAnchor);
        params.set('period_start', from);
      }
      const qs = params.toString();
      const path = qs
        ? `/api/v1/time/timesheets/submit-drafts?${qs}`
        : '/api/v1/time/timesheets/submit-drafts';
      const res = await api<{ submitted: number }>(apiUrl, path, { method: 'POST' });
      await refresh();
      await onDashboardChange?.();
      notify(
        'success',
        res.submitted > 0
          ? `${res.submitted} Stundenzettel eingereicht`
          : 'Keine Entwürfe zum Einreichen',
      );
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function submitAllTeamDraftTimesheets() {
    try {
      const res = await api<{ submitted: number }>(
        apiUrl,
        '/api/v1/time/timesheets/submit-drafts',
        { method: 'POST' },
      );
      await refresh();
      await onDashboardChange?.();
      notify(
        'success',
        res.submitted > 0
          ? `${res.submitted} Team-Entwürfe eingereicht`
          : 'Keine Team-Entwürfe zum Einreichen',
      );
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function rebuildTimesheets() {
    rebuildWarnings = [];
    try {
      const { from } = weekRangeContaining(shiftWeekAnchor);
      const res = await api<{ updated: number; warnings?: string[] }>(
        apiUrl,
        `/api/v1/time/timesheets/rebuild?week_start=${encodeURIComponent(from)}`,
        { method: 'POST' },
      );
      await refresh();
      await onDashboardChange?.();
      rebuildWarnings = res.warnings ?? [];
      let msg = `${res.updated} Stundenzettel aktualisiert`;
      if (rebuildWarnings.length) msg += ` (${rebuildWarnings.length} Hinweise)`;
      notify('success', msg);
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function timesheetAction(id: string, action: 'submit' | 'approve' | 'reject') {
    try {
      if (action === 'reject') {
        await api(apiUrl, `/api/v1/time/timesheets/${id}/reject`, {
          method: 'POST',
          body: JSON.stringify({ reason: rejectReason || 'Korrektur erforderlich' }),
        });
      } else {
        await api(apiUrl, `/api/v1/time/timesheets/${id}/${action}`, { method: 'POST' });
      }
      await refresh();
      await onDashboardChange?.();
      notify('success', 'Stundenzettel aktualisiert');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function approveAllPendingTimesheets() {
    try {
      const res = await api<{ approved: number }>(
        apiUrl,
        '/api/v1/time/timesheets/approve-pending',
        { method: 'POST' },
      );
      await refresh();
      await onDashboardChange?.();
      notify('success', `${res.approved} Stundenzettel freigegeben`);
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function exportTimesheets(fmt: 'csv' | 'html') {
    try {
      const { from } = weekRangeContaining(shiftWeekAnchor);
      const weekTag = timesheetAllWeeks
        ? 'alle'
        : calendarWeekLabel(from).replace(/\s+/g, '-').toLowerCase();
      let path =
        `/api/v1/reports/timesheets/export?format=${fmt}` +
        `&status=${encodeURIComponent(exportTimesheetStatus)}`;
      if (!timesheetAllWeeks) {
        path += `&period_start=${encodeURIComponent(from)}`;
      }
      if (fmt === 'csv') {
        await downloadFile(apiUrl, path, `stundenzettel_${exportTimesheetStatus}_${weekTag}.csv`);
      } else {
        await openHtmlExport(apiUrl, path);
      }
      notify('success', 'Export gestartet');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function submitCorrection() {
    if (!correctionReason.trim()) {
      notify('error', 'Begründung ist Pflicht');
      return;
    }
    try {
      await api(apiUrl, '/api/v1/time/corrections', {
        method: 'POST',
        body: JSON.stringify({
          employee_id: correctionEmployeeId,
          kind: correctionKind,
          occurred_at: fromLocalDatetimeInputValue(correctionAtLocal),
          reason: correctionReason,
        }),
      });
      correctionReason = '';
      await refreshTimeEvents();
      await onDashboardChange?.();
      notify('success', 'Korrektur gespeichert');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  $effect(() => {
    if (employees.length && !correctionEmployeeId) {
      correctionEmployeeId = employees[0].id;
    }
  });

  $effect(() => {
    if (active) {
      shiftWeekAnchor;
      void refresh();
    }
  });
</script>

{#if canCorrectTime}
  <div class="card" style="margin-top: 1rem;">
    <h3>Zeitkorrektur</h3>
    <div class="grid-form">
      <select bind:value={correctionEmployeeId} onchange={() => refreshTimeEvents()}>
        {#each employees as e}
          <option value={e.id}>{e.employee_no} — {e.display_name}</option>
        {/each}
      </select>
      <select bind:value={correctionKind}>
        <option value="clock_in">Kommen</option>
        <option value="clock_out">Gehen</option>
        <option value="break_start">Pause Start</option>
        <option value="break_end">Pause Ende</option>
      </select>
      <input type="datetime-local" bind:value={correctionAtLocal} />
      <input bind:value={correctionReason} placeholder="Begründung (Pflicht)" />
      <button type="button" onclick={submitCorrection}>Korrektur buchen</button>
    </div>
    <ul class="compact-list" style="margin-top: 0.75rem;">
      {#each timeEvents as ev}
        <li>
          {statusLabel(ev.kind)} — {formatIsoLocalShort(ev.occurred_at)}
          {#if ev.notes}<span class="muted"> ({ev.notes})</span>{/if}
        </li>
      {/each}
    </ul>
  </div>
{/if}

<div class="card" style="margin-top: 1rem;">
  <div class="btn-row">
    <button type="button" onclick={rebuildTimesheets}>
      Stundenzettel ({weekLabelForAnchor(shiftWeekAnchor)}) neu berechnen
    </button>
    <button class="secondary" type="button" onclick={submitAllDraftTimesheets}>
      Entwürfe einreichen
    </button>
    {#if canApprove && pendingTimesheets > 0}
      <button class="secondary" type="button" onclick={approveAllPendingTimesheets}>
        Alle {pendingTimesheets} Stundenzettel freigeben
      </button>
    {/if}
    {#if canApprove}
      <select bind:value={exportTimesheetStatus}>
        <option value="approved">Export: Freigegeben</option>
        <option value="pending">Export: Eingereicht</option>
        <option value="draft">Export: Entwurf</option>
        <option value="rejected">Export: Abgelehnt</option>
      </select>
      <button class="secondary" type="button" onclick={() => exportTimesheets('csv')}>
        CSV Export
      </button>
      <button class="secondary" type="button" onclick={() => exportTimesheets('html')}>
        HTML / PDF
      </button>
    {/if}
  </div>
  {#if activePolicy}
    <p class="muted" style="margin-top: 0.75rem;">
      ArbZG (aktiv): max. {Math.floor(activePolicy.max_daily_minutes / 60)} h/Tag,{' '}
      {Math.floor(activePolicy.max_weekly_minutes / 60)} h/Woche — Pause min.{' '}
      {activePolicy.min_break_minutes} min
    </p>
  {/if}
  <div class="btn-row" style="margin-top: 0.75rem;">
    <select bind:value={timesheetFilter} onchange={() => refresh()}>
      <option value="all">Alle Stundenzettel</option>
      <option value="pending">Eingereicht</option>
      <option value="draft">Entwurf</option>
      <option value="approved">Freigegeben</option>
      <option value="rejected">Abgelehnt</option>
    </select>
    <label class="muted">
      <input type="checkbox" bind:checked={timesheetAllWeeks} onchange={() => refresh()} />
      Alle Kalenderwochen
    </label>
    {#if canApprove}
      <select bind:value={timesheetEmployeeFilter} onchange={() => refresh()}>
        <option value="">Alle Mitarbeiter</option>
        {#each employees.filter((e) => e.active !== false) as e}
          <option value={e.id}>{e.employee_no} — {e.display_name}</option>
        {/each}
      </select>
    {/if}
  </div>
  {#if canApprove}
    <TimeSettlementCard {apiUrl} {employees} {active} onMessage={onMessage} />
  {/if}
  <h3 style="margin-top: 1rem;">
    Stundenzettel{#if !timesheetAllWeeks}
      ({weekLabelForAnchor(shiftWeekAnchor)}){/if}
  </h3>
  {#each timesheets as t}
    <div class="row-card">
      <p>
        <strong>{t.employee_no} {t.employee_name}</strong>
        <span class="muted">
          ({calendarWeekLabel(t.period_start)}
          {formatPeriodRange(t.period_start, t.period_end)})
        </span>
        — Ist {formatMinutes(t.worked_minutes)}
        {#if t.expected_minutes != null && t.expected_minutes > 0}
          · Soll {formatMinutes(t.expected_minutes)}
          · Saldo {formatMinutes(t.balance_minutes ?? t.worked_minutes - t.expected_minutes)}
        {/if}
        (ÜS {formatMinutes(t.overtime_minutes)}) — <em>{statusLabel(t.status)}</em>
      </p>
      {#if t.evaluation?.work_calendar_name}
        <p class="muted">
          Kalender: {t.evaluation.work_calendar_name}
          {#if t.evaluation.part_time_percent < 100}
            · {t.evaluation.part_time_percent}% Teilzeit
          {/if}
        </p>
      {/if}
      {#if t.rejection_reason}<p class="muted">Grund: {t.rejection_reason}</p>{/if}
      {#if t.evaluation?.days?.length}
        <button
          class="secondary"
          type="button"
          onclick={() => (expandedTimesheetId = expandedTimesheetId === t.id ? '' : t.id)}
        >
          {expandedTimesheetId === t.id ? 'Tagesdetails ausblenden' : 'Tagesdetails'}
        </button>
        {#if expandedTimesheetId === t.id}
          <table class="data-table" style="margin-top: 0.5rem; font-size: 0.9rem;">
            <thead>
              <tr>
                <th>Tag</th>
                <th>Modell</th>
                <th>Soll</th>
                <th>Ist</th>
                <th>Saldo</th>
              </tr>
            </thead>
            <tbody>
              {#each t.evaluation.days as d}
                <tr>
                  <td>{d.date}</td>
                  <td>
                    {d.model_name}
                    {#if d.absence_label}
                      <span class="muted"> · {d.absence_label}</span>
                    {/if}
                  </td>
                  <td>{formatMinutes(d.expected_minutes)}</td>
                  <td>{formatMinutes(d.worked_minutes)}</td>
                  <td>{formatMinutes(d.balance_minutes)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      {/if}
      <div class="btn-row">
        {#if t.status === 'draft' || t.status === 'rejected'}
          <button class="secondary" type="button" onclick={() => timesheetAction(t.id, 'submit')}>
            Einreichen
          </button>
        {/if}
        {#if canApprove && t.status === 'pending'}
          <button type="button" onclick={() => timesheetAction(t.id, 'approve')}>Freigeben</button>
          <button class="secondary" type="button" onclick={() => timesheetAction(t.id, 'reject')}>
            Ablehnen
          </button>
        {/if}
      </div>
    </div>
  {:else}
    <p class="muted">Nach Stempeln: „neu berechnen“, dann einreichen/freigeben.</p>
  {/each}
  {#if rebuildWarnings.length}
    <div style="margin-top: 0.75rem;">
      <p class="muted">ArbZG-Hinweise:</p>
      <ul class="compact-list">
        {#each rebuildWarnings as w}<li>{w}</li>{/each}
      </ul>
    </div>
  {/if}
  {#if canApprove && teamDraftRows.length > 0}
    <div class="card" style="margin-top: 1rem;">
      <h3>Team-Entwürfe ({teamDraftRows.length})</h3>
      <p class="muted" style="margin-bottom: 0.5rem;">
        Entwürfe anderer Mitarbeiter vor Freigabe einreichen.
      </p>
      <button
        class="secondary"
        type="button"
        style="margin-bottom: 0.5rem;"
        onclick={submitAllTeamDraftTimesheets}
      >
        Alle {teamDraftRows.length} Team-Entwürfe einreichen
      </button>
      {#each teamDraftRows as t}
        <div class="row-card">
          <p>
            <strong>{t.employee_no} {t.employee_name}</strong>
            <span class="muted">
              ({calendarWeekLabel(t.period_start)}
              {formatPeriodRange(t.period_start, t.period_end)})
            </span>
            — {formatMinutes(t.worked_minutes)} · {statusLabel(t.status)}
          </p>
          {#if t.rejection_reason}<p class="muted">{t.rejection_reason}</p>{/if}
          <button class="secondary" type="button" onclick={() => timesheetAction(t.id, 'submit')}>
            Einreichen
          </button>
        </div>
      {/each}
    </div>
  {/if}
  {#if canApprove}
    <input
      bind:value={rejectReason}
      placeholder="Ablehnungsgrund (optional)"
      style="margin-top: 0.75rem;"
    />
  {/if}
</div>
