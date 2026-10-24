<script lang="ts">
  import type { PatternData } from '$lib/types';
  import ScrollField from '../ScrollField.svelte';
  import { deleteOut } from '$lib/deleteFx';

  // One card in the Playlist's patterns panel. `active` = the pattern
  // being drawn/edited (patternStore.selectedPatternId). While
  // `renaming`, the name becomes an input: Enter or blur commits, Escape
  // cancels (an empty name is ignored).
  let {
    pattern,
    active,
    renaming,
    onSelect,
    onContextMenu,
    onStartRename,
    onEndRename,
  }: {
    pattern: PatternData;
    active: boolean;
    renaming: boolean;
    onSelect: () => void;
    onContextMenu: (e: MouseEvent) => void;
    onStartRename: () => void;
    onEndRename: () => void;
  } = $props();

  let draft = $state('');
  let inputEl = $state<HTMLInputElement | null>(null);
  // Removing a focused input from the DOM can still fire a trailing blur,
  // which would re-commit an edit Escape just discarded.
  let cancelled = false;

  $effect(() => {
    if (renaming && inputEl) {
      draft = pattern.name;
      cancelled = false;
      inputEl.focus();
      inputEl.select();
    }
  });

  function commit() {
    if (cancelled || !renaming) return;
    const name = draft.trim();
    if (name) pattern.name = name;
    onEndRename();
  }

  function onKeydown(e: KeyboardEvent) {
    e.stopPropagation(); // keep Delete/Space etc. out of the window-level shortcuts
    if (e.key === 'Enter') { e.preventDefault(); commit(); }
    else if (e.key === 'Escape') { e.preventDefault(); cancelled = true; onEndRename(); }
  }

  // The length field has its own click/drag/double-click meaning.
  const fromLengthField = (e: Event) => !!(e.target as HTMLElement).closest('.sf-wrap');
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  class="pattern-card"
  class:pattern-card--active={active}
  style="--pc: {pattern.color}"
  data-del-key="pattern:{pattern.id}"
  out:deleteOut
  onclick={(e) => { if (!fromLengthField(e) && !renaming) onSelect(); }}
  ondblclick={(e) => { if (!fromLengthField(e)) onStartRename(); }}
  oncontextmenu={(e) => { e.preventDefault(); if (!fromLengthField(e)) onContextMenu(e); }}
  role="button"
  tabindex="0"
>
  <span class="pattern-swatch"></span>
  {#if renaming}
    <input
      class="pattern-name-input"
      bind:this={inputEl}
      bind:value={draft}
      onkeydown={onKeydown}
      onblur={commit}
      onclick={(e) => e.stopPropagation()}
      ondblclick={(e) => e.stopPropagation()}
    />
  {:else}
    <span class="pattern-name">{pattern.name}</span>
  {/if}
  <ScrollField bind:value={pattern.lengthBeats} min={0.25} max={999} step={0.25} decimals={2} width={40} label="beats" tooltipKey="LENGTH" name="{pattern.name} length" />
</div>

<style>
  .pattern-card {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 6px 7px;
    background: var(--btn-bg);
    border: 1px solid var(--btn-border);
    border-radius: 7px;
    transition: background 0.1s, border-color 0.1s;
  }

  .pattern-card:hover { background: var(--btn-hover); }

  /* The pattern being drawn/edited. */
  .pattern-card--active {
    border-color: var(--pc);
    background: color-mix(in srgb, var(--pc) 14%, var(--btn-bg));
  }

  .pattern-swatch {
    width: 9px;
    height: 9px;
    border-radius: 4px;
    background: var(--pc);
    flex-shrink: 0;
  }

  .pattern-name {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    color: var(--main-text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    user-select: none;
  }

  .pattern-name-input {
    flex: 1;
    min-width: 0;
    padding: 1px 5px;
    font: inherit;
    font-size: 12px;
    color: var(--main-text);
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid var(--accent);
    border-radius: 4px;
    outline: none;
  }
</style>
