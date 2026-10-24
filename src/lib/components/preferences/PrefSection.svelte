<script lang="ts">
  import type { Snippet } from 'svelte';

  let { title, children }: { title: string; children: Snippet } = $props();
</script>

<section class="pref-section">
  <h3 class="pref-section-title">{title}</h3>
  <div class="pref-section-body">{@render children()}</div>
</section>

<style>
  .pref-section {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .pref-section-title {
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--accent);
    margin: 0 0 2px;
  }
  .pref-section-body {
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: #1b1e27;
    border: 1px solid var(--main-border);
    border-radius: 9px;
    padding: 6px 10px;
  }
</style>
