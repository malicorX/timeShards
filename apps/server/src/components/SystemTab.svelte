<script lang="ts">
  import { api } from '../lib/api';
  import { formatIsoLocalShort } from '../lib/datetime';
  import TsPageHeader from '@timeshards/shared/ui/TsPageHeader.svelte';

  type UiMessage = { type: 'error' | 'success'; text: string };

  let {
    apiUrl,
    apiBind,
    dbPath,
    apiHealth = null,
    active = false,
    onMessage,
    onRefreshHealth,
  }: {
    apiUrl: string;
    apiBind: string;
    dbPath: string;
    apiHealth?: {
      status: string;
      version: string;
      database: string;
      service: string;
      demo_seeding_enabled?: boolean;
      default_password_login_blocked?: boolean;
      hardware_adapter?: string;
      hardware_adapter_configured?: string | null;
      hardware_tcp_listen?: string | null;
    } | null;
    active?: boolean;
    onMessage?: (msg: UiMessage) => void;
    onRefreshHealth?: () => void | Promise<void>;
  } = $props();

  let roles = $state<{ id: string; name: string }[]>([]);
  let sites = $state<{ id: string; name: string; timezone: string }[]>([]);
  let auditLog = $state<
    {
      actor_type: string;
      action: string;
      object_type: string;
      object_id?: string | null;
      occurred_at: string;
      reason?: string;
    }[]
  >([]);
  let auditLimit = $state(100);
  let auditObjectFilter = $state('');
  let auditActionFilter = $state('');
  let auditActorFilter = $state('');
  let currentPassword = $state('');
  let newPassword = $state('');

  function notify(type: 'error' | 'success', text: string) {
    onMessage?.({ type, text });
  }

  export async function refresh() {
    const params = new URLSearchParams({ limit: String(Math.min(500, Math.max(1, auditLimit))) });
    if (auditObjectFilter) params.set('object_type', auditObjectFilter);
    if (auditActionFilter) params.set('action', auditActionFilter);
    if (auditActorFilter) params.set('actor_type', auditActorFilter);
    auditLog = await api<typeof auditLog>(apiUrl, `/api/v1/admin/audit?${params}`).catch(() => []);
    sites = await api<typeof sites>(apiUrl, '/api/v1/admin/sites').catch(() => []);
    roles = await api<typeof roles>(apiUrl, '/api/v1/admin/roles').catch(() => []);
    await onRefreshHealth?.();
  }

  $effect(() => {
    if (active) void refresh();
  });

  async function changePassword() {
    try {
      await api(apiUrl, '/api/v1/auth/change-password', {
        method: 'POST',
        body: JSON.stringify({
          current_password: currentPassword,
          new_password: newPassword,
        }),
      });
      currentPassword = '';
      newPassword = '';
      notify('success', 'Passwort geändert');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }
</script>

<TsPageHeader
  title="System"
  lead="API-Status, Audit-Log, Standorte und eigenes Passwort. Produktion: Demo aus, starke Passwörter."
/>
<div class="card" style="margin-bottom: 1rem;">
  <h3>API-Status</h3>
  {#if apiHealth}
    <p>
      {apiHealth.service} v{apiHealth.version} — <strong>{apiHealth.status}</strong>
      (Datenbank: {apiHealth.database})
    </p>
    {#if apiHealth.demo_seeding_enabled || apiHealth.default_password_login_blocked}
      <p class="error" style="margin-top: 0.5rem; font-size: 0.9rem;">
        {#if apiHealth.default_password_login_blocked}
          Standardpasswörter sind an der API gesperrt. Passwort unter „Passwort ändern“ unten setzen oder
          Admin-Zurücksetzen nutzen.
        {/if}
        {#if apiHealth.demo_seeding_enabled}
          Demo-Seeding aktiv (`demo`/`manager`). Produktion: <code>TIMESHARDS_DISABLE_DEMO=1</code>.
        {/if}
      </p>
    {/if}
  {:else}
    <p class="muted">Nicht erreichbar</p>
  {/if}
  <p class="muted" style="margin-top: 0.5rem;">Bind: {apiBind || '…'}</p>
  <p class="muted">DB: {dbPath || '…'}</p>
  {#if apiHealth?.hardware_adapter}
    <p class="muted">
      Hardware-Adapter: <code>{apiHealth.hardware_adapter}</code>
      {#if apiHealth.hardware_adapter_configured}
        <span class="muted">
          (konfiguriert: <code>{apiHealth.hardware_adapter_configured}</code> — Fallback aktiv)
        </span>
      {/if}
      {#if apiHealth.hardware_tcp_listen}
        — TCP <code>{apiHealth.hardware_tcp_listen}</code>
      {/if}
    </p>
  {/if}
  <button class="secondary" type="button" style="margin-top: 0.5rem;" onclick={onRefreshHealth}>
    Erneut prüfen
  </button>
  <p class="muted" style="margin-top: 0.75rem; font-size: 0.9rem;">
    OpenAPI:
    <a href="{apiUrl}/api/v1/openapi.json" target="_blank" rel="noopener noreferrer">
      {apiUrl}/api/v1/openapi.json
    </a>
    (Swagger UI: Import URL)
  </p>
</div>
<div class="card" style="margin-bottom: 1rem;">
  <h3>Demo-Zugänge</h3>
  <p class="muted" style="font-size: 0.9rem;">
    Client: <code>demo</code> / <code>demo</code> (PN 0002, Badge DEMO-0002) — <code>manager</code> /
    <code>demo</code> (Freigaben, DEMO-0003). Server: <code>admin</code> / <code>admin</code>
    (DEMO-ADMIN-001).
  </p>
</div>
{#if roles.length}
  <div class="card" style="margin-bottom: 1rem;">
    <h3>Rollen</h3>
    <ul class="compact-list">
      {#each roles as r}
        <li>{r.name}</li>
      {/each}
    </ul>
  </div>
{/if}
{#if sites.length}
  <div class="card" style="margin-bottom: 1rem;">
    <h3>Standorte</h3>
    <ul class="compact-list">
      {#each sites as s}
        <li>{s.name} — <span class="muted">{s.timezone}</span></li>
      {/each}
    </ul>
  </div>
{/if}
<div class="card" style="margin-bottom: 1rem;">
  <h3>Audit-Log</h3>
  <div class="grid-form" style="margin-bottom: 0.5rem;">
    <input bind:value={auditActorFilter} placeholder="Akteur (z.B. user, hardware)" />
    <input bind:value={auditObjectFilter} placeholder="Objekt (z.B. door, employee)" />
    <input bind:value={auditActionFilter} placeholder="Aktion (z.B. update, reader_offline)" />
    <input
      type="number"
      min="1"
      max="500"
      bind:value={auditLimit}
      placeholder="Limit"
      style="max-width: 6rem;"
    />
    <button class="secondary" type="button" onclick={() => refresh()}>Filtern</button>
  </div>
  <ul class="compact-list">
    {#each auditLog as a}
      <li>
        {formatIsoLocalShort(a.occurred_at)} — <span class="muted">{a.actor_type}</span>
        {a.action} {a.object_type}
        {#if a.object_id}<span class="muted"> #{a.object_id.slice(0, 8)}</span>{/if}
        {#if a.reason}<span class="muted"> ({a.reason})</span>{/if}
      </li>
    {:else}
      <li class="muted">Keine Einträge</li>
    {/each}
  </ul>
</div>
<div class="card">
  <h3>Passwort ändern</h3>
  <input type="password" bind:value={currentPassword} placeholder="Aktuelles Passwort" />
  <input type="password" bind:value={newPassword} placeholder="Neues Passwort (min. 6)" />
  <button type="button" style="margin-top: 0.5rem;" onclick={changePassword}>Speichern</button>
</div>
