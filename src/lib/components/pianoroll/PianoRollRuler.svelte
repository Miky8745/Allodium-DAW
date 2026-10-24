<script lang="ts">
  import { stepW, patternWidth } from '$lib/pianoroll/pitch';

  let {
    patternLength,
    scrollLeft,
    activeStep,
  }: {
    patternLength: number;
    scrollLeft: number;
    activeStep: number;
  } = $props();

  function stepGroup(i: number) {
    return Math.floor(i / 4) % 2; // 0 or 1 — one beat per group of 4 steps
  }
</script>

<div class="ruler-viewport">
  <div class="ruler-content" style="width:{patternWidth(patternLength)}px; transform: translateX(-{scrollLeft}px);">
    {#each Array(patternLength) as _, i}
      <div
        class="ruler-step"
        class:ruler-step--grey={stepGroup(i) === 0}
        class:ruler-step--orange={stepGroup(i) === 1}
        class:ruler-step--playing={i === activeStep}
        style="width:{stepW()}px;"
      >
        {#if i % 4 === 0}<span class="ruler-num">{i / 4 + 1}</span>{/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .ruler-viewport {
    height: 100%;
    overflow: hidden;
    background: #1d1f29;
    border-bottom: 1px solid var(--explorer-border, #383c4a);
  }

  .ruler-content {
    display: flex;
    height: 100%;
  }

  .ruler-step {
    box-sizing: border-box;
    height: 100%;
    border-right: 1px solid rgba(255,255,255,0.04);
    display: flex;
    align-items: center;
    padding-left: 3px;
    flex-shrink: 0;
  }

  .ruler-step--grey   { background: #24262f; }
  .ruler-step--orange { background: #281f16; }
  .ruler-step--playing { outline: 2px solid rgba(255,255,255,0.55); outline-offset: -2px; }

  .ruler-num {
    font-size: 9px;
    font-family: 'DM Mono', monospace;
    color: rgba(255,255,255,0.35);
    pointer-events: none;
  }
</style>
