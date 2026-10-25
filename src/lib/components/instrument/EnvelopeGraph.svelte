<script lang="ts">
  // Small ADSR sketch for the amp envelope section. Times are drawn on a
  // log-ish scale so a 5 ms attack and a 3 s release both stay visible.
  let { attack, decay, sustain, release }: { attack: number; decay: number; sustain: number; release: number } = $props();

  const W = 120, H = 44, PAD = 3;
  const seg = (s: number) => Math.log10(1 + s * 100) / Math.log10(1001); // 0..10 s → 0..1-ish
  let path = $derived.by(() => {
    const a = seg(attack), d = seg(decay), r = seg(release), hold = 0.25;
    const total = a + d + hold + r || 1;
    const x = (u: number) => PAD + (u / total) * (W - PAD * 2);
    const y = (lvl: number) => H - PAD - lvl * (H - PAD * 2);
    return `M ${x(0)} ${y(0)} L ${x(a)} ${y(1)} L ${x(a + d)} ${y(sustain)} L ${x(a + d + hold)} ${y(sustain)} L ${x(total)} ${y(0)}`;
  });
</script>

<svg class="env" viewBox="0 0 {W} {H}" width={W} height={H} aria-hidden="true">
  <path d={path} fill="none" stroke="var(--accent)" stroke-width="1.6" stroke-linejoin="round" stroke-linecap="round" />
</svg>

<style>
  .env {
    background: rgba(0, 0, 0, 0.18);
    border: 1px solid var(--main-border);
    border-radius: 6px;
    flex-shrink: 0;
  }
</style>
