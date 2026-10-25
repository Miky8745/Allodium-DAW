<script lang="ts">
  import { onMount } from 'svelte';
  import { packStore } from '$lib/instrument/packs/packStore.svelte';
  import { formatBytes, installedPresetPath, type CatalogInstrument, type CatalogPack } from '$lib/instrument/packs/types';

  // Bloom's "Get more…" view: every downloadable instrument from the pack
  // catalog, installable one at a time (only its own samples are fetched).
  let { onBack, onLoad }: { onBack: () => void; onLoad: (presetPath: string) => void } = $props();

  let packId = $state<string | null>(null);
  let query = $state('');

  onMount(() => {
    packStore.loadCatalog().then(c => { packId ??= c.packs[0]?.id ?? null; });
    void packStore.refreshInstalled();
  });

  let pack = $derived(packStore.catalog?.packs.find(p => p.id === packId) ?? null);

  let groups = $derived.by(() => {
    if (!pack) return [];
    const q = query.trim().toLowerCase();
    const map = new Map<string, CatalogInstrument[]>();
    for (const inst of pack.instruments) {
      if (q && !`${inst.name} ${inst.category}`.toLowerCase().includes(q)) continue;
      if (!map.has(inst.category)) map.set(inst.category, []);
      map.get(inst.category)!.push(inst);
    }
    return [...map.entries()].sort((a, b) => a[0].localeCompare(b[0]));
  });

  let installedCount = $derived(pack ? pack.instruments.filter(i => packStore.installed.has(installedPresetPath(pack!, i))).length : 0);

  function percent(p: CatalogPack, inst: CatalogInstrument): number {
    const pr = packStore.progress;
    if (packStore.status(p, inst) !== 'downloading' || !pr || pr.totalBytes === 0) return 0;
    return Math.round((pr.doneBytes / pr.totalBytes) * 100);
  }
</script>

<div class="pk">
  <div class="pk-top">
    <button class="pk-btn" onclick={onBack}>← Presets</button>
    <div class="pk-tabs" role="tablist">
      {#each packStore.catalog?.packs ?? [] as p (p.id)}
        <button class="pk-tab" class:pk-tab--on={p.id === packId} role="tab" aria-selected={p.id === packId} onclick={() => (packId = p.id)}>
          {p.name} <span class="pk-tab-n">{p.instruments.length}</span>
        </button>
      {/each}
    </div>
    <input class="pk-search" type="text" placeholder="Search instruments" bind:value={query} />
    {#if packStore.current || packStore.queue.length > 0}
      <button class="pk-btn" onclick={() => packStore.cancelAll()} title="Stop downloading">Cancel downloads</button>
    {/if}
  </div>

  {#if pack}
    <p class="pk-about">
      {pack.credit} — {pack.license}, free to use and share. {installedCount} of {pack.instruments.length} installed.
      Installing downloads only that instrument's samples.
    </p>
  {/if}

  <div class="pk-list">
    {#if !packStore.catalog}
      <p class="pk-note">Loading catalog…</p>
    {/if}
    {#each groups as [category, items] (category)}
      <div class="pk-cat">{category}</div>
      {#each items as inst (inst.sfz)}
        {@const st = packStore.status(pack!, inst)}
        {@const err = packStore.errors.get(installedPresetPath(pack!, inst))}
        <div class="pk-row" class:pk-row--installed={st === 'installed'}>
          <div class="pk-name">
            <span>{inst.name}</span>
            {#if err}<span class="pk-err" title={err}>Download failed — {err}</span>{/if}
          </div>
          <span class="pk-size">{formatBytes(inst.bytes)}</span>
          {#if st === 'downloading'}
            <div class="pk-bar" title="{percent(pack!, inst)}%"><div class="pk-bar-fill" style="width:{percent(pack!, inst)}%"></div></div>
          {:else if st === 'queued'}
            <span class="pk-state">Queued</span>
          {:else if st === 'installed'}
            <button class="pk-btn pk-btn--accent" onclick={() => onLoad(installedPresetPath(pack!, inst))}>Load</button>
            <button class="pk-btn pk-btn--danger" onclick={() => packStore.remove(pack!, inst)} title="Delete this instrument's files">Remove</button>
          {:else}
            <button class="pk-btn" onclick={() => packStore.install(pack!, inst)}>{err ? 'Retry' : 'Install'}</button>
          {/if}
        </div>
      {/each}
    {/each}
  </div>
</div>

<style>
  .pk {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
    background: var(--main-bg);
  }
  .pk-top {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    border-bottom: 1px solid var(--main-border);
    flex-wrap: wrap;
  }
  .pk-tabs { display: flex; gap: 3px; }
  .pk-tab {
    height: 24px;
    padding: 0 10px;
    font-size: 12px;
    border-radius: 5px;
    border: 1px solid var(--btn-border);
    background: var(--btn-bg);
    color: var(--btn-text);
    cursor: pointer;
  }
  .pk-tab:hover { background: var(--btn-hover); }
  .pk-tab--on, .pk-tab--on:hover {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-text);
  }
  .pk-tab-n { opacity: 0.65; font-size: 10px; }
  .pk-search {
    flex: 1;
    min-width: 120px;
    height: 24px;
    padding: 0 8px;
    font-size: 12px;
    border-radius: 5px;
    border: 1px solid var(--btn-border);
    background: var(--search-bg);
    color: var(--main-text);
  }
  .pk-about {
    margin: 0;
    padding: 6px 10px;
    font-size: 11px;
    color: var(--main-text-muted);
    border-bottom: 1px solid var(--main-border);
  }
  .pk-list {
    flex: 1;
    overflow-y: auto;
    padding: 2px 0 10px;
  }
  .pk-cat {
    padding: 10px 12px 4px;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--main-text-muted);
  }
  .pk-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 6px;
    padding: 4px 6px;
    border-radius: 5px;
    font-size: 12px;
    color: var(--main-text);
  }
  .pk-row:hover { background: rgba(255, 255, 255, 0.03); }
  .pk-name {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .pk-row--installed .pk-name > span:first-child { color: var(--accent); }
  .pk-err {
    font-size: 10px;
    color: #f08484;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pk-size {
    width: 56px;
    text-align: right;
    font-size: 11px;
    color: var(--main-text-muted);
    font-variant-numeric: tabular-nums;
  }
  .pk-state {
    width: 70px;
    text-align: center;
    font-size: 11px;
    color: var(--main-text-muted);
  }
  .pk-bar {
    width: 120px;
    height: 6px;
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.08);
    overflow: hidden;
  }
  .pk-bar-fill {
    height: 100%;
    background: var(--accent);
    transition: width 0.15s;
  }
  .pk-btn {
    height: 22px;
    padding: 0 9px;
    font-size: 11px;
    border-radius: 5px;
    border: 1px solid var(--btn-border);
    background: var(--btn-bg);
    color: var(--btn-text);
    cursor: pointer;
    white-space: nowrap;
  }
  .pk-btn:hover { background: var(--btn-hover); }
  .pk-btn--accent {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-text);
  }
  .pk-btn--accent:hover { background: var(--accent-hover); }
  .pk-btn--danger:hover {
    background: rgba(220, 50, 50, 0.18);
    color: #f08484;
    border-color: rgba(220, 50, 50, 0.3);
  }
  .pk-note {
    margin: 12px;
    font-size: 12px;
    color: var(--main-text-muted);
  }
</style>
