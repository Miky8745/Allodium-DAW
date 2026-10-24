<script lang="ts" generics="T extends string | number">
  let {
    value = $bindable(),
    options,
  }: {
    value: T;
    options: { value: T; label: string }[];
  } = $props();
</script>

<div class="seg" role="radiogroup">
  {#each options as opt (opt.value)}
    <button
      class="seg-btn"
      class:seg-btn--on={opt.value === value}
      role="radio"
      aria-checked={opt.value === value}
      onclick={() => (value = opt.value)}
    >{opt.label}</button>
  {/each}
</div>

<style>
  .seg {
    display: flex;
    flex-wrap: wrap;
    background: #171a23;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    padding: 2px;
    gap: 2px;
  }
  .seg-btn {
    padding: 3px 7px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--main-text-muted);
    font-family: 'DM Mono', 'Cascadia Code', monospace;
    font-size: 11px;
    cursor: pointer;
  }
  .seg-btn:hover { color: var(--main-text); background: var(--btn-hover); }
  .seg-btn--on,
  .seg-btn--on:hover {
    background: var(--accent);
    color: var(--accent-text);
  }
</style>
