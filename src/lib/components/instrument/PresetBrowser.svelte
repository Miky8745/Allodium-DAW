<script lang="ts">
  import { onMount } from 'svelte';
  import { forgetPresets, listPresets } from '$lib/instrument/presetLoader';
  import { presetNameFromPath } from '$lib/instrument/sfz';
  import { packStore } from '$lib/instrument/packs/packStore.svelte';

  // Every .sfz under data/instruments/, grouped by its first folder (the
  // category; installed pack instruments as "<Pack> · <category>").
  // Clicking one loads it onto the channel; "Get more…" opens the pack view.
  let { currentPath, onpick, onGetMore }: { currentPath: string; onpick: (path: string) => void; onGetMore: () => void } = $props();

  let paths = $state<string[]>([]);
  let query = $state('');
  let error = $state<string | null>(null);

  async function refresh() {
    forgetPresets();
    try {
      paths = await listPresets();
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    void packStore.loadCatalog(); // pack category names
    void refresh();
  });

  // Rescan after a pack install/removal.
  $effect(() => {
    if (packStore.revision > 0) void refresh();
  });

  let groups = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const map = new Map<string, string[]>();
    for (const p of paths) {
      if (q && !p.toLowerCase().includes(q)) continue;
      const parts = p.split('/');
      const category = packStore.categoryOf(p) ?? (parts.length > 2 ? parts[1] : 'Uncategorized');
      if (!map.has(category)) map.set(category, []);
      map.get(category)!.push(p);
    }
    // Bundled categories first, then installed packs.
    const isPack = (c: string) => c.includes(' · ');
    return [...map.entries()].sort((a, b) => Number(isPack(a[0])) - Number(isPack(b[0])) || a[0].localeCompare(b[0]));
  });
</script>

<div class="pb">
  <div class="pb-top">
    <input class="pb-search" type="text" placeholder="Search presets" bind:value={query} />
    <button class="pb-refresh" onclick={refresh} title="Rescan presets" aria-label="Rescan presets">
      <svg width="12" height="12" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M12 7a5 5 0 1 1-1.5-3.6"/>
        <path d="M12 2v3h-3"/>
      </svg>
    </button>
  </div>
  <div class="pb-list">
    {#if error}
      <p class="pb-note">Couldn't list presets: {error}</p>
    {:else if paths.length === 0}
      <p class="pb-note">No presets yet — put .sfz files in <code>data/instruments/&lt;Category&gt;/</code>.</p>
    {/if}
    {#each groups as [category, items] (category)}
      <div class="pb-cat">{category}</div>
      {#each items as p (p)}
        <button class="pb-item" class:pb-item--on={p === currentPath} onclick={() => onpick(p)} title={p}>
          {presetNameFromPath(p)}
        </button>
      {/each}
    {/each}
  </div>
  <button class="pb-more" onclick={onGetMore}>Get more instruments…</button>
</div>

<style>
  .pb {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
    border-right: 1px solid var(--main-border);
    background: var(--explorer-bg);
  }
  .pb-top {
    display: flex;
    gap: 4px;
    padding: 6px;
    border-bottom: 1px solid var(--main-border);
  }
  .pb-search {
    flex: 1;
    min-width: 0;
    height: 22px;
    padding: 0 7px;
    font-size: 11px;
    border-radius: 5px;
    border: 1px solid var(--btn-border);
    background: var(--search-bg);
    color: var(--main-text);
  }
  .pb-refresh {
    width: 22px;
    height: 22px;
    display: grid;
    place-items: center;
    border-radius: 5px;
    border: 1px solid var(--btn-border);
    background: var(--btn-bg);
    color: var(--btn-text);
    cursor: pointer;
  }
  .pb-refresh:hover { background: var(--btn-hover); }
  .pb-list {
    flex: 1;
    overflow-y: auto;
    padding: 4px 0 8px;
  }
  .pb-cat {
    padding: 8px 10px 3px;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--main-text-muted);
  }
  .pb-item {
    display: block;
    width: calc(100% - 8px);
    margin: 0 4px;
    padding: 4px 8px;
    text-align: left;
    font-size: 12px;
    border: none;
    border-radius: 5px;
    background: transparent;
    color: var(--main-text);
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pb-item:hover { background: var(--btn-hover); }
  .pb-item--on,
  .pb-item--on:hover {
    background: var(--accent);
    color: var(--accent-text);
  }
  .pb-more {
    margin: 6px;
    height: 26px;
    font-size: 12px;
    border-radius: 6px;
    border: 1px solid var(--accent);
    background: transparent;
    color: var(--accent);
    cursor: pointer;
  }
  .pb-more:hover {
    background: var(--accent);
    color: var(--accent-text);
  }
  .pb-note {
    margin: 8px 10px;
    font-size: 11px;
    color: var(--main-text-muted);
    line-height: 1.4;
  }
</style>
