<script lang="ts">
  let { checked = $bindable(), label = '' }: { checked: boolean; label?: string } = $props();
</script>

<button
  class="pref-toggle"
  class:pref-toggle--on={checked}
  role="switch"
  aria-checked={checked}
  aria-label={label}
  onclick={() => (checked = !checked)}
>
  <span class="knob"></span>
</button>

<style>
  .pref-toggle {
    position: relative;
    width: 32px;
    height: 18px;
    padding: 0;
    border-radius: 9px;
    border: 1px solid var(--btn-border);
    background: var(--btn-bg);
    cursor: pointer;
    transition: background 0.15s, border-color 0.15s;
    flex-shrink: 0;
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--main-text-muted);
    transition: transform 0.15s, background 0.15s;
  }
  .pref-toggle--on {
    background: rgba(230, 176, 127, 0.22);
    border-color: var(--accent);
  }
  .pref-toggle--on .knob {
    transform: translateX(14px);
    background: var(--accent);
  }
</style>
