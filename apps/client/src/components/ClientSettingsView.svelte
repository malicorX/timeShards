<script lang="ts">
  import type { ServerHealth } from '../lib/serverHealth';

  let {
    serverUrl = $bindable(''),
    serverHealth = null,
    onCheckHealth,
    onSave,
  }: {
    serverUrl?: string;
    serverHealth?: ServerHealth | null;
    onCheckHealth?: () => void;
    onSave?: () => void;
  } = $props();
</script>

<div class="content" style="max-width: 420px; margin: 4rem auto;">
  <h2>Server-Verbindung</h2>
  <input bind:value={serverUrl} onchange={() => onCheckHealth?.()} />
  {#if serverHealth}
    <p class="muted" style="margin-top: 0.5rem;">
      API v{serverHealth.version} — {serverHealth.status === 'ok' ? 'erreichbar' : serverHealth.status}
    </p>
    {#if serverHealth.default_password_login_blocked}
      <p class="muted" style="margin-top: 0.5rem; font-size: 0.85rem;">
        Standardpasswörter werden abgelehnt — nur geänderte Zugangsdaten nutzen.
      </p>
    {/if}
    {#if serverHealth.hardware_adapter}
      <p class="muted" style="margin-top: 0.25rem; font-size: 0.85rem;">
        Hardware-Adapter: <code>{serverHealth.hardware_adapter}</code>
        {#if serverHealth.hardware_adapter_configured}
          (konfiguriert: <code>{serverHealth.hardware_adapter_configured}</code>)
        {/if}
        {#if serverHealth.hardware_tcp_listen}
          — TCP <code>{serverHealth.hardware_tcp_listen}</code>
        {/if}
      </p>
    {/if}
  {:else}
    <p class="error" style="margin-top: 0.5rem;">Server nicht erreichbar</p>
  {/if}
  <button type="button" style="margin-top: 1rem;" onclick={() => onSave?.()}>Speichern & weiter</button>
</div>
