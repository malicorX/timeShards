<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    label,
    hint,
    children,
    class: className = '',
  }: {
    label: string;
    hint?: string;
    children?: Snippet;
    class?: string;
  } = $props();
</script>

<label class="ts-field {className}">
  <span class="ts-field-label">{label}</span>
  {@render children?.()}
  {#if hint}<span class="ts-field-label" style="font-weight: normal;">{hint}</span>{/if}
</label>
