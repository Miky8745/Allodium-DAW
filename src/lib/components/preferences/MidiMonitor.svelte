<script lang="ts">
  // Last 100 MIDI messages in either direction (midiStore.log). Incoming
  // lines are the raw message as received (before per-device processing);
  // outgoing lines are what was actually sent.
  import { tick } from 'svelte';
  import { midiStore } from '$lib/midi/midiStore.svelte';

  let listEl: HTMLDivElement | undefined = $state();
  let paused = $state(false);

  $effect(() => {
    void midiStore.log.length;
    void midiStore.log[midiStore.log.length - 1];
    if (paused || !listEl) return;
    tick().then(() => { if (listEl) listEl.scrollTop = listEl.scrollHeight; });
  });

  function fmt(t: number) {
    return (t / 1000).toFixed(3).padStart(9, ' ');
  }
</script>

<div class="monitor">
  <div class="bar">
    <span class="count">{midiStore.log.length} messages</span>
    <button class="mini-btn" class:mini-btn--on={paused} onclick={() => (paused = !paused)}>{paused ? 'Resume scroll' : 'Pause scroll'}</button>
    <button class="mini-btn" onclick={() => midiStore.clearLog()}>Clear</button>
  </div>
  <div class="log" bind:this={listEl}>
    {#each midiStore.log as e (e.id)}
      <div class="line" class:line--out={e.direction === 'out'}>
        <span class="t">{fmt(e.time)}</span>
        <span class="dir">{e.direction === 'in' ? 'IN ' : 'OUT'}</span>
        <span class="text">{e.text}</span>
        <span class="port" title={e.port}>{e.port}</span>
      </div>
    {:else}
      <div class="empty">Waiting for MIDI…</div>
    {/each}
  </div>
</div>

<style>
  .monitor {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .count {
    flex: 1;
    font-size: 11px;
    color: var(--main-text-muted);
  }
  .log {
    height: 160px;
    overflow: auto;
    background: #14161d;
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 6px;
    padding: 4px 6px;
    font-family: 'DM Mono', 'Cascadia Code', monospace;
    font-size: 10.5px;
    user-select: text;
  }
  .line {
    display: grid;
    grid-template-columns: 62px 28px 1fr minmax(0, 160px);
    gap: 8px;
    color: var(--main-text);
    line-height: 1.55;
  }
  .line--out { color: var(--accent); }
  .t, .port { color: var(--main-text-muted); }
  .t { white-space: pre; }
  .port {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: right;
  }
  .empty { color: var(--main-text-muted); padding: 4px 0; }
  .mini-btn {
    padding: 3px 8px;
    border-radius: 5px;
    border: 1px solid var(--btn-border);
    background: var(--btn-bg);
    color: var(--btn-text);
    font-family: inherit;
    font-size: 11px;
    cursor: pointer;
  }
  .mini-btn:hover { background: var(--btn-hover); }
  .mini-btn--on { border-color: var(--accent); color: var(--accent); }
</style>
