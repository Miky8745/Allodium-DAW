<script lang="ts">
  import ColorWheel from '../ColorWheel.svelte';
  import { portal } from '$lib/portal';
  import { hoverInfoAction } from '$lib/hoverInfo';
  import { noteColorStore, DEFAULT_NOTE_COLORS } from '$lib/noteColorStore.svelte';

  // 5×3 note color swatches. Left-click picks a color (onPick); right-click
  // or double-click opens a ColorWheel popover to change that swatch itself
  // — same ColorWheel the Channel Rack's Recolor uses, writing live into
  // noteColorStore.palette (so every note using that slot recolors live).
  let { current, onPick }: { current: number; onPick: (index: number) => void } = $props();

  let editing = $state<{ index: number; x: number; y: number } | null>(null);

  function openEditor(e: MouseEvent, index: number) {
    e.preventDefault();
    onPick(index);
    // Keep the ~200px-wide popover on screen.
    const x = Math.min(e.clientX, window.innerWidth - 210);
    const y = Math.min(e.clientY, window.innerHeight - 270);
    editing = { index, x, y };
  }

  function close() { editing = null; }

  function resetSwatch() {
    if (!editing) return;
    noteColorStore.palette[editing.index] = DEFAULT_NOTE_COLORS[editing.index];
    close(); // ColorWheel only seeds from its value on mount
  }
</script>

<div class="palette">
  {#each noteColorStore.palette as color, i (i)}
    <button
      class="swatch"
      class:swatch--on={i === current}
      style="background:{color};"
      onclick={() => onPick(i)}
      ondblclick={(e) => openEditor(e, i)}
      oncontextmenu={(e) => openEditor(e, i)}
      use:hoverInfoAction={{ name: `Note color ${i + 1}`, value: 'Right-click to edit' }}
      aria-label="Note color {i + 1}"
      aria-pressed={i === current}
    ></button>
  {/each}
</div>

{#if editing}
  <div class="pop-root" use:portal>
    <div class="pop-backdrop" onmousedown={close} oncontextmenu={(e) => e.preventDefault()} role="presentation"></div>
    <div class="pop" style="left:{editing.x}px; top:{editing.y}px;">
      <span class="pop-title">Note color {editing.index + 1}</span>
      {#key editing.index}
        <ColorWheel bind:value={noteColorStore.palette[editing.index]} />
      {/key}
      <div class="pop-actions">
        <button class="pop-btn" onclick={resetSwatch}>Reset</button>
        <button class="pop-btn pop-btn--done" onclick={close}>Done</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .palette {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 4px;
  }

  .swatch {
    aspect-ratio: 1;
    padding: 0;
    border: 1px solid rgba(0,0,0,0.45);
    border-radius: 5px;
    cursor: pointer;
    transition: transform 0.08s, box-shadow 0.08s;
  }

  .swatch:hover { transform: scale(1.08); }

  .swatch--on {
    box-shadow: 0 0 0 2px #1d1f29, 0 0 0 3.5px #e4e5e6;
  }

  .pop-root {
    position: fixed;
    inset: 0;
    z-index: 10000;
  }

  .pop-backdrop {
    position: absolute;
    inset: 0;
  }

  .pop {
    position: fixed;
    width: 196px;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    background: #272a33;
    border: 1px solid var(--explorer-border, #34373f);
    border-radius: 9px;
    box-shadow: 0 6px 18px rgba(0,0,0,0.5);
  }

  .pop-title {
    font-size: 11px;
    color: rgba(255,255,255,0.6);
  }

  .pop-actions {
    display: flex;
    gap: 4px;
  }

  .pop-btn {
    flex: 1;
    padding: 5px 0;
    font-size: 12px;
    background: transparent;
    border: none;
    border-radius: 5px;
    color: rgba(255,255,255,0.7);
    cursor: pointer;
  }

  .pop-btn:hover { background: rgba(255,255,255,0.07); }
  .pop-btn--done { color: var(--accent); }
</style>
