<script lang="ts">
  // Combined multi-channel thumbnail for a pattern placed on the Playlist —
  // flattens every channel's piano-roll notes into one MiniNoteRoll render
  // (its geometry is already channel-agnostic, see MiniNoteRoll.svelte), and
  // for step-sequencer-only channels (no notes, just steps[]) synthesizes
  // one note per active step on a virtual row just below the lowest melodic
  // note, sized to that channel's sample's own decoded duration converted to
  // steps rather than a fixed 1-step sliver.
  import { DEFAULT_NOTE_PROPS } from '$lib/noteProps';
  import { channelStore } from '$lib/channelStore.svelte';
  import { patternStore } from '$lib/patternStore.svelte';
  import { playback } from '$lib/playbackStore.svelte';
  import { audioEngine } from '$lib/audioEngine';
  import { channelLoopLength } from '$lib/channelLoop';
  import { profiler } from '$lib/debug/profiler';
  import MiniNoteRoll from '../pianoroll/MiniNoteRoll.svelte';
  import type { Note } from '$lib/types';

  let { patternId, lengthBeats }: { patternId: number; lengthBeats: number } = $props();

  let channels = $derived(channelStore.getPatternChannels(patternId, patternStore.selectedPatternId));

  // Explicit rerender trigger: increments whenever this pattern's content
  // or placement/length changes, forcing MiniNoteRoll to remount so
  // modifications are always visible.
  let rerenderVersion = $derived.by(() => {
    let hash = 0;
    for (const c of channels) hash = hash ^ c.id ^ c.steps.length ^ c.notes.length;
    return hash + lengthBeats + patternStore.selectedPatternId;
  });

  // samplePath -> that sample's own duration, in 16th-note steps at the
  // current tempo. Populated async as loadSample() resolves (it's globally
  // cached in audioEngine, so this just remembers the converted step count
  // rather than re-deriving it on every render); a step-channel's synthetic
  // note falls back to a 1-step length until its entry lands here.
  let sampleStepLengths = $state<Record<string, number>>({});

  $effect(() => {
    const stepDur = 60 / playback.tempo / 4;
    const wantedFiles = new Set<string>();
    for (const ch of channels) {
      if (ch.notes.length === 0 && ch.samplePath && ch.steps.some(Boolean)) wantedFiles.add(ch.samplePath);
    }
    for (const path of wantedFiles) {
      if (path in sampleStepLengths) continue;
      audioEngine.loadSample(path).then(buf => {
        if (!buf) return;
        sampleStepLengths = { ...sampleStepLengths, [path]: buf.duration / stepDur };
      });
    }
  });

  // Unique ids across channels: `channelId * 1_000_000 + localId` — each
  // channel is either notes-driven or steps-driven, never both, so this
  // can't collide within a channel, and the multiplier keeps channels apart
  // (large enough to cover a synthetic step's absolute position too, not
  // just its within-bar index — see the tiling below).
  //
  // Derivation of `flatNotes` also implicitly depends on `playback.tempo`
  // and `channelStore.barLength` via `channelLoopLength`, which means
  // the component re-renders whenever the pattern's content, tempo, or
  // beat structure changes — this is what triggers a rerender when a
  // pattern is placed down or modified.
  let flatNotes = $derived.by(() => {
    profiler.count('derived.patternPreview');
    const patternLengthSteps = lengthBeats * 4;
    const barLength = channelStore.barLength;
    const result: Note[] = [];

    // Step channels live on one "virtual row" just below the lowest melodic
    // note, as if the pattern had a super-deep tone underneath its real
    // range. It's a real row inside MiniNoteRoll's autofit (the range grows
    // by one), so it's always on-screen — sitting at a fixed pitch far below
    // the melodic range would put it outside the thumbnail and clip it away.
    let lowestMelodic = Infinity;
    for (const ch of channels) {
      for (const n of ch.notes) if (n.pitch < lowestMelodic) lowestMelodic = n.pitch;
    }
    const virtualPitch = lowestMelodic === Infinity ? 0 : Math.max(0, lowestMelodic - 1);

    for (const ch of channels) {
      if (ch.notes.length > 0) {
        for (const n of ch.notes) result.push({ ...n, id: ch.id * 1_000_000 + n.id });
      } else if (ch.samplePath || ch.sampleFolder) {
        const key = ch.samplePath || ch.sampleFolder || '';
        // Drawn at a quarter of the sample's real length, and never longer
        // than one step, so hits on adjacent steps stay visibly separate
        // even when every step is filled.
        const lengthSteps = Math.min(1, (sampleStepLengths[key] ?? 4) / 4);
        // Mirrors audioEngine's Song-mode scheduling (scheduleChannelStep
        // with loopNotes=false, see CLAUDE.md's "Song mode" section): a
        // step-sequenced channel loops to fill the whole block, so its
        // preview needs to tile every repeat too, not just draw its first
        // cycle — otherwise a short drum loop plays for the full block but
        // visually appears to stop after one bar.
        const loopLen = channelLoopLength(ch, patternLengthSteps, barLength);
        for (let cycleStart = 0; cycleStart < patternLengthSteps; cycleStart += loopLen) {
          ch.steps.forEach((active, i) => {
            // Steps at/after loopLen never play (audioEngine wraps the step
            // modulo loopLen), e.g. steps 12–15 in a 3/4 pattern.
            if (!active || i >= loopLen) return;
            const start = cycleStart + i;
            if (start >= patternLengthSteps) return;
            result.push({
              ...DEFAULT_NOTE_PROPS,
              id: ch.id * 1_000_000 + start,
              pitch: virtualPitch,
              start,
              length: Math.min(lengthSteps, patternLengthSteps - start),
              velocity: 1,
            });
          });
        }
      }
    }
    return result;
  });
</script>

{#key rerenderVersion}
  <MiniNoteRoll notes={flatNotes} patternLength={lengthBeats * 4} fit="stretch" />
{/key}