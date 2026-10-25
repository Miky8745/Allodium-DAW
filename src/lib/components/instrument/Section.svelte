<script lang="ts">
  import type { Snippet } from 'svelte';

  // Titled control group in the instrument window (Amp envelope, Filter, …).
  let { title, children }: { title: string; children: Snippet } = $props();
</script>

<section class="sec">
  <h3 class="sec-title">{title}</h3>
  <div class="sec-row">{@render children()}</div>
</section>

<style>
  .sec {
    padding: 8px 10px 10px;
    border: 1px solid var(--main-border);
    border-radius: 9px;
    background: rgba(255, 255, 255, 0.015);
  }
  .sec-title {
    margin: 0 0 8px;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--main-text-muted);
  }
  .sec-row {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
  }
</style>
