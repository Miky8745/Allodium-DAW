<script lang="ts">
  import type { Snippet } from 'svelte';

  // One label | control row inside a PrefSection. `hint` is a small muted
  // second line under the label.
  let { label, hint = '', children }: { label: string; hint?: string; children: Snippet } = $props();
</script>

<div class="pref-row">
  <div class="pref-label">
    <span>{label}</span>
    {#if hint}<span class="pref-hint">{hint}</span>{/if}
  </div>
  <div class="pref-control">{@render children()}</div>
</div>

<style>
  .pref-row {
    display: grid;
    grid-template-columns: minmax(140px, 1fr) auto;
    align-items: center;
    gap: 12px;
    min-height: 30px;
    padding: 3px 0;
  }
  .pref-row + :global(.pref-row) {
    border-top: 1px solid rgba(255, 255, 255, 0.04);
  }
  .pref-label {
    display: flex;
    flex-direction: column;
    gap: 1px;
    font-size: 12.5px;
    color: var(--main-text);
  }
  .pref-hint {
    font-size: 10.5px;
    color: var(--main-text-muted);
  }
  .pref-control {
    display: flex;
    align-items: center;
    gap: 6px;
    justify-content: flex-end;
  }
</style>
