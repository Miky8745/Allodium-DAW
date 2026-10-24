<script lang="ts">
  // Tiny SVG plot of incoming → outgoing velocity for the current input
  // settings — computed with the exact same shapeVelocity() the live path
  // uses, so it can't drift from actual behavior.
  import type { MidiInputPrefs } from '$lib/preferencesTypes';
  import { shapeVelocity } from '$lib/midi/midiProcessing';

  let { prefs }: { prefs: MidiInputPrefs } = $props();

  const W = 120;
  const H = 60;

  let path = $derived.by(() => {
    const pts: string[] = [];
    for (let v = 1; v <= 127; v += 2) {
      const out = shapeVelocity(v, prefs);
      pts.push(`${((v / 127) * W).toFixed(1)},${(H - (out / 127) * H).toFixed(1)}`);
    }
    return `M${pts.join(' L')}`;
  });
</script>

<svg width={W} height={H} viewBox="0 0 {W} {H}" class="curve" aria-label="Velocity response curve">
  <line x1="0" y1={H} x2={W} y2="0" class="diag" />
  <path d={path} class="line" />
</svg>

<style>
  .curve {
    background: #171a23;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 5px;
  }
  .diag {
    stroke: rgba(255, 255, 255, 0.08);
    stroke-dasharray: 3 3;
  }
  .line {
    fill: none;
    stroke: var(--accent);
    stroke-width: 1.5;
  }
</style>
