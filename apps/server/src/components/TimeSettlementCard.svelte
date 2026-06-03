<script lang="ts">
  import { api, downloadFile } from '../lib/api';
  import { formatMinutes } from '../lib/formatMinutes';

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
    active = false,
    onMessage,
  }: {
    apiUrl: string;
    employees: Employee[];
    active?: boolean;
    onMessage?: (msg: UiMessage) => void;
  } = $props();

  let timeAccountsEmployeeFilter = $state('');
  let timeAccounts = $state<
    { account_kind: string; label: string; balance_minutes: number }[]
  >([]);
  let settlementRules = $state<
    {
      id: string;
      name: string;
      config: { enforce_flex_band?: boolean; warn_negative_balance?: boolean };
    }[]
  >([]);
  let selectedSettlementRuleId = $state('sr-weekly-default');
  let settlementEmployeeId = $state('');
  let settlementYear = $state(new Date().getFullYear());
  let settlementMonth = $state(new Date().getMonth() + 1);
  let payrollAggregate = $state(true);
  let payrollEmployeeFilter = $state('');

  let monthSettlementPreview = $state<{
    worked_minutes: number;
    expected_minutes: number;
    balance_minutes: number;
    approved_weeks: number;
    pending_weeks: number;
    draft_weeks: number;
    already_closed: boolean;
  } | null>(null);

  const activeEmployees = $derived(employees.filter((e) => e.active !== false));

  function notify(type: 'error' | 'success', text: string) {
    onMessage?.({ type, text });
  }

  async function refreshTimeAccounts() {
    const params = new URLSearchParams();
    if (timeAccountsEmployeeFilter) {
      params.set('employee_id', timeAccountsEmployeeFilter);
    }
    const qs = params.toString();
    timeAccounts = await api<typeof timeAccounts>(
      apiUrl,
      qs ? `/api/v1/time/accounts?${qs}` : '/api/v1/time/accounts',
    ).catch(() => []);
  }

  async function refreshSettlementRules() {
    settlementRules = await api<typeof settlementRules>(
      apiUrl,
      '/api/v1/time/settlement-rules',
    ).catch(() => []);
    if (
      settlementRules.length &&
      !settlementRules.some((r) => r.id === selectedSettlementRuleId)
    ) {
      selectedSettlementRuleId = settlementRules[0].id;
    }
  }

  async function refreshMonthSettlementPreview() {
    if (!settlementEmployeeId) {
      monthSettlementPreview = null;
      return;
    }
    const params = new URLSearchParams({
      year: String(settlementYear),
      month: String(settlementMonth),
      employee_id: settlementEmployeeId,
    });
    monthSettlementPreview = await api<typeof monthSettlementPreview>(
      apiUrl,
      `/api/v1/time/settlement-periods/preview?${params}`,
    ).catch(() => null);
  }

  export async function refresh() {
    await refreshSettlementRules();
    if (!settlementEmployeeId && activeEmployees.length) {
      settlementEmployeeId = activeEmployees[0].id;
    }
    await refreshTimeAccounts();
    await refreshMonthSettlementPreview();
  }

  $effect(() => {
    if (active) void refresh();
  });

  async function saveSettlementRuleConfig() {
    const rule = settlementRules.find((r) => r.id === selectedSettlementRuleId);
    if (!rule) return;
    try {
      await api(apiUrl, `/api/v1/time/settlement-rules/${rule.id}`, {
        method: 'PUT',
        body: JSON.stringify({ config: rule.config }),
      });
      notify('success', 'Abrechnungsregel gespeichert');
      await refreshSettlementRules();
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function exportPayrollCsv() {
    try {
      const params = new URLSearchParams({
        year: String(settlementYear),
        month: String(settlementMonth),
        format: 'csv',
      });
      if (payrollAggregate) params.set('aggregate', 'employee');
      if (payrollEmployeeFilter) params.set('employee_id', payrollEmployeeFilter);
      const path = `/api/v1/reports/payroll/export?${params}`;
      const name = `lohn_export_${settlementYear}_${String(settlementMonth).padStart(2, '0')}.csv`;
      await downloadFile(apiUrl, path, name);
      notify('success', `Lohn-CSV ${settlementMonth}/${settlementYear} heruntergeladen`);
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function exportAbsencesCsv() {
    try {
      const params = new URLSearchParams({
        year: String(settlementYear),
        month: String(settlementMonth),
        format: 'csv',
      });
      if (payrollEmployeeFilter) params.set('employee_id', payrollEmployeeFilter);
      const path = `/api/v1/reports/absences/export?${params}`;
      const name = `abwesenheit_export_${settlementYear}_${String(settlementMonth).padStart(2, '0')}.csv`;
      await downloadFile(apiUrl, path, name);
      notify('success', `Abwesenheiten-CSV ${settlementMonth}/${settlementYear} heruntergeladen`);
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function closeMonthSettlement() {
    if (!settlementEmployeeId) {
      notify('error', 'Mitarbeiter für Monatsabschluss wählen');
      return;
    }
    try {
      await api(apiUrl, '/api/v1/time/settlement-periods', {
        method: 'POST',
        body: JSON.stringify({
          employee_id: settlementEmployeeId,
          year: settlementYear,
          month: settlementMonth,
        }),
      });
      await refreshMonthSettlementPreview();
      await refreshTimeAccounts();
      notify('success', `Monat ${settlementMonth}/${settlementYear} abgeschlossen`);
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }
</script>

<div
  style="margin-top: 0.75rem; padding: 0.75rem; border: 1px solid var(--border, #e2e8f0); border-radius: 6px;"
>
  <h4 style="margin: 0 0 0.5rem;">Zeitkonten</h4>
  <select
    bind:value={timeAccountsEmployeeFilter}
    onchange={() => refreshTimeAccounts()}
    style="margin-bottom: 0.5rem;"
  >
    <option value="">Eigener Kontostand (eingeloggter MA)</option>
    {#each activeEmployees as e}
      <option value={e.id}>{e.employee_no} — {e.display_name}</option>
    {/each}
  </select>
  <ul class="compact-list">
    {#each timeAccounts as a}
      <li>{a.label}: <strong>{formatMinutes(a.balance_minutes)}</strong></li>
    {:else}
      <li class="muted">Noch keine Buchungen — nach Freigabe eines Stundenzettels sichtbar.</li>
    {/each}
  </ul>

  <h4 style="margin: 1rem 0 0.5rem;">Abrechnungsregel</h4>
  <div class="grid-form" style="margin-bottom: 0.5rem;">
    <select bind:value={selectedSettlementRuleId}>
      {#each settlementRules as r}
        <option value={r.id}>{r.name}</option>
      {/each}
    </select>
    {#each settlementRules.filter((r) => r.id === selectedSettlementRuleId) as r}
      <label class="muted">
        <input type="checkbox" bind:checked={r.config.enforce_flex_band} />
        Gleitzeit beim Stempeln erzwingen (außerhalb Kernzeit ablehnen)
      </label>
      <button class="secondary" type="button" onclick={saveSettlementRuleConfig}>
        Regel speichern
      </button>
    {/each}
  </div>

  <h4 style="margin: 1rem 0 0.5rem;">Lohn-Export (CSV)</h4>
  <p class="muted" style="margin-bottom: 0.5rem;">
    Freigegebene Stundenzettel im Kalendermonat (Berlin). Für Lohnbüro / Excel — kein DATEV-Format.
  </p>
  <div class="grid-form">
    <label class="muted" for="payroll-export-year">Jahr</label>
    <input
      id="payroll-export-year"
      type="number"
      bind:value={settlementYear}
      min="2020"
      max="2035"
    />
    <label class="muted" for="payroll-export-month">Monat</label>
    <input id="payroll-export-month" type="number" bind:value={settlementMonth} min="1" max="12" />
    <select bind:value={payrollEmployeeFilter}>
      <option value="">Alle Mitarbeiter</option>
      {#each activeEmployees as e}
        <option value={e.id}>{e.employee_no} — {e.display_name}</option>
      {/each}
    </select>
    <label class="muted">
      <input type="checkbox" bind:checked={payrollAggregate} />
      Eine Zeile pro Mitarbeiter (Monatssumme)
    </label>
    <button class="secondary" type="button" onclick={exportPayrollCsv}>Lohn-CSV herunterladen</button>
    <button class="secondary" type="button" onclick={exportAbsencesCsv}>Abwesenheiten-CSV</button>
  </div>

  <h4 style="margin: 1rem 0 0.5rem;">Monatsabschluss</h4>
  <p class="muted" style="margin-bottom: 0.5rem;">
    Summiert freigegebene Wochenstundenzettel (KW-Montag im Monat). Abschluss nur ohne offene
    Entwürfe/Einreichungen.
  </p>
  <div class="grid-form">
    <select bind:value={settlementEmployeeId} onchange={() => refreshMonthSettlementPreview()}>
      {#each activeEmployees as e}
        <option value={e.id}>{e.employee_no} — {e.display_name}</option>
      {/each}
    </select>
    <input
      type="number"
      bind:value={settlementYear}
      min="2020"
      max="2035"
      onchange={() => refreshMonthSettlementPreview()}
    />
    <input
      type="number"
      bind:value={settlementMonth}
      min="1"
      max="12"
      onchange={() => refreshMonthSettlementPreview()}
    />
    <button class="secondary" type="button" onclick={() => refreshMonthSettlementPreview()}>
      Vorschau
    </button>
  </div>
  {#if monthSettlementPreview}
    <p style="margin-top: 0.5rem;">
      Ist {formatMinutes(monthSettlementPreview.worked_minutes)} · Soll{' '}
      {formatMinutes(monthSettlementPreview.expected_minutes)} · Saldo{' '}
      <strong>{formatMinutes(monthSettlementPreview.balance_minutes)}</strong>
    </p>
    <p class="muted">
      {monthSettlementPreview.approved_weeks} Woche(n) freigegeben ·{' '}
      {monthSettlementPreview.pending_weeks} eingereicht ·{' '}
      {monthSettlementPreview.draft_weeks} Entwurf
      {#if monthSettlementPreview.already_closed}
        · <em>Monat bereits abgeschlossen</em>
      {/if}
    </p>
    {#if !monthSettlementPreview.already_closed}
      <button
        class="secondary"
        type="button"
        onclick={closeMonthSettlement}
        disabled={monthSettlementPreview.pending_weeks > 0 ||
          monthSettlementPreview.draft_weeks > 0 ||
          monthSettlementPreview.approved_weeks < 1}
      >
        Monat abschließen
      </button>
    {/if}
  {/if}
</div>
