<script lang="ts">
  // Blender-style Preferences (Edit > Preferences…, Sidebar > Settings,
  // Ctrl+,): a modal with a tab list on the left. Every change applies and
  // saves immediately (preferencesStore debounce-saves to
  // data/preferences.xml) — there's no OK/Cancel.
  import { preferencesUi, type PreferencesTab } from '$lib/preferencesUi.svelte';
  import GeneralTab from './GeneralTab.svelte';
  import MidiTab from './MidiTab.svelte';

  const TABS: { id: PreferencesTab; label: string }[] = [
    { id: 'general', label: 'General' },
    { id: 'midi', label: 'MIDI' },
  ];

  function onKeydown(e: KeyboardEvent) {
    if (preferencesUi.open && e.key === 'Escape') {
      e.preventDefault();
      preferencesUi.close();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if preferencesUi.open}
  <div class="backdrop" role="presentation" onmousedown={(e) => { if (e.target === e.currentTarget) preferencesUi.close(); }}>
    <div class="modal" role="dialog" aria-modal="true" aria-label="Preferences">
      <nav class="tabs">
        <div class="title">Preferences</div>
        {#each TABS as t (t.id)}
          <button class="tab-btn" class:tab-btn--on={preferencesUi.tab === t.id} onclick={() => (preferencesUi.tab = t.id)}>
            {t.label}
          </button>
        {/each}
      </nav>
      <div class="content">
        <button class="close" onclick={() => preferencesUi.close()} title="Close (Esc)" aria-label="Close">
          <svg width="10" height="10" viewBox="0 0 10 10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" aria-hidden="true">
            <path d="M1 1l8 8M9 1l-8 8" />
          </svg>
        </button>
        {#if preferencesUi.tab === 'general'}
          <GeneralTab />
        {:else}
          <MidiTab />
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9000; /* above every floating window, below DragTooltip (10000) */
    backdrop-filter: blur(2px);
    font-family: 'DM Sans', 'Segoe UI', system-ui, sans-serif;
  }
  .modal {
    display: flex;
    width: min(760px, calc(100vw - 32px));
    height: min(620px, calc(100vh - 32px));
    background: var(--main-bg);
    border: 1px solid var(--main-border);
    border-radius: 14px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.35);
    overflow: hidden;
  }
  .tabs {
    width: 150px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 16px 10px;
    background: var(--sidebar-bg);
    border-right: 1px solid var(--sidebar-border);
  }
  .title {
    font-family: 'Playfair Display', Georgia, serif;
    font-size: 15px;
    font-weight: 600;
    color: var(--main-text);
    padding: 0 8px 12px;
  }
  .tab-btn {
    text-align: left;
    padding: 7px 10px;
    border-radius: 7px;
    border: none;
    background: transparent;
    color: var(--sidebar-text);
    font-family: inherit;
    font-size: 13px;
    cursor: pointer;
  }
  .tab-btn:hover { background: var(--sidebar-hover); }
  .tab-btn--on,
  .tab-btn--on:hover {
    background: var(--sidebar-active-bg);
    color: var(--sidebar-active-text);
  }
  .content {
    position: relative;
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding: 20px 22px;
  }
  .close {
    position: absolute;
    top: 12px;
    right: 12px;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 6px;
    border: none;
    background: transparent;
    color: var(--main-text-muted);
    cursor: pointer;
    z-index: 1;
  }
  .close:hover { background: var(--btn-hover); color: var(--main-text); }
</style>
