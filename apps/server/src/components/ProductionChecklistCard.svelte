<script lang="ts">
  import {
    computeProductionChecklist,
    checklistAllOk,
    type ProductionHealth,
    type ProductionDashboard,
  } from '../lib/productionChecklist';

  let {
    apiHealth = null,
    dashboard = null,
    onOpenWizard,
  }: {
    apiHealth?: ProductionHealth | null;
    dashboard?: ProductionDashboard | null;
    onOpenWizard?: () => void;
  } = $props();

  const items = $derived(computeProductionChecklist(apiHealth, dashboard));
  const allOk = $derived(checklistAllOk(items));
  const openCount = $derived(items.filter((i) => !i.ok).length);
</script>

<div class="card production-checklist" style="margin-top: 1rem;">
  <div class="btn-row" style="justify-content: space-between; align-items: flex-start; flex-wrap: wrap;">
    <h3 style="margin: 0;">Produktions-Checkliste</h3>
    {#if onOpenWizard}
      <button type="button" class="secondary" onclick={onOpenWizard}>
        Go-Live-Assistent
        {#if openCount > 0}
          <span class="nav-badge">{openCount}</span>
        {/if}
      </button>
    {/if}
  </div>
  <p class="muted" style="font-size: 0.9rem; margin-top: 0;">
    Kurzprüfung vor Go-Live — Details in <code>docs/PRODUCTION.md</code>.
  </p>
  <ul class="checklist">
    {#each items as item}
      <li class:ok={item.ok} class:warn={!item.ok}>
        <span class="check-icon" aria-hidden="true">{item.ok ? '✓' : '○'}</span>
        {item.label}
        {#if item.hint && !item.ok}
          <span class="muted"> — {item.hint}</span>
        {/if}
      </li>
    {/each}
  </ul>
  {#if allOk}
    <p class="success" style="margin-top: 0.75rem; font-size: 0.9rem;">
      Alle Punkte erfüllt — bereit für produktiven Betrieb (nach fachlicher Freigabe).
    </p>
  {:else}
    <p class="muted" style="margin-top: 0.75rem; font-size: 0.85rem;">
      Offene Punkte beheben oder bewusst dokumentieren (z. B. Homeoffice ohne Zutritt).
    </p>
  {/if}
</div>

<style>
  .checklist {
    list-style: none;
    padding: 0;
    margin: 0.75rem 0 0;
  }
  .checklist li {
    padding: 0.35rem 0;
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
</style>
