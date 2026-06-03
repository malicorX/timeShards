<script lang="ts">
  import type { OverviewNavigate } from './OverviewTab.svelte';
  import {
    computeProductionChecklist,
    checklistAllOk,
    type ProductionHealth,
    type ProductionDashboard,
  } from '../lib/productionChecklist';

  const WIZARD_DISMISS_KEY = 'timeshards.production_wizard_dismissed';

  let {
    open = $bindable(false),
    apiHealth = null,
    dashboard = null,
    onNavigate,
    onFoundationFix,
    onRefresh,
  }: {
    open?: boolean;
    apiHealth?: ProductionHealth | null;
    dashboard?: ProductionDashboard | null;
    onNavigate?: (target: OverviewNavigate) => void;
    onFoundationFix?: () => void | Promise<void>;
    onRefresh?: () => void | Promise<void>;
  } = $props();

  let dialogEl = $state<HTMLDialogElement | null>(null);
  let step = $state(0);
  let foundationBusy = $state(false);

  const items = $derived(computeProductionChecklist(apiHealth, dashboard));
  const allOk = $derived(checklistAllOk(items));
  const openCount = $derived(items.filter((i) => !i.ok).length);

  const steps = [
    { title: 'Go-Live-Assistent', body: 'intro' },
    { title: 'Umgebung', body: 'env' },
    { title: 'Zeitbasis', body: 'foundation' },
    { title: 'Personal', body: 'personnel' },
    { title: 'Stundenzettel', body: 'timesheets' },
    { title: 'Zeit ↔ Zutritt', body: 'mismatch' },
    { title: 'Abschluss', body: 'done' },
  ] as const;

  const lastStep = steps.length - 1;

  $effect(() => {
    if (open && dialogEl && !dialogEl.open) dialogEl.showModal();
    if (!open && dialogEl?.open) dialogEl.close();
  });

  function closeWizard(dismiss = false) {
    if (dismiss && typeof localStorage !== 'undefined') {
      localStorage.setItem(WIZARD_DISMISS_KEY, '1');
    }
    open = false;
    step = 0;
  }

  async function runFoundationFix() {
    foundationBusy = true;
    try {
      await onFoundationFix?.();
      await onRefresh?.();
    } finally {
      foundationBusy = false;
    }
  }

  function itemById(id: (typeof items)[number]['id']) {
    return items.find((i) => i.id === id);
  }
</script>

<dialog
  bind:this={dialogEl}
  class="wizard-dialog"
  onclose={() => {
    open = false;
    step = 0;
  }}
