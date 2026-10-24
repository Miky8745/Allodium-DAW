<script lang="ts">
  import PatternCard from './PatternCard.svelte';
  import PatternContextMenu from './PatternContextMenu.svelte';
  import NewPatternModal from './NewPatternModal.svelte';
  import { patternStore } from '$lib/patternStore.svelte';
  import { deletePatterns } from '$lib/patternActions';
  import type { PatternData } from '$lib/types';

  // The Playlist's patterns panel. Clicking a card makes it the active
  // pattern (what gets drawn/edited). Double-click renames inline;
  // right-click opens the rename/recolor/delete menu — the only way to
  // delete a pattern (no Delete key here: that removes *placed* blocks
  // selected on the timeline, see playlistEditor.svelte.ts).
  let renamingId = $state<number | null>(null);
  let naming = $state(false); // `+` asks for a name first (NewPatternModal)
  let menu = $state<{ pattern: PatternData; x: number; y: number } | null>(null);

  function deleteFromMenu() {
    if (!menu) return;
    deletePatterns([menu.pattern.id]);
    menu = null;
  }

  function renameFromMenu() {
    if (!menu) return;
    renamingId = menu.pattern.id;
    menu = null;
  }
</script>

<div class="patterns-panel">
  <div class="patterns-head">Patterns</div>
  <div class="patterns-list">
    {#each patternStore.patterns as pattern (pattern.id)}
      <PatternCard
        {pattern}
        active={pattern.id === patternStore.selectedPatternId}
        renaming={renamingId === pattern.id}
        onSelect={() => (patternStore.selectedPatternId = pattern.id)}
        onContextMenu={(e) => (menu = { pattern, x: e.clientX, y: e.clientY })}
        onStartRename={() => (renamingId = pattern.id)}
        onEndRename={() => { if (renamingId === pattern.id) renamingId = null; }}
      />
    {/each}
  </div>
  <div class="patterns-footer">
    <button class="add-btn" onclick={() => (naming = true)} aria-label="Add pattern" title="Add pattern">+</button>
  </div>
</div>

{#if naming}
  <NewPatternModal onClose={() => (naming = false)} />
{/if}

{#if menu}
  <PatternContextMenu
    pattern={menu.pattern}
    x={menu.x}
    y={menu.y}
    onRename={renameFromMenu}
    onDelete={deleteFromMenu}
    onClose={() => (menu = null)}
  />
{/if}

<style>
  .patterns-panel {
    width: var(--patterns-w);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: #1d1f29;
    border-right: 1px solid var(--main-border);
  }

  .patterns-head {
    flex-shrink: 0;
    padding: 7px 10px 6px;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--main-text-muted);
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }

  .patterns-list {
    position: relative; /* offsetParent for the deletion ring */
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .patterns-list::-webkit-scrollbar { width: 6px; }
  .patterns-list::-webkit-scrollbar-thumb { background: var(--main-border); border-radius: 5px; }

  .patterns-footer {
    flex-shrink: 0;
    display: flex;
    justify-content: center;
    padding: 8px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
  }

  .add-btn {
    width: 22px;
    height: 22px;
    border-radius: 7px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    background: transparent;
    color: rgba(255, 255, 255, 0.6);
    font-size: 16px;
    line-height: 1;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    transition: background 0.1s, color 0.1s, border-color 0.1s;
  }

  .add-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    color: var(--main-text);
    border-color: rgba(255, 255, 255, 0.3);
  }
</style>
