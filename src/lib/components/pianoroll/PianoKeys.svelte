<script lang="ts">
  import { MIN_PITCH, MAX_PITCH, keyH, gridTotalH, isBlackKey, isCNote, pitchName } from '$lib/pianoroll/pitch';

  // midiPitches: live-held notes (MIDI input / QWERTY keyboard, see
  // liveNotes.svelte.ts), pitch -> 0..1 velocity — drawn as a glow whose
  // intensity follows velocity.
  let { scrollTop, highlightPitches = new Set<number>(), midiPitches }: {
    scrollTop: number;
    highlightPitches?: Set<number>;
    midiPitches?: ReadonlyMap<number, number>;
  } = $props();

  const pitches = Array.from({ length: MAX_PITCH - MIN_PITCH + 1 }, (_, i) => MAX_PITCH - i);
</script>

<div class="keys-viewport">
  <div class="keys-content" style="height:{gridTotalH()}px; transform: translateY(-{scrollTop}px);">
    {#each pitches as p, i (p)}
      {@const belowP = pitches[i + 1]}
      {@const noBorder = isBlackKey(p) || (belowP !== undefined && isBlackKey(belowP))}
      {@const highlighted = highlightPitches.has(p)}
      {@const csharpBelowCHighlighted = p % 12 === 1 && highlightPitches.has(p - 1)}
      {@const glow = midiPitches?.get(p)}
      <div
        class="key-row"
        class:key-row--c={isCNote(p)}
        class:key-row--no-border={noBorder}
        class:key-row--csharp={p % 12 === 1}
        class:key-row--highlighted={highlighted && !isBlackKey(p)}
        class:key-row--csharp-below-highlighted={csharpBelowCHighlighted}
        style="height:{keyH()}px;"
      >
        {#if glow !== undefined && !isBlackKey(p)}
          <div class="midi-glow" style="--glow:{glow};"></div>
        {/if}
        {#if isBlackKey(p)}
          <div class="black-key" class:black-key--highlighted={highlighted}>
            {#if glow !== undefined}<div class="midi-glow" style="--glow:{glow};"></div>{/if}
          </div>
        {:else}
          <span class="key-label" class:key-label--c={isCNote(p)}>{pitchName(p)}</span>
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .keys-viewport {
    height: 100%;
    overflow: hidden;
    position: relative;
    background: #1b1e27;
    /* Key palette in one place. Deliberately close together — pure
       white/black keys against the dark grid were too harsh. */
    --key-white: #b5b6b9;
    --key-c:     #a3a5a9;
    --key-black: #383b43;
  }

  .keys-content {
    position: relative;
  }

  .key-row {
    position: relative;
    box-sizing: border-box;
    background: var(--key-white);
    border-bottom: 1px solid rgba(0,0,0,0.3);
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding-right: 6px;
  }

  /* C rows are shaded grey so octave boundaries are easy to scan at a glance. */
  .key-row--c { background: var(--key-c); border-bottom: 1px solid rgba(255,255,255,0.15); }

  /* Real pianos have no visible seam between a white key and the black key
     sitting on top of/beside it — only white-to-white boundaries with no
     black key between them (E/F, B/C) show a line. */
  .key-row--no-border { border-bottom: none; }

  /* Greying out the bottom half of the exposed white sliver next to C#'s
     black key (the part not covered by .black-key) makes the row read as
     if it had shifted up, away from the grey C row below it. */
  .key-row--csharp { background: linear-gradient(to bottom, var(--key-white) 50%, var(--key-c) 50%); }

  /* Orange highlight while a note is being created/resized/moved in the grid
     (see NoteGrid's highlightPitches). Whole row for plain white/C keys;
     only the nested .black-key div for black-key rows, since the row's own
     white/grey background must stay put. The C# row's gradient bottom stop
     is tinted independently whenever the C row directly below it (p-1) is
     highlighted, so the C key's grey-sliver illusion into the C# row above
     it isn't broken when only the C row's own highlight changes. */
  .key-row--highlighted { background: var(--pr-key-highlight, #f2a766); }

  .key-row--csharp-below-highlighted {
    background: linear-gradient(to bottom, var(--key-white) 50%, var(--pr-key-highlight, #f2a766) 50%);
  }

  /* Real black keys are shorter (don't reach as far toward the grid) and
     slimmer (don't fill the full semitone row) than the white keys they
     interrupt — the row itself stays full height so it still lines up
     1:1 with the note grid. */
  .black-key {
    position: absolute;
    left: 0;
    top: 2px;
    bottom: 2px;
    width: 62%;
    background: var(--key-black);
    border-radius: 0 4px 4px 0;
    box-shadow: 1px 0 2px rgba(0,0,0,0.3);
    pointer-events: none;
  }

  /* Compound selector (not source order) so this reliably beats the base
     .black-key background regardless of where it's declared in the file. */
  .black-key.black-key--highlighted { background: var(--pr-key-highlight, #f2a766); }

  /* Live MIDI/QWERTY note glow — opacity and bloom scale with velocity
     (--glow, 0..1). Inside .black-key for black keys so it takes the key's
     own shape; spans the whole row for white keys. */
  .midi-glow {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: var(--accent);
    opacity: calc(0.45 + 0.55 * var(--glow));
    box-shadow:
      0 0 calc(3px + 9px * var(--glow)) var(--accent),
      inset 0 0 calc(2px + 4px * var(--glow)) rgba(255, 255, 255, 0.5);
    pointer-events: none;
    z-index: 2;
  }

  .key-label {
    font-size: 8px;
    font-family: 'DM Mono', monospace;
    color: rgba(0,0,0,0.45);
    line-height: 1;
    pointer-events: none;
    z-index: 3;
  }

  /* C is the octave anchor — make its label read more prominently than the
     other white-key labels. */
  .key-label--c {
    color: #252831;
    font-weight: 700;
  }
</style>
