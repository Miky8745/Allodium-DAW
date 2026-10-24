<script lang="ts">
  import type { EditTool } from '$lib/editTools';

  // Draw / Delete / Drag-select toggle buttons, shared by the Piano Roll and
  // Playlist headers. `items` names what the tools act on, for the tooltips.
  let { tool = $bindable(), items }: { tool: EditTool; items: string } = $props();

  function pick(e: MouseEvent, t: EditTool) {
    e.stopPropagation(); // the header behind is the window's drag handle
    tool = t;
  }
</script>

<button
  class="tool-btn"
  class:tool-btn--on={tool === 'draw'}
  onclick={(e) => pick(e, 'draw')}
  aria-label="Draw tool"
  title="Draw (place, select and move {items})"
>
  <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round">
    <path d="M2 12l1-3 7-7 2 2-7 7-3 1z"/>
  </svg>
</button>
<button
  class="tool-btn"
  class:tool-btn--on={tool === 'delete'}
  onclick={(e) => pick(e, 'delete')}
  aria-label="Delete tool"
  title="Delete (click or drag over {items})"
>
  <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round">
    <path d="M5.5 12.5h7"/>
    <path d="M8.2 2.3l3.5 3.5-6.2 6.2H3.4L1.8 10.4z"/>
    <path d="M4.8 5.7l3.5 3.5"/>
  </svg>
</button>
<button
  class="tool-btn"
  class:tool-btn--on={tool === 'marquee'}
  onclick={(e) => pick(e, 'marquee')}
  aria-label="Drag select tool"
  title="Drag select (box)"
>
  <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-dasharray="2 1.6">
    <rect x="2" y="2" width="10" height="10" rx="1.5"/>
  </svg>
</button>

<style>
  /* Same look as the headers' .hdr-btn.hdr-tgl buttons. */
  .tool-btn {
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

  .tool-btn svg { width: 14px; height: 14px; }

  .tool-btn:hover {
    background: rgba(255,255,255,0.07);
    color: rgba(255,255,255,0.75);
  }

  .tool-btn--on {
    color: var(--accent, #e6b07f);
    border-color: rgba(230,176,127,0.3);
  }

  .tool-btn--on:hover {
    color: var(--accent, #e6b07f);
    background: rgba(230,176,127,0.1);
  }
</style>
