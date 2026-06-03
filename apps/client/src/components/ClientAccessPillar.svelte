<script lang="ts">
  import { api, downloadFile } from '../lib/api';
  import { formatIsoShort } from '../lib/datetime';
  import { accessDecisionLabel, reasonLabel } from '../lib/accessLabels';

  type UiMessage = { type: 'error' | 'success'; text: string };

  let {
    serverUrl,
    canExportAccess = false,
    active = false,
    onMessage,
    onRefreshParent,
  }: {
    serverUrl: string;
    canExportAccess?: boolean;
    active?: boolean;
    onMessage?: (msg: UiMessage) => void;
    onRefreshParent?: () => void | Promise<void>;
  } = $props();

  let accessSummary = $state<{
    badges: { credential_uid: string; status: string }[];
    recent_events: {
      decision: string;
      reason_code: string;
      zone_name?: string;
      occurred_at: string;
    }[];
    readers?: { id: string; label: string }[];
  } | null>(null);
  let clientSimReader = $state('sim.reader.main');

  function notify(type: 'error' | 'success', text: string) {
    onMessage?.({ type, text });
  }

  export async function refresh() {
    accessSummary = await api<typeof accessSummary>(serverUrl, '/api/v1/access/me').catch(() => null);
    const readers = accessSummary?.readers?.map((r) => r.id) ?? [];
    if (readers.length > 0 && !readers.includes(clientSimReader)) {
      clientSimReader = readers[0];
    }
  }

  $effect(() => {
    if (active) void refresh();
  });

  async function exportAccessLog() {
    try {
      await downloadFile(serverUrl, '/api/v1/reports/access/export?format=csv&limit=500', 'zutritt.csv');
      notify('success', 'Zutritt-Protokoll exportiert');
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  async function simulateMyBadge() {
    const uid = accessSummary?.badges.find((b) => b.status === 'active')?.credential_uid;
    if (!uid) {
      notify('error', 'Kein aktives Badge');
      return;
    }
    try {
      const res = await api<{ decision: string; reason_code: string }>(
        serverUrl,
        '/api/v1/access/me/simulate-scan',
        {
          method: 'POST',
          body: JSON.stringify({ credential_uid: uid, reader_id: clientSimReader }),
        },
      );
      await refresh();
      await onRefreshParent?.();
      notify('success', `${accessDecisionLabel(res.decision)} (${reasonLabel(res.reason_code)})`);
      if (res.decision === 'grant' || res.decision === 'allow') {
        clientSimReader =
          clientSimReader === 'sim.reader.main' ? 'sim.reader.main.out' : 'sim.reader.main';
      }
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
    }
  }

  function copyUid(uid: string) {
    navigator.clipboard.writeText(uid).then(() => notify('success', 'UID kopiert'));
  }
</script>

<h2>Zutritt</h2>
{#if accessSummary}
  <div class="card">
    <h3>Meine Karte</h3>
    {#each accessSummary.badges as b}
      <p>
        <code>{b.credential_uid}</code>
        <button class="secondary" type="button" onclick={() => copyUid(b.credential_uid)}>Kopieren</button>
        — {b.status}
      </p>
    {:else}
      <p class="muted">Kein Badge zugewiesen</p>
    {/each}
    {#if accessSummary.badges.some((b) => b.status === 'active')}
      <p class="muted" style="margin-top: 0.5rem;">
        Demo: zweiter Scan am Eingang ohne Ausgang löst Anti-Passback aus. Nach erfolgreichem Scan wechselt der
        Reader automatisch (Eingang ↔ Ausgang).
      </p>
      <select bind:value={clientSimReader} style="margin-top: 0.5rem;">
        {#if accessSummary.readers?.length}
          {#each accessSummary.readers as r}
            <option value={r.id}>{r.label}</option>
          {/each}
        {:else}
          <option value="sim.reader.main">Eingang</option>
          <option value="sim.reader.main.out">Ausgang</option>
        {/if}
      </select>
      <button type="button" style="margin-top: 0.5rem;" onclick={simulateMyBadge}>Scan simulieren</button>
    {/if}
    {#if canExportAccess}
      <button class="secondary" type="button" style="margin-top: 0.5rem;" onclick={exportAccessLog}>
        Zutritt-Protokoll (CSV)
      </button>
    {/if}
  </div>
  <div class="card" style="margin-top: 1rem;">
    <h3>Letzte Zutritte</h3>
    <ul>
      {#each accessSummary.recent_events as ev}
        <li>
          {accessDecisionLabel(ev.decision)} — {reasonLabel(ev.reason_code)}
          {#if ev.zone_name}<span class="muted"> ({ev.zone_name})</span>{/if}
          — {formatIsoShort(ev.occurred_at)}
        </li>
      {:else}
        <li class="muted">Noch keine Ereignisse</li>
      {/each}
    </ul>
  </div>
{:else}
  <p class="muted">Zutrittsdaten werden geladen…</p>
{/if}
