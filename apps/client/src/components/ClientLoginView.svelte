<script lang="ts">
  import type { ServerHealth } from '../lib/serverHealth';

  let {
    serverUrl,
    serverHealth = null,
    username = $bindable(''),
    password = $bindable(''),
    error = '',
    onLogin,
    onOpenSettings,
  }: {
    serverUrl: string;
    serverHealth?: ServerHealth | null;
    username?: string;
    password?: string;
    error?: string;
    onLogin?: () => void;
    onOpenSettings?: () => void;
  } = $props();

  const showDemoShortcuts = $derived(
    serverHealth?.demo_seeding_enabled !== false &&
      !serverHealth?.default_password_login_blocked,
  );
</script>

<div class="content" style="max-width: 420px; margin: 4rem auto;">
  <h2>Anmelden</h2>
  <p class="muted">{serverUrl}</p>
  {#if serverHealth}
    <p class="muted" style="font-size: 0.85rem;">
      API {serverHealth.status === 'ok' ? 'bereit' : serverHealth.status} (v{serverHealth.version})
    </p>
    {#if serverHealth.default_password_login_blocked}
      <p class="muted" style="font-size: 0.85rem; margin-top: 0.35rem;">
        Standardpasswörter gesperrt — nur geänderte Zugangsdaten.
      </p>
    {/if}
  {:else}
    <p class="error" style="font-size: 0.85rem;">API nicht erreichbar - URL prüfen</p>
  {/if}
  <input
    bind:value={username}
    placeholder="Benutzername"
    onkeydown={(e) => e.key === 'Enter' && onLogin?.()}
  />
  <input
    type="password"
    bind:value={password}
    placeholder="Passwort"
    style="margin-top: 0.5rem;"
    onkeydown={(e) => e.key === 'Enter' && onLogin?.()}
  />
  {#if showDemoShortcuts}
    <div class="btn-row" style="margin-top: 0.75rem;">
      <button type="button" class="secondary" onclick={() => { username = 'demo'; password = 'demo'; }}
        >demo</button
      >
      <button
        type="button"
        class="secondary"
        onclick={() => {
          username = 'manager';
          password = 'demo';
        }}>manager</button
      >
      <button type="button" class="secondary" onclick={() => { username = 'admin'; password = 'admin'; }}
        >admin</button
      >
    </div>
    <p class="muted" style="margin-top: 0.75rem; font-size: 0.85rem;">
      Demo: <code>demo</code>/<code>demo</code> (Zeit, DEMO-0002) ·
      <code>manager</code>/<code>demo</code> (Freigaben, Zutritt DEMO-0003)
    </p>
  {/if}
  <button type="button" style="margin-top: 1rem;" onclick={() => onLogin?.()}>Anmelden</button>
  <button type="button" class="secondary" style="margin-top: 0.5rem;" onclick={() => onOpenSettings?.()}>
    Server ändern
  </button>
  {#if error}<p class="error">{error}</p>{/if}
</div>
