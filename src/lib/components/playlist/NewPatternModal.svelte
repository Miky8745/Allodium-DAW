<script lang="ts">
  import { portal } from '$lib/portal';
  import { patternStore } from '$lib/patternStore.svelte';

  // Name prompt shown before a pattern is created (the patterns panel's
  // `+`). Seeded with — and pre-selected — the name that would be
  // auto-assigned, so Enter / OK right away just accepts it; an emptied
  // field also falls back to it. Escape, Cancel or the backdrop create
  // nothing.
  let { onClose }: { onClose: () => void } = $props();

  const defaultName = patternStore.nextPatternName;
  let name = $state(defaultName);
  let inputEl = $state<HTMLInputElement | null>(null);

  $effect(() => {
    inputEl?.focus();
    inputEl?.select();
  });

  function create() {
    patternStore.addPattern(name.trim() || defaultName);
    onClose();
  }

  function onKeydown(e: KeyboardEvent) {
    e.stopPropagation(); // keep Delete/Space etc. out of window-level shortcuts
    if (e.key === 'Enter') { e.preventDefault(); create(); }
    else if (e.key === 'Escape') { e.preventDefault(); onClose(); }
  }
</script>

<div class="np-backdrop" use:portal onmousedown={(e) => { if (e.target === e.currentTarget) onClose(); }} role="presentation">
  <div class="np-modal" role="dialog" aria-modal="true" aria-labelledby="np-title">
    <h2 id="np-title" class="np-title">New pattern</h2>
    <label class="np-label" for="np-name">Pattern name</label>
    <input
      id="np-name"
      class="np-input"
      type="text"
      autocomplete="off"
      bind:this={inputEl}
      bind:value={name}
      onkeydown={onKeydown}
    />
    <div class="np-actions">
      <button class="np-btn np-btn--secondary" onclick={onClose}>Cancel</button>
      <button class="np-btn np-btn--primary" onclick={create}>OK</button>
    </div>
  </div>
</div>

<style>
  .np-backdrop {
    position: fixed;
    inset: 0;
    z-index: 9000; /* above every floating window, same tier as Preferences */
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.45);
    backdrop-filter: blur(2px);
  }

  .np-modal {
    width: 320px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 20px;
    background: var(--main-bg);
    border: 1px solid var(--main-border);
    border-radius: 14px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.35);
  }

  .np-title {
    margin-bottom: 6px;
    font-family: 'Playfair Display', Georgia, serif;
    font-size: 15px;
    font-weight: 600;
    color: var(--main-text);
  }

  .np-label {
    font-size: 12px;
    font-weight: 500;
    letter-spacing: 0.03em;
    color: var(--main-text-muted);
  }

  .np-input {
    padding: 8px 12px;
    background: var(--search-bg);
    border: 1px solid var(--main-border);
    border-radius: 10px;
    font-family: inherit;
    font-size: 13.5px;
    color: var(--main-text);
    outline: none;
    transition: border-color 0.12s;
  }

  .np-input:focus { border-color: var(--accent); }

  .np-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 10px;
  }

  .np-btn {
    padding: 6px 14px;
    border-radius: 9px;
    border: 1px solid transparent;
    font-family: inherit;
    font-size: 13px;
    font-weight: 500;
    line-height: 1;
    cursor: pointer;
    transition: background 0.12s, border-color 0.12s;
  }

  .np-btn--primary { background: var(--accent); color: var(--accent-text); border-color: var(--accent); }
  .np-btn--primary:hover { background: var(--accent-hover); border-color: var(--accent-hover); }

  .np-btn--secondary { background: var(--btn-bg); color: var(--btn-text); border-color: var(--btn-border); }
  .np-btn--secondary:hover { background: var(--btn-hover); }
</style>
