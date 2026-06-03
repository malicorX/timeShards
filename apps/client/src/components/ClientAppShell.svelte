<script lang="ts">
  import type { LoginResponse } from '../lib/api';
  import { formatMinutes } from '../lib/formatMinutes';

  type WorkSummary = {
    is_clocked_in: boolean;
    is_on_break: boolean;
    flex_balance_minutes?: number | null;
    work_calendar_assigned?: boolean | null;
    current_week?: {
      worked_minutes: number;
      expected_minutes: number;
      balance_minutes: number;
    } | null;
  };

  type Pillar = 'time' | 'absence' | 'approvals' | 'access' | 'account';

  let {
    user,
    workSummary = null,
    pillar = $bindable<Pillar>('time'),
    canApprove = false,
    approvalQueueCount = 0,
    ownDraftCount = 0,
    ownPendingAbsences = 0,
    onRefresh,
    onLogout,
    children,
  }: {
    user: LoginResponse['user'] | null;
    workSummary?: WorkSummary | null;
    pillar?: Pillar;
    canApprove?: boolean;
    approvalQueueCount?: number;
    ownDraftCount?: number;
    ownPendingAbsences?: number;
    onRefresh?: () => void;
    onLogout?: () => void;
    children?: import('svelte').Snippet;
  } = $props();
</script>

<div class="layout">
  <aside class="sidebar">
    <h1>TimeShards</h1>
    <p class="muted" style="font-size: 0.8rem;">
      {user?.display_name}
      {#if user?.employee_no}
        <br />PN {user.employee_no}
      {/if}
    </p>
    {#if workSummary}
      <p class="muted" style="font-size: 0.75rem; margin-top: 0.35rem;">
        {#if workSummary.is_on_break}
          Pause
        {:else if workSummary.is_clocked_in}
          Eingestempelt
        {:else}
          Ausgestempelt
        {/if}
        {#if workSummary.flex_balance_minutes != null}
          <br />Gleitzeit: {formatMinutes(workSummary.flex_balance_minutes)}
        {/if}
        {#if workSummary.work_calendar_assigned === false}
          <br /><span class="error">Kein Arbeitskalender</span>
        {:else if workSummary.current_week && workSummary.current_week.expected_minutes > 0}
          <br />KW: Ist {formatMinutes(workSummary.current_week.worked_minutes)} · Soll{' '}
          {formatMinutes(workSummary.current_week.expected_minutes)} · Saldo{' '}
          {formatMinutes(workSummary.current_week.balance_minutes)}
        {/if}
      </p>
    {/if}
    <nav class="nav">
      {#if canApprove}
        <button type="button" class:active={pillar === 'approvals'} onclick={() => (pillar = 'approvals')}>
          Freigaben
          {#if approvalQueueCount > 0}
            <span class="nav-badge">{approvalQueueCount}</span>
          {/if}
        </button>
      {/if}
      <button type="button" class:active={pillar === 'time'} onclick={() => (pillar = 'time')}>
        Zeit
        {#if ownDraftCount > 0}
          <span class="nav-badge draft">{ownDraftCount}</span>
        {/if}
      </button>
      <button type="button" class:active={pillar === 'absence'} onclick={() => (pillar = 'absence')}>
        Abwesenheit
        {#if ownPendingAbsences > 0}
          <span class="nav-badge">{ownPendingAbsences}</span>
        {/if}
      </button>
      <button type="button" class:active={pillar === 'access'} onclick={() => (pillar = 'access')}>
        Zutritt
      </button>
      <button type="button" class:active={pillar === 'account'} onclick={() => (pillar = 'account')}>
        Konto
      </button>
    </nav>
    <button type="button" class="secondary" style="margin-top: 1rem;" onclick={() => onRefresh?.()}>
      Aktualisieren
    </button>
    <button type="button" class="secondary" style="margin-top: 0.5rem;" onclick={() => onLogout?.()}>
      Abmelden
    </button>
  </aside>

  <main class="content">
    {@render children?.()}
  </main>
</div>
