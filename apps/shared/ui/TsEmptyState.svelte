<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    message,
    children,
  }: {
    message: string;
    children?: Snippet;
  } = $props();
</script>

<div class="ts-empty">
  <p>{message}</p>
  {#if children}{@render children()}{/if}
</div>
