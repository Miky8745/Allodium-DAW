<script module lang="ts">
  // draw: place/select/move/resize notes · delete: click/drag erases notes ·
  // marquee: every drag is a selection box (see NoteGrid.svelte)
  export type { EditTool as PianoRollTool } from '$lib/editTools';
</script>

<script lang="ts">
  import type { EditTool as PianoRollTool } from '$lib/editTools';
  import EditToolButtons from '../EditToolButtons.svelte';

  let {
    title,
    tool = $bindable(),
    showNoteProps = $bindable(),
    maximized,
    isPlaying,
    onClose,
    onDragStart,
    onToggleMaximize,
    onTogglePlay,
  }: {
    title: string;
    tool: PianoRollTool;
    showNoteProps: boolean;
    maximized: boolean;
    isPlaying: boolean;
    onClose: () => void;
    onDragStart: (e: MouseEvent) => void;
    onToggleMaximize: () => void;
    onTogglePlay: () => void;
  } = $props();
</script>

<div class="pr-header" onmousedown={onDragStart} role="toolbar" aria-label="Piano Roll controls" tabindex="-1">
  <div class="hdr-left">
    <EditToolButtons bind:tool items="notes" />
    <button
      class="hdr-btn hdr-tgl"
      class:hdr-tgl--on={isPlaying}
      onclick={(e) => { e.stopPropagation(); onTogglePlay(); }}
      aria-label={isPlaying ? 'Stop' : 'Play'}
      title={isPlaying ? 'Stop' : 'Play'}
    >
      {#if isPlaying}
        <svg viewBox="0 0 14 14" fill="currentColor">
          <rect x="3" y="3" width="3" height="8" rx="0.5"/>
          <rect x="8" y="3" width="3" height="8" rx="0.5"/>
        </svg>
      {:else}
        <svg viewBox="0 0 14 14" fill="currentColor">
          <path d="M4 2.5l7 4.5-7 4.5z"/>
        </svg>
      {/if}
    </button>
    <button
      class="hdr-btn hdr-tgl"
      class:hdr-tgl--on={showNoteProps}
      onclick={(e) => { e.stopPropagation(); showNoteProps = !showNoteProps; }}
      aria-label="Note properties"
      title="Note properties"
    >
      <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round">
        <line x1="2" y1="4" x2="12" y2="4"/><circle cx="5" cy="4" r="1.4" fill="currentColor"/>
        <line x1="2" y1="10" x2="12" y2="10"/><circle cx="9" cy="10" r="1.4" fill="currentColor"/>
      </svg>
    </button>
  </div>

  <span class="pr-title">{title}</span>

  <div class="hdr-right">
    <button
      class="hdr-btn"
      onclick={(e) => { e.stopPropagation(); onToggleMaximize(); }}
      aria-label={maximized ? 'Restore' : 'Maximize'}
      title={maximized ? 'Restore' : 'Maximize'}
    >
      {#if maximized}
        <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round">
          <rect x="4" y="4" width="8" height="8" rx="1"/>
          <path d="M2 8V3a1 1 0 0 1 1-1h5"/>
        </svg>
      {:else}
        <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
          <rect x="2" y="2" width="10" height="10" rx="1"/>
        </svg>
      {/if}
    </button>
    <button class="close-btn" onclick={onClose} aria-label="Close">
      <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
        <line x1="3" y1="3" x2="11" y2="11"/>
        <line x1="11" y1="3" x2="3" y2="11"/>
      </svg>
    </button>
  </div>
</div>

<style>
  .pr-header {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 8px;
    height: 36px;
    background: #1d1f29;
    border-bottom: 1px solid var(--explorer-border, #383c4a);
    flex-shrink: 0;
    user-select: none;
  }

  .hdr-left,
  .hdr-right {
    display: flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
  }

  .pr-title {
    position: absolute;
    left: 50%;
    transform: translateX(-50%);
    font-family: 'Playfair Display', Georgia, serif;
    font-size: 13px;
    font-weight: 600;
    color: var(--main-text, #bcbcc0);
    pointer-events: none;
    white-space: nowrap;
    max-width: 50%;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .hdr-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 7px;
    color: rgba(255,255,255,0.45);
    cursor: pointer;
    padding: 0;
    transition: color 0.1s, background 0.1s, border-color 0.1s;
  }

  .hdr-btn svg { width: 14px; height: 14px; }

  .hdr-btn:hover {
    background: rgba(255,255,255,0.07);
    color: rgba(255,255,255,0.75);
  }

  .hdr-tgl--on {
    color: var(--accent, #e6b07f);
    border-color: rgba(230,176,127,0.3);
  }

  .hdr-tgl--on:hover {
    color: var(--accent, #e6b07f);
    background: rgba(230,176,127,0.1);
  }

  .close-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 7px;
    color: rgba(255,255,255,0.4);
    cursor: pointer;
    padding: 0;
    transition: color 0.1s, background 0.1s;
  }

  .close-btn svg { width: 12px; height: 12px; }

  .close-btn:hover {
    background: rgba(220,50,50,0.18);
    color: #f08484;
    border-color: rgba(220,50,50,0.3);
  }
</style>
