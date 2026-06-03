<script lang="ts">
  import type { LoginResponse } from '../lib/api';
  import { formatIsoLocalShort } from '../lib/datetime';
  import { statusLabel } from '../lib/statusLabels';
  import { doorStatusLabel } from '../lib/accessLabels';
  import ProductionChecklistCard from './ProductionChecklistCard.svelte';
  import ProductionWizard from './ProductionWizard.svelte';
  import SetupGuideCard from './SetupGuideCard.svelte';
  import { checklistAllOk, computeProductionChecklist } from '../lib/productionChecklist';

  export type OverviewNavigate = {
    tab: 'time' | 'absence' | 'access' | 'personnel';
    timesheetFilter?: 'pending' | 'draft';
    absenceFilter?: 'pending';
    shiftFilter?: 'planned';
    personnelNoCalendar?: boolean;
    timeSection?: 'stammdaten' | 'planung' | 'stundenzettel' | 'abschluss';
  };

  type Dashboard = {
    clocked_in_employees: number;
    employees_total: number;
    pending_timesheets: number;
    draft_timesheets: number;
    pending_absences: number;
    shifts_this_week: number;
    planned_shifts_this_week: number;
    people_in_building: number;
    doors_alarm: number;
    doors_forced_open: number;
    doors_open: number;
    employees_without_work_calendar?: number;
    timesheets_current_week_no_soll?: number;
    time_access_mismatch_count?: number;
    time_access_mismatches?: {
      employee_no: string;
      display_name: string;
      kind: string;
    }[];
    door_alerts: { id: string; name: string; status: string }[];
    hardware_adapter?: string;
    default_password_login_blocked?: boolean;
    demo_seeding_enabled?: boolean;
  };

  let {
    apiBind,
    apiUrls,
    dbPath,
    apiHealth,
    dashboard,
    zoneOccupancy,
    clockedIn,
    user,
    username = $bindable('admin'),
    password = $bindable('admin'),
    onRefreshAll,
    onRefreshHealth,
    onCopyApiUrl,
    onLogin,
    onNavigate,
    onFoundationFix,
    onSetDoorStatus,
    timeAccessMismatchLabel,
  }: {
    apiBind: string;
    apiUrls: string[];
    dbPath: string;
    apiHealth: {
      status: string;
      version: string;
      database: string;
      service: string;
      demo_seeding_enabled?: boolean;
      default_password_login_blocked?: boolean;
      time_foundation?: {
        workday_models: number;
        work_calendars: number;
        active_employees: number;
        employees_without_work_calendar: number;
        current_week_drafts_without_soll: number;
      };
    } | null;
    dashboard: Dashboard | null;
    zoneOccupancy: {
      zone_name: string;
      inside_count: number;
      occupants: { display_name: string }[];
    }[];
    clockedIn: {
      employee_no: string;
      display_name: string;
      last_kind: string;
      last_at: string;
      is_on_break: boolean;
    }[];
    user: LoginResponse['user'] | null;
    username?: string;
    password?: string;
    onRefreshAll?: () => void;
    onRefreshHealth?: () => void;
    onCopyApiUrl?: (url: string) => void;
    onLogin?: () => void;
    onNavigate?: (target: OverviewNavigate) => void;
    onFoundationFix?: () => void;
    onSetDoorStatus?: (doorId: string, status: string) => void;
    timeAccessMismatchLabel: (kind: string) => string;
  } = $props();

  let wizardOpen = $state(false);
  let wizardAutoPrompted = $state(false);

  $effect(() => {
    if (!user || !apiHealth || wizardAutoPrompted || wizardOpen) return;
    if (typeof localStorage !== 'undefined' && localStorage.getItem('timeshards.production_wizard_dismissed') === '1') {
      wizardAutoPrompted = true;
      return;
    }
    const items = computeProductionChecklist(apiHealth, dashboard);
    if (checklistAllOk(items)) return;
    wizardAutoPrompted = true;
    wizardOpen = true;
  });
</script>

