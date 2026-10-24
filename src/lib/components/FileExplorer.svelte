<script lang="ts">
  import type { FileNode, FlatNode, FileDropDetail, FileDragOverDetail } from '$lib/types';
  import { beginDragCursor, endDragCursor } from '$lib/dragCursor';
  import ExplorerResizeHandle from './ExplorerResizeHandle.svelte';

  let {
    fileTree,
    show = $bindable(true),
  }: { fileTree: FileNode[]; show?: boolean } = $props();

  let expandedIds = $state(new Set<string>());
  let explorerExpanded = $state(true);
  let width = $state(220); // user-resizable via ExplorerResizeHandle; session-only

  interface DragTarget { name: string; path: string; isDir: boolean; files: string[] }
  let dragging = $state<(DragTarget & { x: number; y: number }) | null>(null);
  let pendingDrag = $state<(DragTarget & { startX: number; startY: number }) | null>(null);

  let flatNodes = $derived(buildFlat(fileTree, 0, ''));

  function buildFlat(nodes: FileNode[], depth: number, parentId: string): FlatNode[] {
    const result: FlatNode[] = [];
    for (const node of nodes) {
      const id = `${parentId}/${node.name}`;
      const isExpanded = expandedIds.has(id);
      result.push({ node, depth, expanded: isExpanded, id });
      if (node.is_dir && isExpanded) {
        result.push(...buildFlat(node.children, depth + 1, id));
      }
    }
    return result;
  }

  function toggleDir(id: string) {
    const next = new Set(expandedIds);
    if (next.has(id)) next.delete(id); else next.add(id);
    expandedIds = next;
  }

  // Directory nodes are draggable (to build a multisample instrument from
  // their contents) as well as clickable (to expand/collapse) — the 4px
  // move threshold below is what lets both coexist on the same element,
  // same as Playlist.svelte's pattern cards.
  function onNodePointerDown(e: PointerEvent, node: FileNode, path: string) {
    e.preventDefault();
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    const files = node.is_dir
      ? node.children.filter(c => !c.is_dir).map(c => `${path}/${c.name}`)
      : [];
    pendingDrag = { name: node.name, path, isDir: node.is_dir, files, startX: e.clientX, startY: e.clientY };
  }

  $effect(() => {
    let hoveredDrop: Element | null = null;

    function findDrop(x: number, y: number): Element | null {
      return document.elementsFromPoint(x, y).find(el => el.hasAttribute('data-sample-drop')) ?? null;
    }

    function onMove(e: PointerEvent) {
      if (pendingDrag) {
        const dx = e.clientX - pendingDrag.startX;
        const dy = e.clientY - pendingDrag.startY;
        if (Math.abs(dx) > 4 || Math.abs(dy) > 4) {
          dragging = { ...pendingDrag, x: e.clientX, y: e.clientY };
          pendingDrag = null;
          beginDragCursor('move');
        }
      } else if (dragging) {
        dragging = { ...dragging, x: e.clientX, y: e.clientY };
        const target = findDrop(e.clientX, e.clientY);
        if (target !== hoveredDrop) {
          if (hoveredDrop) hoveredDrop.dispatchEvent(new CustomEvent('filedragleave'));
          if (target) target.dispatchEvent(new CustomEvent('filedragenter'));
          hoveredDrop = target;
        }
        // Fired every tick (not just on target change) so a drop zone can
        // track live cursor position — e.g. ChannelRack's insertion-line
        // indicator — without a second document-wide pointermove listener.
        if (target) {
          const detail: FileDragOverDetail = { x: e.clientX, y: e.clientY };
          target.dispatchEvent(new CustomEvent('filedragover', { detail }));
        }
      }
    }

    function onUp(e: PointerEvent) {
      if (dragging && hoveredDrop) {
        const detail: FileDropDetail = dragging.isDir
          ? { kind: 'folder', path: dragging.path, files: dragging.files, x: dragging.x, y: dragging.y }
          : { kind: 'file', path: dragging.path, x: dragging.x, y: dragging.y };
        hoveredDrop.dispatchEvent(new CustomEvent('filedrop', { detail }));
      }
      if (hoveredDrop) {
        hoveredDrop.dispatchEvent(new CustomEvent('filedragleave'));
        hoveredDrop = null;
      }
      if (dragging) endDragCursor();
      dragging = null;
      pendingDrag = null;
    }

    document.addEventListener('pointermove', onMove);
    document.addEventListener('pointerup', onUp);
    return () => {
      document.removeEventListener('pointermove', onMove);
      document.removeEventListener('pointerup', onUp);
    };
  });

  $effect(() => {
    if (dragging) {
      document.body.style.cursor = 'grabbing';
    } else {
      document.body.style.cursor = '';
    }
  });
</script>

<div class="fe-mount" class:fe-hidden={!show}>

