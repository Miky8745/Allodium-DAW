<script lang="ts">
  import type { Note } from '$lib/types';
  import { BASE_STEP_W, stepToX, patternWidth } from '$lib/pianoroll/pitch';

  let {
    notes,
    patternLength,
    activeStep = -1,
    rowHeight = 28,
    fit = 'steps',
  }: {
    notes: Note[];
    patternLength: number;
    activeStep?: number;
    rowHeight?: number;
    fit?: 'steps' | 'stretch';
  } = $props();

  const MINI_MARGIN = 3;
  const MINI_MIN_BAR_H = 3;

  let pitchRange = $derived.by(() => {
    let min = Infinity, max = -Infinity;
    for (const n of notes) {
      if (n.pitch < min) min = n.pitch;
      if (n.pitch > max) max = n.pitch;
    }
    if (min === Infinity) { min = 0; max = 0; }
    return { min, max };
  });

  function barHeightPct() {
    const span = Math.max(1, pitchRange.max - pitchRange.min);
    return Math.max(3, (100 - 2 * 3) / (span + 1));
  }

  function barTopPct(pitch: number) {
    const span = Math.max(1, pitchRange.max - pitchRange.min);
    return 3 + ((pitchRange.max - pitch) / span) * (100 - 2 * 3 - barHeightPct());
  }

  function barHeight() {
    const span = Math.max(1, pitchRange.max - pitchRange.min);
    return Math.max(MINI_MIN_BAR_H, (rowHeight - 2 * MINI_MARGIN) / (span + 1));
  }

  function barTop(pitch: number) {
    const span = Math.max(1, pitchRange.max - pitchRange.min);
    const barH = barHeight();
    return MINI_MARGIN + ((pitchRange.max - pitch) / span) * (rowHeight - 2 * MINI_MARGIN - barH);
  }
</script>

{#if fit === 'stretch'}
  <div class="mini-roll mini-roll--stretch" style="height:{rowHeight}px;">
    {#each notes as note (note.id)}
      <div
        class="mini-note"
        style="left:{note.start / patternLength * 100}%; top:{barTopPct(note.pitch)}%; width:{note.length / patternLength * 100}%; height:{barHeightPct()}%;"
      ></div>
    {/each}
    {#if activeStep >= 0}
      <!-- transform, not left: per-frame layout changes are what make overlapping windows stutter (see NoteGrid's playhead overlay) -->
      <div class="mini-playhead" style="width:{1 / patternLength * 100}%; transform: translateX({activeStep * 100}%);"></div>
    {/if}
  </div>
{:else}
  <div class="mini-roll" style="width:{patternWidth(patternLength, BASE_STEP_W)}px; height:{rowHeight}px;">
    {#each notes as note (note.id)}
      <div
        class="mini-note"
        style="left:{stepToX(note.start, BASE_STEP_W)}px; top:{barTop(note.pitch)}px; width:{note.length * BASE_STEP_W - 1}px; height:{barHeight()}px;"
      ></div>
    {/each}
    {#if activeStep >= 0}
      <div class="mini-playhead" style="width:{BASE_STEP_W}px; transform: translateX({stepToX(activeStep, BASE_STEP_W)}px);"></div>
    {/if}
  </div>
{/if}

<style>
  .mini-roll {
    position: relative;
    border-radius: 5px;
    background: #20232c;
    overflow: hidden;
    flex-shrink: 0;
  }

  .mini-roll--stretch {
    width: 100%;
  }

  .mini-note {
    position: absolute;
    box-sizing: border-box;
    background: var(--accent, #e6b07f);
    border: 1px solid rgba(0,0,0,0.4);
    border-radius: 4px;
  }

  .mini-playhead {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    background: rgba(255,255,255,0.08);
    outline: 1px solid rgba(255,255,255,0.3);
    pointer-events: none;
  }
</style>