>
  <div class="wizard-panel">
    <header class="wizard-header">
      <h3>{steps[step].title}</h3>
      <p class="muted" style="margin: 0.25rem 0 0; font-size: 0.85rem;">
        Schritt {step + 1} von {steps.length}
        {#if openCount > 0 && step > 0 && step < lastStep}
          · {openCount} offen
        {/if}
      </p>
    </header>

    <div class="wizard-body">
      {#if steps[step].body === 'intro'}
        <p>
          Dieser Assistent führt durch die wichtigsten Punkte vor dem produktiven Betrieb. Details stehen in
          <code>docs/PRODUCTION.md</code>.
        </p>
        <ul class="checklist compact">
          {#each items as item}
            <li class:ok={item.ok} class:warn={!item.ok}>
              <span class="check-icon" aria-hidden="true">{item.ok ? '✓' : '○'}</span>
              {item.label}
            </li>
          {/each}
        </ul>
      {:else if steps[step].body === 'env'}
        <p>Server-Umgebung für Produktion oder dokumentiertes Staging:</p>
        <ul class="compact-list">
          <li class:ok={itemById('demo')?.ok}>
            <strong>Demo:</strong>
            {#if apiHealth?.demo_seeding_enabled === false}
              deaktiviert
            {:else}
              aktiv — <code>TIMESHARDS_DISABLE_DEMO=1</code>
            {/if}
          </li>
          <li class:ok={itemById('passwords')?.ok}>
            <strong>Passwörter:</strong>
            {#if apiHealth?.default_password_login_blocked}
              Standardpasswörter blockiert
            {:else}
              Demo-Passwörter möglich — <code>TIMESHARDS_BLOCK_DEFAULT_PASSWORDS=1</code>
            {/if}
          </li>
        </ul>
        <p class="muted" style="font-size: 0.85rem;">
          Initiales Admin-Passwort bei leerer DB: <code>TIMESHARDS_ADMIN_PASSWORD</code>
        </p>
      {:else if steps[step].body === 'foundation'}
        <p>
          Weist fehlende Standard-Arbeitskalender zu und berechnet die aktuelle Kalenderwoche neu (Soll aus
          Tagesmodell).
        </p>
        {#if itemById('calendar')?.ok && itemById('soll')?.ok}
          <p class="success">Zeitbasis-KPIs sind grün.</p>
        {:else}
          <p>
            {#if !itemById('calendar')?.ok}
              {itemById('calendar')?.hint}
            {/if}
            {#if !itemById('soll')?.ok}
              {#if !itemById('calendar')?.ok}<br />{/if}
              {itemById('soll')?.hint}
            {/if}
          </p>
          <button type="button" disabled={foundationBusy} onclick={runFoundationFix}>
            {foundationBusy ? 'Repariere…' : 'Zeitbasis reparieren'}
          </button>
        {/if}
        <p class="muted" style="font-size: 0.85rem; margin-top: 0.75rem;">
          CLI: <code>npm run foundation:health</code> (API muss laufen)
        </p>
      {:else if steps[step].body === 'personnel'}
        <p>
          Jeder aktive Mitarbeiter braucht einen Arbeitskalender (Soll). Beim Anlegen „Arbeitskalender“
          anhaken oder Zuweisung unter Zeit → Arbeitskalender.
        </p>
        {#if itemById('calendar')?.ok}
          <p class="success">Alle aktiven MA haben einen Kalender.</p>
        {:else}
          <button
            type="button"
            onclick={() => {
              onNavigate?.({ tab: 'personnel', personnelNoCalendar: true });
              closeWizard();
            }}
          >
            Personal — ohne Kalender anzeigen
          </button>
        {/if}
      {:else if steps[step].body === 'timesheets'}
        <p>
          Entwürfe der aktuellen KW sollten Ist, Soll und Saldo haben (nach Stempeln: automatischer Rebuild;
          sonst „Neu berechnen“).
        </p>
        {#if itemById('soll')?.ok}
          <p class="success">Keine KW-Entwürfe ohne Soll.</p>
        {:else}
          <div class="btn-row">
            <button type="button" disabled={foundationBusy} onclick={runFoundationFix}>
              Zeitbasis reparieren
            </button>
            <button
              type="button"
              class="secondary"
              onclick={() => {
                onNavigate?.({ tab: 'time', timesheetFilter: 'draft', timeSection: 'stundenzettel' });
                closeWizard();
              }}
            >
              Stundenzettel (Entwürfe)
            </button>
          </div>
        {/if}
      {:else if steps[step].body === 'mismatch'}
        <p>
          Vergleich: eingestempelt vs. im Gebäude (letztes Zutrittsereignis). Abweichungen sind oft
          Homeoffice oder vergessene Stempel — bewusst klären, nicht zwingend Fehler.
        </p>
        {#if itemById('mismatch')?.ok}
          <p class="success">Keine offenen Abweichungen in der Übersicht.</p>
        {:else}
          <p>{itemById('mismatch')?.hint}</p>
          <button
            type="button"
            class="secondary"
            onclick={() => {
              closeWizard();
            }}
          >
            Schließen — Liste auf Übersicht prüfen
          </button>
        {/if}
      {:else}
        {#if allOk}
          <p class="success">
            Alle Checklisten-Punkte erfüllt. Nach fachlicher Freigabe (Stundenzettel, Freigaben) kann der
            Betrieb starten.
          </p>
        {:else}
          <p>
            Noch {openCount} Punkt(e) offen — Assistent jederzeit erneut öffnen oder Checkliste auf der
            Übersicht nutzen.
          </p>
        {/if}
        <p class="muted" style="font-size: 0.85rem;">
          Verify: <code>npm run smoke:production</code> · <code>npm run verify:foundation</code>
        </p>
      {/if}
    </div>

    <footer class="wizard-footer">
      <div class="btn-row">
        {#if step > 0}
          <button type="button" class="secondary" onclick={() => (step -= 1)}>Zurück</button>
        {/if}
        {#if step < lastStep}
          <button type="button" onclick={() => (step += 1)}>Weiter</button>
        {:else}
          <button type="button" onclick={() => closeWizard()}>Fertig</button>
        {/if}
      </div>
      <button type="button" class="secondary linkish" onclick={() => closeWizard(true)}>
        Nicht mehr automatisch anzeigen
      </button>
      <button type="button" class="secondary linkish" onclick={() => closeWizard()}>Schließen</button>
    </footer>
  </div>
</dialog>

<style>
  .wizard-dialog {
    border: none;
    border-radius: 8px;
    padding: 0;
    max-width: min(520px, 92vw);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.25);
  }
  .wizard-dialog::backdrop {
    background: rgba(0, 0, 0, 0.45);
  }
  .wizard-panel {
    padding: 1.25rem 1.5rem 1rem;
  }
  .wizard-body {
    margin: 1rem 0;
    font-size: 0.95rem;
    line-height: 1.45;
  }
  .wizard-footer {
    border-top: 1px solid var(--border, #ddd);
    padding-top: 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }
  .wizard-footer .linkish {
    font-size: 0.8rem;
    align-self: flex-start;
    padding: 0.2rem 0;
  }
  .checklist {
    list-style: none;
    padding: 0;
    margin: 0.75rem 0 0;
  }
  .checklist li {
    padding: 0.3rem 0;
    font-size: 0.9rem;
  }
  .checklist li.ok .check-icon {
    color: #2e7d32;
  }
  .checklist li.warn .check-icon {
    color: #b8860b;
  }
  .check-icon {
    display: inline-block;
    width: 1.25rem;
    font-weight: bold;
  }
  li.ok {
    color: inherit;
  }
</style>
