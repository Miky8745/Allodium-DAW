<script lang="ts">
  import Slider from '../Slider.svelte';
  import Dial from '../Dial.svelte';
  import PeakMeterVertical from './PeakMeterVertical.svelte';
  import MixerMuteSolo from './MixerMuteSolo.svelte';
  import MixerRoutePoint from './MixerRoutePoint.svelte';
  import { mixerStore, type MixerChannelData, type MasterChannelData } from '$lib/mixerStore.svelte';
  import { formatPan } from '$lib/panFormat';

  let {
    variant,
    data,
    onRemove,
  }: {
    variant: 'master' | 'channel';
    data: MixerChannelData | MasterChannelData;
    onRemove?: () => void;
  } = $props();

  // Master is selectable too (to view/edit its own FX chain in
  // MixerFxRack.svelte and to highlight it) — it just never gets a routing
  // arrow below, since it has nowhere to output to.
  let isSelected = $derived(data.id === mixerStore.selectedChannelId);

  // This strip's routing dot: does the currently-selected channel route
  // into this one? Meaningless (and hidden) for the selected strip itself,
  // which shows the arrow instead. Naturally false whenever Master is
  // selected too, since mixerStore.selectedChannel is scoped to `channels`
  // and never matches Master — Master can't be the source of a route.
  let routedFromSelected = $derived(mixerStore.selectedChannel?.routeTo.includes(data.id) ?? false);

  // Master has no `name` field of its own (its label is always the fixed
  // "MASTER" string, see the template) — used to give the Pan dial/Volume
  // fader's hover-info name a per-strip label (e.g. "Kick pan dial") rather
  // than a generic one, since the Mixer can have many strips at once.
  let stripName = $derived(variant === 'master' ? 'Master' : (data as MixerChannelData).name);

  function selectStrip() {
    mixerStore.selectChannel(data.id);
  }

  function onDotClick() {
    if (!mixerStore.routingArmed || mixerStore.selectedChannelId === null) return;
    mixerStore.toggleRoute(mixerStore.selectedChannelId, data.id);
  }

  // Rename (channel strips only — Master keeps its fixed label).
  let renaming = $state(false);
  let nameDraft = $state('');
  let nameInputEl = $state<HTMLInputElement | null>(null);

  // Matches NewProjectModal's own focus-on-open convention — the input
  // isn't in the DOM yet on the click that sets renaming=true, so it can't
  // be focused synchronously from startRename() itself.
  $effect(() => {
    if (renaming) nameInputEl?.select();
  });

  function startRename() {
    if (variant !== 'channel') return;
    nameDraft = (data as MixerChannelData).name;
    renaming = true;
  }

  // Escape hides the input (via renaming=false) before its own onblur can
  // fire, but removing a focused element from the DOM can still dispatch a
  // blur event on the way out — this flag stops that from re-committing the
  // just-cancelled edit.
  let renameCancelled = false;

  function commitRename() {
    if (renameCancelled) { renameCancelled = false; renaming = false; return; }
    const trimmed = nameDraft.trim();
    if (trimmed) (data as MixerChannelData).name = trimmed;
    renaming = false;
  }

  function renameKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') { e.preventDefault(); commitRename(); }
    else if (e.key === 'Escape') { e.preventDefault(); renameCancelled = true; renaming = false; }
  }

  // The fader/meter should fill whatever vertical room the strip has (it
  // now stretches to the Mixer window's full height — see Mixer.svelte's
  // `.channels-scroll { align-items: stretch }`), but Slider/PeakMeterVertical
  // both take a plain pixel `height` number (used for more than just CSS —
  // Slider's own drag-sensitivity math divides by it), so a CSS-only
  // `flex: 1` on the fader row isn't enough on its own; the actual rendered
  // height has to be measured and fed back in. `.strip`'s own clientHeight
  // (via Svelte's built-in ResizeObserver-backed binding) gives the total,
  // and `.strip-top`/`.strip-bottom` — the two content blocks that sandwich
  // the fader — give what to subtract; whatever's left is the fader's size.
  let stripHeight = $state(0);
  let topHeight = $state(0);
  let bottomHeight = $state(0);

  // .strip's own vertical padding (8px + 10px, see its CSS below) — part of
  // clientHeight but not available content space.
  const STRIP_PADDING_V = 18;
  // .strip's flex `gap` (8px) between its 3 direct children (top block,
  // fader, bottom block) — 2 gaps.
  const STRIP_GAP = 8;

  let faderHeight = $derived(
    Math.max(40, stripHeight - STRIP_PADDING_V - topHeight - bottomHeight - STRIP_GAP * 2),
  );
</script>

