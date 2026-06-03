<script lang="ts">
  type SetupNavigate = {
    tab: 'personnel' | 'time' | 'access';
    timeSection?: 'stammdaten' | 'planung' | 'stundenzettel' | 'abschluss';
  };

  let {
    apiHealth = null,
    onNavigate,
    onOpenWizard,
  }: {
    apiHealth?: {
      demo_seeding_enabled?: boolean;
      time_foundation?: { active_employees: number };
    } | null;
    onNavigate?: (target: SetupNavigate) => void;
    onOpenWizard?: () => void;
  } = $props();

  const show = $derived(
    !!apiHealth &&
      !apiHealth.demo_seeding_enabled &&
      (apiHealth.time_foundation?.active_employees ?? 0) < 2,
  );
</script>

{#if show}
  <div class="card setup-guide" style="margin-top: 1rem; border-color: #2563eb;">
    <h3 style="margin-top: 0;">Ersteinrichtung</h3>
    <p class="muted" style="font-size: 0.9rem;">
      Produktionsmodus ohne Demo — legen Sie Ihr Team an und prüfen Sie die Zeitbasis, bevor Sie Stempeln
      und Freigaben testen.
    </p>
    <ol class="compact-list" style="margin: 0.5rem 0;">
      <li><strong>Personal</strong> — Mitarbeiter, Benutzer, Badges (Arbeitskalender standardmäßig)</li>
      <li><strong>Zeit</strong> — Arbeitskalender und Tagesmodelle (Standard-Seed meist vorhanden)</li>
      <li><strong>Zutritt</strong> — optional: Zonen, Türen, Regeln</li>
      <li><strong>Go-Live</strong> — Checkliste und Assistent unten</li>
    </ol>
    <div class="btn-row">
      <button type="button" onclick={() => onNavigate?.({ tab: 'personnel' })}>Zum Personal</button>
      <button
        type="button"
        class="secondary"
        onclick={() => onNavigate?.({ tab: 'time', timeSection: 'stammdaten' })}
      >
        Zeit → Stammdaten
      </button>
      {#if onOpenWizard}
        <button type="button" class="secondary" onclick={onOpenWizard}>Go-Live-Assistent</button>
      {/if}
    </div>
    <p class="muted" style="font-size: 0.8rem; margin-bottom: 0;">
      Leitfaden: <code>docs/PILOT.md</code> · <code>docs/PRODUCTION.md</code>
    </p>
  </div>
{/if}
