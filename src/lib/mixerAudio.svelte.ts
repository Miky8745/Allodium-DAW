// Bridges mixerStore's plain $state into audioEngine's real per-track Web
// Audio nodes. A plain module singleton (not a component) has no parent
// effect context, so — exactly like playbackStore.svelte.ts and
// historyStore.svelte.ts — this opens its own standalone $effect.root() via
// startWatching()/stopWatching(), wired into +page.svelte's
// openProject()/exitProject() alongside those two.
import { mixerStore } from '$lib/mixerStore.svelte';
import { audioEngine } from '$lib/audioEngine';

class MixerAudioBridge {
  #stop: (() => void) | null = null;

  constructor() {
    // Preferences > General latency/sample-rate changes rebuild the whole
    // AudioContext (audioEngine.rebuildContext), wiping every track chain —
    // restarting the bridge with a fresh lastRoutes re-creates them all and
    // re-applies every volume/pan/mute/routing edge on the new graph.
    audioEngine.onContextRebuilt(() => {
      if (!this.#stop) return;
      this.stopWatching();
      this.startWatching();
    });
  }

  startWatching(): void {
    if (this.#stop) return;

    // Each channel's most-recently-applied routeTo set — lets the routing
    // effect below send audioEngine only the edges that actually changed
    // (Web Audio has no bulk "replace all edges" call, just individual
    // connect()/disconnect()), and lets the lifecycle effect know who was
    // routing into a channel that just got removed.
    const lastRoutes = new Map<number, Set<number>>();

    this.#stop = $effect.root(() => {
      // Creates chains for new channels and tears down chains (plus every
      // still-live channel's edge into it) for removed ones. Declared
      // first for readability — Svelte effects still re-run independently
      // per their own dependencies, not in write order, and every method
      // called here is either idempotent create-if-absent or guards
      // against an already-gone target, so declaration order isn't a
      // correctness requirement, just where a reader would look first.
      $effect(() => {
        const liveIds = new Set(mixerStore.channels.map(ch => ch.id));
        for (const ch of mixerStore.channels) audioEngine.ensureTrackChain(ch.id);

        for (const id of [...lastRoutes.keys()]) {
          if (liveIds.has(id)) continue;
          // Sever every other channel's still-live edge into this
          // now-removed channel first — AudioNode.disconnect() only drops
          // a node's own OUTGOING edges, so the source side (not the
          // now-gone target) has to be the one to call it, or the
          // connection would dangle forever even after the target's own
          // internal wiring is torn apart below. Safe to run unconditionally
          // here even if the routing-diff effect already did this for a
          // given edge (via mixerStore.removeChannel()'s own routeTo
          // fixup) — disconnectTrackRoute() no-ops once its target chain
          // is gone, and by then lastRoutes no longer lists the edge
          // either way.
          for (const [otherId, targets] of lastRoutes) {
            if (targets.has(id)) {
              audioEngine.disconnectTrackRoute(otherId, id);
              targets.delete(id);
            }
          }
          audioEngine.removeTrackChain(id);
          lastRoutes.delete(id);
        }
      });

      $effect(() => {
        for (const ch of mixerStore.channels) audioEngine.setTrackVolume(ch.id, ch.volume);
      });

      $effect(() => {
        for (const ch of mixerStore.channels) audioEngine.setTrackPan(ch.id, ch.pan);
      });

      // Re-runs whenever ANY channel's muted/soloed changes, since
      // audibility is a function of the whole channel set (anySoloed), not
      // just the one channel being written.
      $effect(() => {
        for (const ch of mixerStore.channels) audioEngine.setTrackAudible(ch.id, mixerStore.isAudible(ch));
      });

      // routeTo is multi-target (fan-out) — diffs each channel's current
      // set against what was last applied and sends only the edges that
      // actually changed, rather than reconnecting everything every time
      // any channel's routing changes.
      $effect(() => {
        for (const ch of mixerStore.channels) {
          const prev = lastRoutes.get(ch.id) ?? new Set<number>();
          const next = new Set(ch.routeTo);
          for (const target of next) if (!prev.has(target)) audioEngine.connectTrackRoute(ch.id, target);
          for (const target of prev) if (!next.has(target)) audioEngine.disconnectTrackRoute(ch.id, target);
          lastRoutes.set(ch.id, next);
        }
      });

      $effect(() => {
        audioEngine.setMasterVolume(mixerStore.master.volume);
      });
    });
  }

  stopWatching(): void {
    this.#stop?.();
    this.#stop = null;
  }
}

export const mixerAudio = new MixerAudioBridge();
