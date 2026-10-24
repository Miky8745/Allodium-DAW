<script lang="ts">
  // Right-side FX panel: shows the insert-slot chain for whichever strip is
  // currently selected (mixerStore.selectedChannelId) — a channel or
  // Master, both selectable, see "Mixer" in CLAUDE.md. No selector of its
  // own; it's purely a mirror of the Mixer's own click-to-select strips.
  import { mixerStore, MASTER_TRACK_ID, type MixerChannelData } from '$lib/mixerStore.svelte';
  import MixerInsertSlots from './MixerInsertSlots.svelte';

  let entity = $derived(mixerStore.selectedEntity);
  let isChannel = $derived(!!entity && entity.id !== MASTER_TRACK_ID);
  let label = $derived(
    !entity ? null : entity.id === MASTER_TRACK_ID ? 'MASTER' : (entity as MixerChannelData).name,
  );

  // Rename — mirrors MixerChannelStrip.svelte's own rename widget exactly
  // (same focus-on-open + Escape/blur-race guard) so both places edit the
  // same underlying MixerChannelData.name and always agree. Master's fixed
  // "MASTER" label stays unrenamable here too.
  let renaming = $state(false);
  let nameDraft = $state('');
  let nameInputEl = $state<HTMLInputElement | null>(null);

  $effect(() => {
    if (renaming) nameInputEl?.select();
  });

  function startRename() {
    if (!isChannel) return;
    nameDraft = (entity as MixerChannelData).name;
    renaming = true;
  }

  let renameCancelled = false;

  function commitRename() {
    if (renameCancelled) { renameCancelled = false; renaming = false; return; }
    const trimmed = nameDraft.trim();
    if (trimmed && isChannel) (entity as MixerChannelData).name = trimmed;
    renaming = false;
  }

  function renameKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') { e.preventDefault(); commitRename(); }
    else if (e.key === 'Escape') { e.preventDefault(); renameCancelled = true; renaming = false; }
  }

  // The selection can change while mid-rename (clicking a different strip
  // blurs the input first, which normally commits before the selection
  // changes — but this is cheap extra insurance against any future path
  // that could swap `entity` without a DOM blur ever firing).
  $effect(() => {
    entity;
    renaming = false;
  });
</script>

<div class="fx-rack">
  <div class="fx-rack-title">
    FX
    {#if label}
      {#if renaming}
        <input
          class="fx-name-input"
          bind:this={nameInputEl}
          bind:value={nameDraft}
          onblur={commitRename}
          onkeydown={renameKeydown}
          onclick={(e) => e.stopPropagation()}
        />
      {:else}
        <span
          class="fx-rack-title-channel"
          ondblclick={(e) => { e.stopPropagation(); startRename(); }}
          role="region"
          aria-label="Channel name"
          title={isChannel ? 'Double-click to rename' : undefined}
        > — {label}</span>
      {/if}
    {/if}
  </div>
  {#if entity}
    <MixerInsertSlots slots={entity.insertSlots} />
  {:else}
    <div class="fx-rack-empty">No channel selected</div>
  {/if}
</div>

<style>
  .fx-rack {
    width: 190px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    background: var(--explorer-bg, #22252e);
    min-height: 0;
  }

  .fx-rack-title {
    display: flex;
    align-items: center;
    min-width: 0;
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--main-text-muted, #606269);
    padding: 8px 10px;
    border-bottom: 1px solid var(--explorer-border, #383c4a);
    flex-shrink: 0;
  }

  .fx-rack-title-channel {
    color: var(--accent, #e6b07f);
    text-transform: none;
    letter-spacing: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .fx-name-input {
    flex: 1;
    min-width: 0;
    margin-left: 6px;
    font-family: 'DM Mono', 'Cascadia Code', monospace;
    font-size: 10px;
    letter-spacing: 0;
    text-transform: none;
    color: var(--main-text, #bcbcc0);
    background: #171a23;
    border: 1px solid var(--accent, #e6b07f);
    border-radius: 4px;
    padding: 1px 4px;
  }

  .fx-rack-empty {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 12px;
    font-size: 11px;
    color: var(--main-text-muted, #606269);
    font-style: italic;
  }
</style>
