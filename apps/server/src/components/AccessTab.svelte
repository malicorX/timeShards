<script lang="ts">
  import { api, downloadFile, openHtmlExport } from '../lib/api';
  import {
    toLocalDatetimeInputValue,
    fromLocalDatetimeInputValue,
    isoToLocalDatetimeInput,
    formatIsoLocalShort,
  } from '../lib/datetime';
  import { doorStatusLabel, reasonLabel, accessDecisionLabel } from '../lib/accessLabels';
  import TsPageHeader from '@timeshards/shared/ui/TsPageHeader.svelte';
  import TsEmptyState from '@timeshards/shared/ui/TsEmptyState.svelte';

  type Employee = {
    id: string;
    employee_no: string;
    display_name: string;
    active?: boolean;
  };

  type ZoneOccupancy = {
    zone_name: string;
    inside_count: number;
    occupants: { display_name: string }[];
  };

  type UiMessage = { type: 'error' | 'success'; text: string };

  let {
    apiUrl,
    employees,
    apiHealth = null,
    active = false,
    onMessage,
    onDashboardChange,
    onOccupancyChange,
  }: {
    apiUrl: string;
    employees: Employee[];
    apiHealth?: {
      hardware_tcp_listen?: string | null;
    } | null;
    active?: boolean;
    onMessage?: (msg: UiMessage) => void;
    onDashboardChange?: () => void | Promise<void>;
    onOccupancyChange?: (occupancy: ZoneOccupancy[]) => void;
  } = $props();

  let selectedDoorId = $state('');
  let zones = $state<{ id: string; name: string; site_id: string }[]>([]);
  let doors = $state<
    {
      id: string;
      name: string;
      status: string;
      zone_id: string | null;
      reader_in_id?: string | null;
      reader_out_id?: string | null;
    }[]
  >([]);
  let badges = $state<
    {
      id: string;
      credential_uid: string;
      employee_id: string | null;
      employee_no?: string | null;
      employee_name?: string | null;
      status: string;
    }[]
  >([]);
  let accessExportFrom = $state('');
  let accessExportTo = $state('');
  let accessEvents = $state<
    {
      id: string;
      occurred_at: string;
      decision: string;
      reason_code?: string;
      employee_no?: string;
      employee_name?: string;
      zone_name?: string;
      door_name?: string;
    }[]
  >([]);
  let accessEventFilter = $state<'all' | 'grant' | 'deny'>('all');
  let accessEventEmployeeNo = $state('');
  let simulateUid = $state('DEMO-ADMIN-001');
  let simulateEmployeeId = $state('');
  let simulateReader = $state('sim.reader.main');
  let newZone = $state({ name: '' });
  let newDoor = $state({ name: '', zone_id: '', reader_id: 'sim.reader.main' });
  let newBadge = $state({ employee_id: '', credential_uid: '' });
  let accessRules = $state<
    {
      id: string;
      principal_id: string;
      zone_id?: string | null;
      employee_name?: string;
      zone_name?: string;
      mode: string;
      valid_from: string;
      valid_to?: string | null;
      schedule_json?: string | null;
    }[]
  >([]);
  let newAccessRule = $state({
    employee_id: '',
    zone_id: '',
    use_schedule: true,
    start: '08:00',
    end: '18:00',
    valid_from_local: '',
    valid_to_local: '',
  });
  let accessRuleEmployeeFilter = $state('');
  let accessRuleZoneFilter = $state('');
  let zoneOccupancy = $state<ZoneOccupancy[]>([]);

  const doorReaders = $derived.by(() => {
    const out: { id: string; label: string }[] = [];
    for (const d of doors) {
      if (d.reader_in_id) out.push({ id: d.reader_in_id, label: `${d.name} — Eingang` });
      if (d.reader_out_id) out.push({ id: d.reader_out_id, label: `${d.name} — Ausgang` });
    }
    return out;
  });

  const selectedDoor = $derived(doors.find((d) => d.id === selectedDoorId) ?? null);

  const displayedAccessRules = $derived(
    accessRules.filter((r) => {
      if (accessRuleEmployeeFilter && r.principal_id !== accessRuleEmployeeFilter) return false;
      if (accessRuleZoneFilter && r.zone_id !== accessRuleZoneFilter) return false;
      return true;
    }),
  );

  function notify(type: 'error' | 'success', text: string) {
    onMessage?.({ type, text });
  }

  function defaultAccessExportRange() {
    const to = new Date();
    const from = new Date(to.getTime() - 7 * 24 * 60 * 60 * 1000);
    accessExportFrom = toLocalDatetimeInputValue(from);
    accessExportTo = toLocalDatetimeInputValue(to);
  }

  export async function refresh() {
    if (!accessExportFrom) defaultAccessExportRange();
    zones = await api(apiUrl, '/api/v1/access/zones');
    doors = await api(apiUrl, '/api/v1/access/doors');
    if (doors.length && !doors.some((d) => d.id === selectedDoorId)) {
      selectedDoorId = doors[0].id;
    }
    const readerIds: string[] = [];
    for (const d of doors) {
      if (d.reader_in_id) readerIds.push(d.reader_in_id);
      if (d.reader_out_id) readerIds.push(d.reader_out_id);
    }
    if (readerIds.length > 0 && !readerIds.includes(simulateReader)) {
      simulateReader = readerIds[0];
    }
    badges = await api(apiUrl, '/api/v1/access/badges');
    let eventsPath = '/api/v1/access/events?limit=50';
    if (accessEventFilter !== 'all') eventsPath += `&decision=${accessEventFilter}`;
    if (accessEventEmployeeNo.trim()) {
      eventsPath += `&employee_no=${encodeURIComponent(accessEventEmployeeNo.trim())}`;
    }
    accessEvents = await api(apiUrl, eventsPath);
    accessRules = await api<typeof accessRules>(apiUrl, '/api/v1/access/rules').catch(() => []);
    zoneOccupancy = await api<ZoneOccupancy[]>(apiUrl, '/api/v1/access/occupancy').catch(() => []);
    onOccupancyChange?.(zoneOccupancy);
    if (zones.length && !newDoor.zone_id) newDoor.zone_id = zones[0].id;
    if (zones.length && !newAccessRule.zone_id) newAccessRule.zone_id = zones[0].id;
    if (employees.length && !newAccessRule.employee_id) {
      newAccessRule.employee_id = employees[0].id;
    }
    if (employees.length && !newBadge.employee_id) newBadge.employee_id = employees[0].id;
  }

  export async function setDoorStatus(doorId: string, status: string) {
    await api(apiUrl, `/api/v1/access/doors/${doorId}/status`, {
      method: 'POST',
      body: JSON.stringify({ status }),
    });
    await refresh();
    await onDashboardChange?.();
    notify('success', 'Türstatus aktualisiert');
  }

  $effect(() => {
    if (active) void refresh();
  });

  $effect(() => {
    if (employees.length && !newBadge.employee_id) {
      newBadge.employee_id = employees[0].id;
    }
  });

  function suggestBadgeUid() {
    const emp = employees.find((e) => e.id === newBadge.employee_id);
    if (emp) newBadge = { ...newBadge, credential_uid: `DEMO-${emp.employee_no}` };
  }

  function copyText(text: string) {
    navigator.clipboard.writeText(text).then(() => notify('success', 'Kopiert'));
  }

  function fillSimulateUidFromEmployee() {
    const emp = employees.find((e) => e.id === simulateEmployeeId);
    if (emp) simulateUid = `DEMO-${emp.employee_no}`;
  }

  async function exportAccessLog(fmt: 'csv' | 'html') {
    try {
      let path = `/api/v1/reports/access/export?format=${fmt}`;
      if (accessExportFrom) {
        path += `&from=${encodeURIComponent(fromLocalDatetimeInputValue(accessExportFrom))}`;
      }
      if (accessExportTo) {
        path += `&to=${encodeURIComponent(fromLocalDatetimeInputValue(accessExportTo))}`;
      }
      const fromTag = accessExportFrom
        ? fromLocalDatetimeInputValue(accessExportFrom).slice(0, 10)
        : 'alle';
      const toTag = accessExportTo
        ? fromLocalDatetimeInputValue(accessExportTo).slice(0, 10)
        : 'alle';
      const base = `zutritt_protokoll_${fromTag}_${toTag}`;
      if (fmt === 'csv') await downloadFile(apiUrl, path, `${base}.csv`);
      else await openHtmlExport(apiUrl, path);
      notify('success', 'Zutritt-Export gestartet');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function simulateScan() {
    try {
      const res = await api<{ decision: string; reason_code: string }>(
        apiUrl,
        '/api/v1/access/simulate-scan',
        {
          method: 'POST',
          body: JSON.stringify({ credential_uid: simulateUid, reader_id: simulateReader }),
        },
      );
      await refresh();
      await onDashboardChange?.();
      notify('success', `Scan: ${accessDecisionLabel(res.decision)} (${reasonLabel(res.reason_code)})`);
      if (res.decision === 'grant' || res.decision === 'allow') {
        simulateReader =
          simulateReader === 'sim.reader.main' ? 'sim.reader.main.out' : 'sim.reader.main';
      }
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function fetchAccessEventsSince(since: string) {
    let path = `/api/v1/access/events?limit=20&since=${encodeURIComponent(since)}`;
    if (accessEventFilter !== 'all') path += `&decision=${accessEventFilter}`;
    return api<typeof accessEvents>(apiUrl, path);
  }

  async function hardwarePresentScan() {
    try {
      const since = new Date().toISOString();
      await api<{ queued: boolean }>(apiUrl, '/api/v1/access/hardware-present', {
        method: 'POST',
        body: JSON.stringify({ credential_uid: simulateUid, reader_id: simulateReader }),
      });
      let latest: (typeof accessEvents)[0] | undefined;
      for (let i = 0; i < 25; i++) {
        const batch = await fetchAccessEventsSince(since);
        if (batch.length > 0) {
          latest = batch[0];
          break;
        }
        await new Promise((r) => setTimeout(r, 200));
      }
      await refresh();
      await onDashboardChange?.();
      notify(
        'success',
        latest
          ? `Hardware-Kanal: ${accessDecisionLabel(latest.decision)} (${reasonLabel(latest.reason_code ?? '')})`
          : 'Hardware-Kanal: Timeout — Ereignis nicht sichtbar (Worker/TCP prüfen)',
      );
      if (latest?.decision === 'grant' || latest?.decision === 'allow') {
        simulateReader =
          simulateReader === 'sim.reader.main' ? 'sim.reader.main.out' : 'sim.reader.main';
      }
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function deleteAccessRule(id: string) {
    try {
      await api(apiUrl, `/api/v1/access/rules/${id}`, { method: 'DELETE' });
      await refresh();
      await onDashboardChange?.();
      notify('success', 'Zutrittsregel entfernt');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function updateAccessRuleValidTo(id: string, localValue: string) {
    try {
      const valid_to = localValue ? fromLocalDatetimeInputValue(localValue) : null;
      await api(apiUrl, `/api/v1/access/rules/${id}`, {
        method: 'PATCH',
        body: JSON.stringify({ valid_to }),
      });
      await refresh();
      await onDashboardChange?.();
      notify('success', valid_to ? 'Gültig-bis-Datum gespeichert' : 'Ablauf entfernt (unbegrenzt)');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function updateAccessRuleValidFrom(id: string, localValue: string) {
    if (!localValue) {
      notify('error', 'Gültig-ab-Datum erforderlich');
      return;
    }
    try {
      await api(apiUrl, `/api/v1/access/rules/${id}`, {
        method: 'PATCH',
        body: JSON.stringify({ valid_from: fromLocalDatetimeInputValue(localValue) }),
      });
      await refresh();
      await onDashboardChange?.();
      notify('success', 'Gültig-ab-Datum gespeichert');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function clearAccessRuleValidTo(id: string) {
    await updateAccessRuleValidTo(id, '');
  }

  async function duplicateAccessRule(r: {
    principal_id: string;
    zone_id?: string | null;
    schedule_json?: string | null;
    valid_to?: string | null;
    valid_from: string;
  }) {
    if (!r.zone_id) {
      notify('error', 'Zone fehlt');
      return;
    }
    try {
      const body: Record<string, unknown> = {
        employee_id: r.principal_id,
        zone_id: r.zone_id,
        schedule_json: r.schedule_json ?? null,
      };
      if (r.valid_to) body.valid_to = r.valid_to;
      await api(apiUrl, '/api/v1/access/rules', { method: 'POST', body: JSON.stringify(body) });
      await refresh();
      await onDashboardChange?.();
      notify('success', 'Zutrittsregel dupliziert');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function toggleAccessRuleSchedule(rule: { id: string; schedule_json?: string | null }) {
    try {
      const schedule_json = rule.schedule_json
        ? null
        : JSON.stringify({
            timezone: 'Europe/Berlin',
            weekdays: [1, 2, 3, 4, 5],
            start: '08:00',
            end: '18:00',
          });
      await api(apiUrl, `/api/v1/access/rules/${rule.id}`, {
        method: 'PATCH',
        body: JSON.stringify({ schedule_json }),
      });
      await refresh();
      await onDashboardChange?.();
      notify('success', schedule_json ? 'Zeitplan Mo–Fr 08:00–18:00 gesetzt' : 'Zeitplan entfernt');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function createAccessRule() {
    try {
      const schedule_json = newAccessRule.use_schedule
        ? JSON.stringify({
            timezone: 'Europe/Berlin',
            weekdays: [1, 2, 3, 4, 5],
            start: newAccessRule.start,
            end: newAccessRule.end,
          })
        : null;
      const body: Record<string, unknown> = {
        employee_id: newAccessRule.employee_id,
        zone_id: newAccessRule.zone_id,
        schedule_json,
      };
      if (newAccessRule.valid_from_local) {
        body.valid_from = fromLocalDatetimeInputValue(newAccessRule.valid_from_local);
      }
      if (newAccessRule.valid_to_local) {
        body.valid_to = fromLocalDatetimeInputValue(newAccessRule.valid_to_local);
      }
      await api(apiUrl, '/api/v1/access/rules', { method: 'POST', body: JSON.stringify(body) });
      await refresh();
      await onDashboardChange?.();
      newAccessRule.valid_to_local = '';
      notify('success', 'Zutrittsregel angelegt');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function revokeBadge(id: string) {
    try {
      await api(apiUrl, `/api/v1/access/badges/${id}/revoke`, { method: 'POST' });
      await refresh();
      notify('success', 'Badge gesperrt');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function createZone() {
    try {
      await api(apiUrl, '/api/v1/access/zones', { method: 'POST', body: JSON.stringify(newZone) });
      newZone = { name: '' };
      await refresh();
      notify('success', 'Zone angelegt');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function createDoor() {
    try {
      await api(apiUrl, '/api/v1/access/doors', { method: 'POST', body: JSON.stringify(newDoor) });
      await refresh();
      notify('success', 'Tür angelegt');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function createBadge() {
    try {
      await api(apiUrl, '/api/v1/access/badges', {
        method: 'POST',
        body: JSON.stringify({ ...newBadge, credential_type: 'card' }),
      });
      newBadge = { ...newBadge, credential_uid: '' };
      await refresh();
      notify('success', 'Badge angelegt');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }
</script>

<TsPageHeader
  title="Zutritt"
  lead="Zonen, Türen, Regeln und Simulator. Tür wählen, Status setzen, Scans testen."
/>
<div class="card" style="margin-top: 1rem;">
  <h3>Neue Zone</h3>
  <div class="grid-form">
    <input bind:value={newZone.name} placeholder="Zonenname" />
    <button type="button" onclick={createZone}>Zone anlegen</button>
  </div>
  <h3 style="margin-top: 1rem;">Neue Tür</h3>
  <div class="grid-form">
    <input bind:value={newDoor.name} placeholder="Türname" />
    <select bind:value={newDoor.zone_id}>
      {#each zones as z}<option value={z.id}>{z.name}</option>{/each}
    </select>
    <input bind:value={newDoor.reader_id} placeholder="Reader-ID" />
    <button type="button" onclick={createDoor}>Tür anlegen</button>
  </div>
  <h3 style="margin-top: 1rem;">Badge ausgeben</h3>
  <div class="grid-form">
    <select bind:value={newBadge.employee_id}>
      {#each employees as e}
        <option value={e.id}>{e.employee_no} — {e.display_name}</option>
      {/each}
    </select>
    <input bind:value={newBadge.credential_uid} placeholder="Credential UID" />
    <button class="secondary" type="button" onclick={suggestBadgeUid}>DEMO-PN vorschlagen</button>
    <button type="button" onclick={createBadge}>Badge anlegen</button>
  </div>
</div>
<div class="card" style="margin-top: 1rem;">
  <h3>Zutrittsregel</h3>
  <div class="grid-form">
    <select bind:value={newAccessRule.employee_id}>
      {#each employees as e}
        <option value={e.id}>{e.employee_no} — {e.display_name}</option>
      {/each}
    </select>
    <select bind:value={newAccessRule.zone_id}>
      {#each zones as z}<option value={z.id}>{z.name}</option>{/each}
    </select>
    <label class="muted">
      <input type="checkbox" bind:checked={newAccessRule.use_schedule} />
      Mo–Fr Zeitfenster
    </label>
    {#if newAccessRule.use_schedule}
      <input bind:value={newAccessRule.start} placeholder="08:00" />
      <input bind:value={newAccessRule.end} placeholder="18:00" />
    {/if}
    <label class="muted" for="new-access-rule-from">Gültig ab (optional, sonst jetzt)</label>
    <input id="new-access-rule-from" type="datetime-local" bind:value={newAccessRule.valid_from_local} />
    <label class="muted" for="new-access-rule-to">Gültig bis (optional)</label>
    <input id="new-access-rule-to" type="datetime-local" bind:value={newAccessRule.valid_to_local} />
    <button type="button" onclick={createAccessRule}>Regel (Allow) anlegen</button>
  </div>
  <div class="btn-row" style="margin-top: 0.5rem;">
    <select bind:value={accessRuleEmployeeFilter}>
      <option value="">Alle Mitarbeiter</option>
      {#each employees.filter((e) => e.active !== false) as e}
        <option value={e.id}>{e.employee_no} — {e.display_name}</option>
      {/each}
    </select>
    <select bind:value={accessRuleZoneFilter}>
      <option value="">Alle Zonen</option>
      {#each zones as z}
        <option value={z.id}>{z.name}</option>
      {/each}
    </select>
  </div>
  <ul class="compact-list" style="margin-top: 0.75rem;">
    {#each displayedAccessRules as r}
      <li class="row-card">
        {r.employee_name ?? '—'} → {r.zone_name ?? 'Zone'} ({r.mode})
        {#if r.schedule_json}<span class="muted"> — Zeitplan aktiv</span>{/if}
        <span class="muted">
          — ab {formatIsoLocalShort(r.valid_from)}
          {#if r.valid_to}
            bis {formatIsoLocalShort(r.valid_to)}
          {:else}
            (unbegrenzt)
          {/if}
        </span>
        <div class="grid-form" style="margin-top: 0.35rem;">
          <label class="muted" for="access-rule-from-{r.id}">Gültig ab</label>
          <input
            id="access-rule-from-{r.id}"
            type="datetime-local"
            value={isoToLocalDatetimeInput(r.valid_from)}
            onchange={(ev) => updateAccessRuleValidFrom(r.id, ev.currentTarget.value)}
          />
          <label class="muted" for="access-rule-to-{r.id}">Gültig bis</label>
          <input
            id="access-rule-to-{r.id}"
            type="datetime-local"
            value={isoToLocalDatetimeInput(r.valid_to)}
            onchange={(ev) => updateAccessRuleValidTo(r.id, ev.currentTarget.value)}
          />
          {#if r.valid_to}
            <button class="secondary" type="button" onclick={() => clearAccessRuleValidTo(r.id)}>
              Unbegrenzt
            </button>
          {/if}
        </div>
        <button class="secondary" type="button" onclick={() => toggleAccessRuleSchedule(r)}>
          {r.schedule_json ? 'Zeitplan aus' : 'Mo–Fr 08–18'}
        </button>
        <button class="secondary" type="button" onclick={() => duplicateAccessRule(r)}>Duplizieren</button>
        <button class="secondary" type="button" onclick={() => deleteAccessRule(r.id)}>Entfernen</button>
      </li>
    {:else}
      <li class="muted">Keine Regeln für diesen Filter.</li>
    {/each}
  </ul>
  <h3 style="margin-top: 1rem;">Belegung</h3>
  {#each zoneOccupancy as z}
    <p>
      <strong>{z.zone_name}</strong>: {z.inside_count} im Gebäude
      {#if z.occupants.length}
        — {z.occupants.map((o) => o.display_name).join(', ')}
      {/if}
    </p>
  {/each}
</div>
<div class="card" style="margin-top: 1rem;">
  <h3>Türen</h3>
  {#if doors.length === 0}
    <TsEmptyState message="Noch keine Türen — oben Zone und Tür anlegen." />
  {:else}
    <div class="period-split" style="margin-top: 0.75rem;">
      <ul class="pick-list" role="listbox" aria-label="Türen">
        {#each doors as d}
          <li>
            <button
              type="button"
              class="pick-item"
              class:selected={selectedDoorId === d.id}
              role="option"
              aria-selected={selectedDoorId === d.id}
              onclick={() => (selectedDoorId = d.id)}
            >
              <span class="pick-title">{d.name}</span>
              <span class="pick-meta">{doorStatusLabel(d.status)}</span>
            </button>
          </li>
        {/each}
      </ul>
      <div class="period-editor-pane">
        {#if selectedDoor}
          <h4 style="margin: 0 0 0.5rem;">{selectedDoor.name}</h4>
          <p class="muted" style="margin: 0 0 0.75rem;">
            Status: <strong>{doorStatusLabel(selectedDoor.status)}</strong>
          </p>
          {#if selectedDoor.reader_in_id || selectedDoor.reader_out_id}
            <p class="muted" style="font-size: 0.85rem; margin: 0 0 0.75rem;">
              {#if selectedDoor.reader_in_id}
                Eingang: <code>{selectedDoor.reader_in_id}</code>
              {/if}
              {#if selectedDoor.reader_in_id && selectedDoor.reader_out_id} · {/if}
              {#if selectedDoor.reader_out_id}
                Ausgang: <code>{selectedDoor.reader_out_id}</code>
              {/if}
            </p>
          {/if}
          <div class="btn-row">
            <button
              class="secondary"
              type="button"
              onclick={() => setDoorStatus(selectedDoor.id, 'closed')}
            >
              Zu
            </button>
            <button
              class="secondary"
              type="button"
              onclick={() => setDoorStatus(selectedDoor.id, 'open')}
            >
              Auf
            </button>
            <button
              class="secondary"
              type="button"
              onclick={() => setDoorStatus(selectedDoor.id, 'forced_open')}
            >
              Offen
            </button>
            <button
              class="secondary"
              type="button"
              onclick={() => setDoorStatus(selectedDoor.id, 'alarm')}
            >
              Alarm
            </button>
          </div>
        {:else}
          <TsEmptyState message="Tür in der Liste wählen, um Status und Leser zu bearbeiten." />
        {/if}
      </div>
    </div>
  {/if}
</div>
<div class="card" style="margin-top: 1rem;">
  <h3>Simulator</h3>
  <div class="grid-form" style="margin-bottom: 0.5rem;">
    <label class="muted" for="access-export-from">Export von</label>
    <input id="access-export-from" type="datetime-local" bind:value={accessExportFrom} />
    <label class="muted" for="access-export-to">Export bis</label>
    <input id="access-export-to" type="datetime-local" bind:value={accessExportTo} />
  </div>
  <div class="btn-row" style="margin-bottom: 0.5rem;">
    <button class="secondary" type="button" onclick={() => exportAccessLog('csv')}>Zutritt CSV</button>
    <button class="secondary" type="button" onclick={() => exportAccessLog('html')}>Zutritt HTML/PDF</button>
  </div>
  <p class="muted" style="font-size: 0.85rem; margin: 0 0 0.75rem;">
    Export enthält alle sichtbaren Ereignisse (Admin/HR/Manager/Sicherheit). Optional Zeitraum oben
    einschränken.
  </p>
  <select
    bind:value={simulateEmployeeId}
    onchange={fillSimulateUidFromEmployee}
    style="margin-bottom: 0.5rem;"
  >
    <option value="">Mitarbeiter für DEMO-UID…</option>
    {#each employees.filter((e) => e.active !== false) as e}
      <option value={e.id}>{e.employee_no} — {e.display_name}</option>
    {/each}
  </select>
  <input bind:value={simulateUid} placeholder="Credential UID" />
  <div class="btn-row" style="margin-top: 0.5rem;">
    <button class="secondary" type="button" onclick={() => (simulateUid = 'DEMO-ADMIN-001')}>
      DEMO-ADMIN-001
    </button>
    <button class="secondary" type="button" onclick={() => (simulateUid = 'DEMO-0002')}>DEMO-0002</button>
    <button class="secondary" type="button" onclick={() => (simulateUid = 'DEMO-0003')}>DEMO-0003</button>
  </div>
  <select bind:value={simulateReader} style="margin-top: 0.5rem;">
    {#if doorReaders.length === 0}
      <option value="sim.reader.main">Eingang (sim.reader.main)</option>
      <option value="sim.reader.main.out">Ausgang (sim.reader.main.out)</option>
    {:else}
      {#each doorReaders as r}
        <option value={r.id}>{r.label} ({r.id})</option>
      {/each}
    {/if}
  </select>
  <div class="btn-row" style="margin-top: 0.5rem;">
    <button type="button" onclick={simulateScan}>Scan (REST)</button>
    <button class="secondary" type="button" onclick={hardwarePresentScan}>Scan (Hardware-Kanal)</button>
  </div>
  <p class="muted" style="margin-top: 0.5rem;">
    REST = sofortige Antwort. Hardware-Kanal = wie ein echter Leser (für
    <code>TIMESHARDS_HW_ADAPTER=external</code>). Zweiter Eingangs-Scan ohne Ausgang → Anti-Passback.
  </p>
  {#if apiHealth?.hardware_tcp_listen}
    <p class="muted" style="margin-top: 0.35rem; font-size: 0.9rem;">
      TCP-Ingest aktiv: <code>{apiHealth.hardware_tcp_listen}</code> (JSON / kompakte Zeilen, siehe
      docs/HARDWARE.md). Tür-Status-Updates erscheinen in der Türliste und auf der Übersicht.
    </p>
  {/if}
  <h3 style="margin-top: 1rem;">Badges</h3>
  <ul class="compact-list">
    {#each badges as b}
      <li>
        <code>{b.credential_uid}</code>
        <button class="secondary" type="button" onclick={() => copyText(b.credential_uid)}>Kopieren</button>
        {#if b.employee_name}
          — {b.employee_no} {b.employee_name}
        {/if}
        — {b.status}
        {#if b.status === 'active'}
          <button class="secondary" type="button" onclick={() => revokeBadge(b.id)}>Sperren</button>
        {/if}
      </li>
    {/each}
  </ul>
  <h3>Letzte Ereignisse</h3>
  <div class="btn-row" style="margin-bottom: 0.5rem;">
    <select bind:value={accessEventFilter} onchange={() => refresh()}>
      <option value="all">Alle</option>
      <option value="grant">Nur Zutritt</option>
      <option value="deny">Nur Abgelehnt</option>
    </select>
    <input
      bind:value={accessEventEmployeeNo}
      placeholder="PN filtern (z.B. 0002)"
      onchange={() => refresh()}
    />
  </div>
  <ul class="compact-list">
    {#each accessEvents as ev}
      <li>
        <span class="muted">{formatIsoLocalShort(ev.occurred_at)}</span>
        {ev.employee_name ?? '—'} ({ev.employee_no ?? '—'})
        — {ev.zone_name ?? '—'} / {ev.door_name ?? '—'}
        — <strong>{accessDecisionLabel(ev.decision)}</strong>
        ({reasonLabel(ev.reason_code ?? '')})
      </li>
    {/each}
  </ul>
</div>
