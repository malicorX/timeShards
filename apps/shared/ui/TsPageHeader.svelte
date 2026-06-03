<script lang="ts">
  let { title, lead }: { title: string; lead?: string } = $props();
</script>

<header class="ts-page-header">
  <h2>{title}</h2>
  {#if lead}<p class="ts-lead">{lead}</p>{/if}
</header>
