<script lang="ts">
  let {
    type,
    message,
    onDismiss,
  }: {
    type: 'error' | 'success';
    message: string;
    onDismiss?: () => void;
  } = $props();
</script>

{#if message}
  <div class="ts-flash {type}" role="alert">
    <span>{message}</span>
    {#if onDismiss}
      <button type="button" class="ts-flash-dismiss" onclick={onDismiss} aria-label="Schließen">
        ×
      </button>
    {/if}
  </div>
{/if}