<div
  class="strip"
  class:strip--master={variant === 'master'}
  class:strip--selected={isSelected}
  onclick={selectStrip}
  onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selectStrip(); } }}
  role="button"
  tabindex={0}
  bind:clientHeight={stripHeight}
>
  <div class="strip-top" bind:clientHeight={topHeight}>
    <div class="strip-head">
      {#if renaming}
        <input
          class="name-input"
          bind:this={nameInputEl}
          bind:value={nameDraft}
          onblur={commitRename}
          onkeydown={renameKeydown}
          onclick={(e) => e.stopPropagation()}
        />
      {:else}
        <span
          class="strip-label"
          ondblclick={(e) => { e.stopPropagation(); startRename(); }}
          role="region"
          aria-label="Channel name"
          title={variant === 'channel' ? 'Double-click to rename' : undefined}
        >{variant === 'master' ? 'MASTER' : (data as MixerChannelData).name}</span>
      {/if}
      {#if variant === 'channel' && onRemove}
        <button class="remove-btn" onclick={(e) => { e.stopPropagation(); onRemove(); }} aria-label="Remove channel" title="Remove channel">×</button>
      {/if}
    </div>

    {#if variant === 'channel'}
      {@const ch = data as MixerChannelData}
      <Dial bind:value={ch.pan} defaultValue={0.5} size={22} sensitivity={150} tooltipKey="PAN" format={formatPan} name="{stripName} pan dial" />
    {/if}
  </div>

  <div class="strip-meter-fader">
    <PeakMeterVertical giant={variant === 'master'} height={faderHeight} trackId={data.id} />
    <Slider bind:value={data.volume} min={0} max={2} defaultValue={0.8} height={faderHeight} tooltipKey="VOL" name="{stripName} volume fader" />
  </div>

  <div class="strip-bottom" bind:clientHeight={bottomHeight}>
    {#if variant === 'channel'}
      {@const ch = data as MixerChannelData}
      <MixerMuteSolo data={ch} />
    {/if}

    <div class="strip-route">
      {#if isSelected && variant === 'channel'}
        <MixerRoutePoint
          kind="arrow"
          active={mixerStore.routingArmed}
          onClick={() => mixerStore.toggleRoutingArmed()}
          title={mixerStore.routingArmed ? 'Click a channel to route to it' : 'Route this channel'}
        />
      {:else if !isSelected}
        <MixerRoutePoint
          kind="dot"
          active={routedFromSelected}
          disabled={!mixerStore.routingArmed}
          onClick={onDotClick}
          title={routedFromSelected ? 'Routed from the selected channel' : 'Not routed'}
        />
      {/if}
      <!-- else: Master is selected — it has nowhere to output to, so this
           slot stays empty (the wrapping div keeps its height so the strip's
           layout doesn't jump when selection moves to/from Master). -->
    </div>
  </div>
</div>

<style>
  .strip {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    width: 64px;
    flex-shrink: 0;
    padding: 8px 6px 10px;
    background: var(--btn-bg, #282b33);
    border: 1px solid var(--btn-border, #383b43);
    border-radius: 9px;
    cursor: pointer;
    transition: border-color 0.1s, background 0.1s;
  }

  .strip--master {
    width: 110px;
  }

  .strip--selected {
    border-color: var(--accent, #e6b07f);
    background: rgba(230,176,127, 0.06);
  }

  .strip-top,
  .strip-bottom {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    width: 100%;
    flex-shrink: 0;
  }

  .strip-head {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    width: 100%;
    min-width: 0;
  }

  .strip-label {
    font-family: 'DM Mono', 'Cascadia Code', monospace;
    font-size: 10px;
    letter-spacing: 0.05em;
    color: var(--main-text, #bcbcc0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .strip--master .strip-label {
    font-weight: 700;
    color: var(--accent, #e6b07f);
  }

  .name-input {
    width: 100%;
    min-width: 0;
    font-family: 'DM Mono', 'Cascadia Code', monospace;
    font-size: 10px;
    letter-spacing: 0.05em;
    color: var(--main-text, #bcbcc0);
    background: #171a23;
    border: 1px solid var(--accent, #e6b07f);
    border-radius: 4px;
    padding: 1px 3px;
    text-align: center;
  }

  .remove-btn {
    width: 13px;
    height: 13px;
    line-height: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: transparent;
    color: rgba(255, 255, 255, 0.35);
    font-size: 12px;
    cursor: pointer;
    padding: 0;
    border-radius: 4px;
    flex-shrink: 0;
    transition: color 0.1s, background 0.1s;
  }

  .remove-btn:hover {
    color: #f08484;
    background: rgba(220, 50, 50, 0.15);
  }

  .strip-meter-fader {
    display: flex;
    align-items: flex-end;
    gap: 8px;
  }

  .strip-route {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 16px;
  }
</style>
