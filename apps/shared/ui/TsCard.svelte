<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    title,
    lead,
    children,
    actions,
    class: className = '',
  }: {
    title?: string;
    lead?: string;
    children?: Snippet;
    actions?: Snippet;
    class?: string;
  } = $props();
</script>

<section class="ts-card {className}">
  {#if title || lead || actions}
    <header class="ts-card-header">
      <div>
        {#if title}<h3>{title}</h3>{/if}
        {#if lead}<p class="ts-lead" style="margin-top: 0.35rem; font-size: 0.85rem;">{lead}</p>{/if}
      </div>
      {#if actions}
        <div class="ts-card-actions">{@render actions()}</div>
      {/if}
    </header>
  {/if}
  {@render children?.()}
</section>