<h2>Server-Übersicht</h2>
{#if user}
  <button class="secondary" style="margin-top: 0.5rem;" type="button" onclick={onRefreshAll}>
    Aktualisieren
  </button>
{/if}
<div class="card" style="margin-top: 1rem;">
  <p><strong>API bindet auf:</strong> {apiBind || '…'}</p>
  {#if apiHealth}
    <p class="muted">
      API {apiHealth.service} v{apiHealth.version} - {apiHealth.status}
      (DB: {apiHealth.database})
    </p>
    {#if apiHealth.status !== 'ok'}
      <p class="error" style="margin-top: 0.35rem;">API degradiert — Details prüfen</p>
    {/if}
    {#if apiHealth.demo_seeding_enabled || apiHealth.default_password_login_blocked}
      <p class="error" style="margin-top: 0.35rem; font-size: 0.85rem;">
        {#if apiHealth.default_password_login_blocked && !apiHealth.demo_seeding_enabled}
          Produktionsmodus — Standardpasswörter gesperrt.
        {:else if apiHealth.default_password_login_blocked}
          Staging: Standardpasswörter gesperrt (<code>TIMESHARDS_BLOCK_DEFAULT_PASSWORDS</code>).
        {:else}
          Demo-Seeding aktiv — für Produktion <code>TIMESHARDS_DISABLE_DEMO=1</code>
        {/if}
      </p>
    {/if}
    {#if apiHealth.time_foundation}
      {@const tf = apiHealth.time_foundation}
      <p class="muted" style="margin-top: 0.35rem; font-size: 0.85rem;">
        Zeitbasis: {tf.workday_models} Tagesmodelle, {tf.work_calendars} Kalender, {tf.active_employees}{' '}
        aktive MA
        {#if tf.employees_without_work_calendar > 0 || tf.current_week_drafts_without_soll > 0}
          —
          {#if tf.employees_without_work_calendar > 0}
            <button
              type="button"
              class="linkish warn"
              onclick={() => onNavigate?.({ tab: 'personnel', personnelNoCalendar: true })}
            >
              {tf.employees_without_work_calendar} ohne Kalender
            </button>
          {/if}
          {#if tf.current_week_drafts_without_soll > 0}
            {#if tf.employees_without_work_calendar > 0}; {/if}
            <button
              type="button"
              class="linkish warn"
              onclick={() => onNavigate?.({ tab: 'time', timeSection: 'stammdaten' })}
            >
              {tf.current_week_drafts_without_soll} KW ohne Soll
            </button>
          {/if}
        {:else}
          — OK
        {/if}
      </p>
    {/if}
  {/if}
  <button class="secondary" style="margin-top: 0.35rem;" type="button" onclick={onRefreshHealth}>
    API-Status prüfen
  </button>
  <p><strong>Client-URLs (im LAN):</strong></p>
  <ul>
    {#each apiUrls as u}
      <li>
        <code>{u}</code>
        <button class="secondary" type="button" onclick={() => onCopyApiUrl?.(u)}>Kopieren</button>
      </li>
    {/each}
  </ul>
  <p class="muted">
    Firewall: als Administrator <code>scripts/open-firewall.ps1</code> ausführen.
  </p>
  <p><strong>Datenbank:</strong> {dbPath || '…'}</p>
  {#if dashboard?.hardware_adapter}
    <p class="muted" style="font-size: 0.85rem; margin-top: 0.35rem;">
      System: HW <code>{dashboard.hardware_adapter}</code>
      {#if dashboard.default_password_login_blocked}
        · Standardpasswörter gesperrt
      {/if}
      {#if dashboard.demo_seeding_enabled}
        · Demo-Seeding
      {/if}
    </p>
  {/if}
</div>
<div class="card" style="margin-top: 1rem;">
  <h3>Anmeldung</h3>
  {#if user}
    <p class="success">Angemeldet als {user.display_name} ({user.roles.join(', ')})</p>
    {#if dashboard}
      <div class="stat-grid">
        <button
          type="button"
          class="stat-card stat-card-btn"
          onclick={() => onNavigate?.({ tab: 'time' })}
        >
          <span class="muted">Eingestempelt</span>
          <strong>{dashboard.clocked_in_employees}</strong>
          <span class="muted">/ {dashboard.employees_total} MA</span>
        </button>
        <button
          type="button"
          class="stat-card stat-card-btn"
          onclick={() => onNavigate?.({ tab: 'time', timesheetFilter: 'pending' })}
        >
          <span class="muted">Stundenzettel offen</span>
          <strong>{dashboard.pending_timesheets}</strong>
        </button>
        {#if dashboard.draft_timesheets > 0}
          <button
            type="button"
            class="stat-card stat-card-btn"
            onclick={() => onNavigate?.({ tab: 'time', timesheetFilter: 'draft' })}
          >
            <span class="muted">Entwürfe / abgelehnt</span>
            <strong>{dashboard.draft_timesheets}</strong>
          </button>
        {/if}
        <button
          type="button"
          class="stat-card stat-card-btn"
          onclick={() => onNavigate?.({ tab: 'absence', absenceFilter: 'pending' })}
        >
          <span class="muted">Abwesenheit offen</span>
          <strong>{dashboard.pending_absences}</strong>
        </button>
        <button
          type="button"
          class="stat-card stat-card-btn"
          onclick={() => onNavigate?.({ tab: 'time', shiftFilter: 'planned' })}
        >
          <span class="muted">Schichten diese Woche</span>
          <strong>{dashboard.shifts_this_week}</strong>
          {#if dashboard.planned_shifts_this_week > 0}
            <span class="muted"> ({dashboard.planned_shifts_this_week} geplant)</span>
          {/if}
        </button>
        <button
          type="button"
          class="stat-card stat-card-btn"
          onclick={() => onNavigate?.({ tab: 'access' })}
        >
          <span class="muted">Im Gebäude (Zonen)</span>
          <strong>{dashboard.people_in_building}</strong>
        </button>
        {#if dashboard.doors_alarm + dashboard.doors_forced_open + dashboard.doors_open > 0}
          <button
            type="button"
            class="stat-card stat-card-btn"
            style="border-color: #b00020;"
            onclick={() => onNavigate?.({ tab: 'access' })}
          >
            <span class="muted">Tür-Alerts</span>
            <strong
              >{dashboard.doors_alarm + dashboard.doors_forced_open + dashboard.doors_open}</strong
            >
          </button>
        {/if}
        {#if (dashboard.employees_without_work_calendar ?? 0) > 0}
          <button
            type="button"
            class="stat-card stat-card-btn"
            style="border-color: #b8860b;"
            onclick={() => onNavigate?.({ tab: 'personnel', personnelNoCalendar: true })}
          >
            <span class="muted">MA ohne Arbeitskalender</span>
            <strong>{dashboard.employees_without_work_calendar}</strong>
          </button>
        {/if}
        {#if (dashboard.timesheets_current_week_no_soll ?? 0) > 0}
          <button
            type="button"
            class="stat-card stat-card-btn"
            style="border-color: #b8860b;"
            onclick={() => onNavigate?.({ tab: 'time', timeSection: 'stammdaten' })}
          >
            <span class="muted">KW ohne Soll (Entwurf)</span>
            <strong>{dashboard.timesheets_current_week_no_soll}</strong>
          </button>
        {/if}
        {#if (dashboard.time_access_mismatch_count ?? 0) > 0}
          <button
            type="button"
            class="stat-card stat-card-btn"
            style="border-color: #b8860b;"
            onclick={() => onNavigate?.({ tab: 'access' })}
          >
            <span class="muted">Zeit ↔ Zutritt</span>
            <strong>{dashboard.time_access_mismatch_count}</strong>
          </button>
        {/if}
      </div>
      {#if (dashboard.employees_without_work_calendar ?? 0) > 0 || (dashboard.timesheets_current_week_no_soll ?? 0) > 0}
        <div class="btn-row" style="margin-top: 0.75rem;">
          <button type="button" onclick={onFoundationFix}>Zeitbasis reparieren</button>
        </div>
        <p class="muted" style="margin-top: 0.5rem; font-size: 0.85rem;">
          {#if (dashboard.employees_without_work_calendar ?? 0) > 0}
            {dashboard.employees_without_work_calendar} aktive Mitarbeiter ohne Arbeitskalender —
            Zuweisung + Neuberechnung der aktuellen KW.
          {/if}
          {#if (dashboard.timesheets_current_week_no_soll ?? 0) > 0}
            {#if (dashboard.employees_without_work_calendar ?? 0) > 0}<br />{/if}
            {dashboard.timesheets_current_week_no_soll} Entwürfe diese KW ohne Soll.
          {/if}
        </p>
      {/if}
      {#if zoneOccupancy.some((z) => z.inside_count > 0)}
        <div style="margin-top: 0.75rem;">
          <p class="muted">Im Gebäude (Zonen):</p>
          <ul class="compact-list">
            {#each zoneOccupancy.filter((z) => z.inside_count > 0) as z}
              <li>
                <strong>{z.zone_name}</strong>: {z.inside_count}
                {#if z.occupants.length}
                  — {z.occupants.map((o) => o.display_name).join(', ')}
                {/if}
              </li>
            {/each}
          </ul>
        </div>
      {/if}
      {#if clockedIn.length}
        <div style="margin-top: 0.75rem;">
          <p class="muted">Eingestempelt jetzt:</p>
          <ul class="compact-list">
            {#each clockedIn as c}
              <li>
                {c.employee_no} {c.display_name} — {statusLabel(c.last_kind)}
                {#if c.is_on_break}<span class="muted"> (Pause)</span>{/if}
                <span class="muted"> seit {formatIsoLocalShort(c.last_at)}</span>
              </li>
            {/each}
          </ul>
        </div>
      {/if}
      {#if (dashboard.time_access_mismatches?.length ?? 0) > 0}
        <div style="margin-top: 0.75rem;">
          <p class="muted">Abweichung Stempel vs. Gebäude:</p>
          <ul class="compact-list">
            {#each dashboard.time_access_mismatches ?? [] as m}
              <li>
                {m.employee_no} {m.display_name} —
                <strong>{timeAccessMismatchLabel(m.kind)}</strong>
              </li>
            {/each}
          </ul>
        </div>
      {/if}
      {#if dashboard.door_alerts.length}
        <ul class="compact-list" style="margin-top: 0.75rem;">
          {#each dashboard.door_alerts as d}
            <li>
              {d.name} — <strong>{doorStatusLabel(d.status)}</strong>
              <button
                class="secondary"
                type="button"
                onclick={() => {
                  onNavigate?.({ tab: 'access' });
                  onSetDoorStatus?.(d.id, 'closed');
                }}
              >
                Zurücksetzen
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    {/if}
    <SetupGuideCard
      {apiHealth}
      {onNavigate}
      onOpenWizard={() => (wizardOpen = true)}
    />
    <ProductionChecklistCard {apiHealth} {dashboard} onOpenWizard={() => (wizardOpen = true)} />
    <ProductionWizard
      bind:open={wizardOpen}
      {apiHealth}
      {dashboard}
      onNavigate={onNavigate}
      onFoundationFix={onFoundationFix}
      onRefresh={onRefreshAll}
    />
  {:else}
    <div style="display: grid; gap: 0.5rem; max-width: 320px;">
      <input
        bind:value={username}
        placeholder="Benutzername"
        onkeydown={(e) => e.key === 'Enter' && onLogin?.()}
      />
      <input
        type="password"
        bind:value={password}
        placeholder="Passwort"
        onkeydown={(e) => e.key === 'Enter' && onLogin?.()}
      />
      <div class="btn-row">
        <button
          class="secondary"
          type="button"
          onclick={() => {
            username = 'admin';
            password = 'admin';
          }}>admin</button>
        <button
          class="secondary"
          type="button"
          onclick={() => {
            username = 'demo';
            password = 'demo';
          }}>demo</button>
        <button
          class="secondary"
          type="button"
          onclick={() => {
            username = 'manager';
            password = 'demo';
          }}>manager</button>
      </div>
      <button type="button" onclick={onLogin}>Anmelden</button>
    </div>
  {/if}
</div>