<!-- Explorer panel -->
<div class="explorer" style="width: {width}px;">
  <ExplorerResizeHandle bind:width />
  <div class="explorer-head">Explorer</div>

  <div class="explorer-section">
    <button
      class="section-title"
      onclick={() => explorerExpanded = !explorerExpanded}
      aria-expanded={explorerExpanded}
    >
      <svg
        class="section-chevron"
        class:section-chevron--open={explorerExpanded}
        width="10" height="10" viewBox="0 0 10 10" fill="none"
        stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"
        aria-hidden="true"
      >
        <path d="M2.5 3.5l2.5 3 2.5-3"/>
      </svg>
      <span>DATA</span>
    </button>

    {#if explorerExpanded}
      <div class="file-tree" role="tree">
        {#if flatNodes.length === 0}
          <p class="tree-empty">No files yet</p>
        {:else}
          {#each flatNodes as flat (flat.id)}
            {#if flat.node.is_dir}
              <button
                class="tree-item tree-item--dir"
                style="padding-left: {10 + flat.depth * 16}px"
                onclick={() => toggleDir(flat.id)}
                onpointerdown={(e) => onNodePointerDown(e, flat.node, flat.id.slice(1))}
                role="treeitem"
                aria-selected={false}
                aria-expanded={flat.expanded}
              >
                <svg
                  class="item-chevron"
                  class:item-chevron--open={flat.expanded}
                  width="9" height="9" viewBox="0 0 9 9" fill="none"
                  stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"
                  aria-hidden="true"
                >
                  <path d="M2 3l2.5 2.5L7 3"/>
                </svg>
                <svg class="item-icon item-icon--dir" width="15" height="15" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                  {#if flat.expanded}
                    <path d="M1 4.5h6l1.5 1.5H14v6H1V4.5z"/>
                    <path d="M1 7.5h13"/>
                  {:else}
                    <path d="M1 4.5h5l1.5 1.5H14v7H1V4.5z"/>
                  {/if}
                </svg>
                <span class="item-name">{flat.node.name}</span>
              </button>
            {:else}
              <div
                class="tree-item tree-item--file"
                style="padding-left: {10 + flat.depth * 16 + 13}px"
                role="treeitem"
                aria-selected={false}
                tabindex="0"
                onpointerdown={(e) => onNodePointerDown(e, flat.node, flat.id.slice(1))}
              >
                <svg class="item-icon item-icon--file" width="13" height="13" viewBox="0 0 13 13" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                  <path d="M2 1.5h6l3 3v7.5H2V1.5z"/>
                  <path d="M8 1.5V4.5H11"/>
                </svg>
                <span class="item-name">{flat.node.name}</span>
              </div>
            {/if}
          {/each}
        {/if}
      </div>
    {/if}
  </div>
</div>

<!-- Drag ghost -->
{#if dragging}
  <div
    class="drag-ghost"
    style="left:{dragging.x + 12}px; top:{dragging.y - 10}px"
    aria-hidden="true"
  >
    <svg class="ghost-icon" width="13" height="13" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round">
      {#if dragging.isDir}
        <path d="M1 4.5h5l1.5 1.5H14v7H1V4.5z"/>
      {:else}
        <path d="M2 1.5h6l3 3v7.5H2V1.5z"/>
        <path d="M8 1.5V4.5H11"/>
      {/if}
    </svg>
    <span>{dragging.name}</span>
  </div>
{/if}

</div><!-- /fe-mount -->

<style>
  .fe-mount   { display: contents; }
  .fe-hidden  { display: none; }

  .explorer {
    position: relative;
    flex-shrink: 0;
    background: var(--explorer-bg);
    border-right: 1px solid var(--explorer-border);
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
  }

  .explorer-head {
    padding: 8px 12px 4px;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--main-muted);
    flex-shrink: 0;
  }

  .explorer-section { display: flex; flex-direction: column; flex: 1; overflow: hidden; }

  .section-title {
    display: flex;
    align-items: center;
    gap: 5px;
    width: 100%;
    padding: 5px 8px;
    background: transparent;
    border: none;
    font-family: inherit;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.07em;
    color: var(--sidebar-text);
    cursor: pointer;
    text-align: left;
    flex-shrink: 0;
    transition: color 0.1s;
  }

  .section-title:hover { color: #c9cacd; }

  .section-chevron { flex-shrink: 0; transition: transform 0.15s; transform: rotate(-90deg); }
  .section-chevron--open { transform: rotate(0deg); }

  .file-tree { flex: 1; overflow-y: auto; padding-bottom: 8px; }
  .file-tree::-webkit-scrollbar { width: 5px; }
  .file-tree::-webkit-scrollbar-thumb { background: var(--explorer-border); border-radius: 5px; }

  .tree-empty { padding: 6px 16px; font-size: 12px; color: var(--main-muted); font-style: italic; }

  .tree-item {
    display: flex;
    align-items: center;
    gap: 4px;
    width: 100%;
    height: 22px;
    padding-right: 8px;
    border: none;
    background: transparent;
    font-family: inherit;
    font-size: 13px;
    color: var(--sidebar-text);
    cursor: pointer;
    text-align: left;
    transition: background 0.08s, color 0.08s;
    white-space: nowrap;
    overflow: hidden;
  }

  .tree-item:hover { background: rgba(255,255,255,0.05); color: #c9cacd; }

  .item-chevron { flex-shrink: 0; transition: transform 0.15s; transform: rotate(-90deg); color: var(--main-muted); }
  .item-chevron--open { transform: rotate(0deg); }

  .item-icon { flex-shrink: 0; }
  .item-icon--dir { color: var(--accent); opacity: 0.85; }
  .item-icon--file { color: var(--sidebar-text); opacity: 0.7; }

  .item-name { overflow: hidden; text-overflow: ellipsis; }

  .drag-ghost {
    position: fixed;
    pointer-events: none;
    z-index: 1000;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px 4px 7px;
    background: #282b33;
    border: 1px solid var(--accent);
    border-radius: 8px;
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.6);
    font-family: inherit;
    font-size: 12.5px;
    color: #bcbcc0;
    white-space: nowrap;
    max-width: 240px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ghost-icon { flex-shrink: 0; color: var(--accent); opacity: 0.9; }
</style>
