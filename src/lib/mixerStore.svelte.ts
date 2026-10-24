// Ephemeral, session-only mixer state — not yet wired into projectSerializer.ts
// or historyStore's undo/redo (same treatment as windowFocusStore.svelte.ts).
// Persisting mixer routing/volumes is future work — a bigger tradeoff than
// before, since routing/mute/solo have real audible consequences via
// mixerAudio.svelte.ts + audioEngine.ts's per-track Web Audio graph.

// Joins with ChannelData.mixerTrack (see ChannelRow.svelte's "MIXER TRACK"
// ScrollField) — audioEngine resolves a triggered sample's destination by
// looking up this id among mixerStore.channels, falling back straight to
// master when no channel matches (see audioEngine.ts's resolveDestination).
export const MASTER_TRACK_ID = 0;

// A single insert-effect slot. UI-only bypass placeholder — there is no
// plugin/DSP system in this app, so `enabled` has no audio effect. A
// per-channel insert-slot list, just inert.
export interface InsertSlotData {
  id: number;
  enabled: boolean;
}

export interface MixerChannelData {
  id: number;
  name: string;      // user-editable; defaults to "Channel N" at creation, see makeChannel()
  volume: number;   // 0..2, unity ~0.8 — matches ChannelData.volume's convention
  pan: number;       // 0..1, 0.5 = center — matches ChannelData.pan's convention
  muted: boolean;
  soloed: boolean;
  // Every track this channel's output fans out to (multi-target — a channel
  // can feed several buses/sends at once, e.g. a drum bus AND a sidechain
  // send). MASTER_TRACK_ID by default. See toggleRoute()/canRoute() below.
  routeTo: number[];
  insertSlots: InsertSlotData[];
}

// Master has no pan/mute/solo/routing of its own — it IS the final bus, so
// it can never route (output) anywhere. It CAN still be the "active"
// selection (see selectedChannelId below), just with no arrow: selecting it
// only matters for viewing/editing its own insert-slot FX chain.
export interface MasterChannelData {
  id: typeof MASTER_TRACK_ID;
  volume: number;
  insertSlots: InsertSlotData[];
}

const INITIAL_CHANNEL_COUNT = 10;
const INSERT_SLOT_COUNT = 8; // keep compact

let nextChannelId = 1;
let nextInsertSlotId = 1;

function makeInsertSlots(): InsertSlotData[] {
  return Array.from({ length: INSERT_SLOT_COUNT }, () => ({ id: nextInsertSlotId++, enabled: false }));
}

// `position` drives the default name ("Channel N") and is deliberately the
// channel's place in the current list (this.channels.length + 1 at the call
// site), not its permanent unique `id` — so deleting every channel and
// adding a new one names it "Channel 1" again instead of continuing to
// climb through however high nextChannelId had already reached.
function makeChannel(id: number, position: number): MixerChannelData {
  return {
    id,
    name: `Channel ${position}`,
    volume: 0.8,
    pan: 0.5,
    muted: false,
    soloed: false,
    routeTo: [MASTER_TRACK_ID],
    insertSlots: makeInsertSlots(),
  };
}

class MixerStore {
  master = $state<MasterChannelData>({
    id: MASTER_TRACK_ID,
    volume: 0.8,
    insertSlots: makeInsertSlots(),
  });

  channels = $state<MixerChannelData[]>(
    Array.from({ length: INITIAL_CHANNEL_COUNT }, (_, i) => makeChannel(nextChannelId++, i + 1)),
  );

  // The strip currently "active" (Mixer.svelte / MixerChannelStrip.svelte /
  // MixerFxRack.svelte): drives both the click-to-route UI (the active
  // channel shows an arrow instead of a routing dot, and every other
  // strip's dot reflects whether THIS channel routes to it) and which
  // strip's insert-slot FX chain the FX rack displays. Defaults to the
  // first channel ("channel 1"); null only when there are no channels at
  // all. Can be MASTER_TRACK_ID — Master is selectable (for its own FX
  // chain and to highlight it), it just never gets a routing arrow, since
  // it has nowhere to output to.
  selectedChannelId = $state<number | null>(this.channels[0]?.id ?? null);

  // True while the selected channel's arrow has been clicked — the next
  // click on another strip's dot toggles a route to/from it. Stays true
  // across multiple dot clicks so several targets can be wired up in one
  // go (sidechaining to more than one bus); clicking the arrow again, or
  // selecting a different channel, disarms it.
  routingArmed = $state(false);

  // Scoped to `channels` only (excludes Master) — this is what the
  // click-to-route UI reads, since Master can never be the *source* of a
  // route. Use selectedEntity below for anything that also needs to work
  // when Master is the current selection (e.g. the FX rack).
  get selectedChannel(): MixerChannelData | undefined {
    return this.channels.find(c => c.id === this.selectedChannelId);
  }

  // The selected strip's own data, channel OR Master — what the FX rack
  // reads to show the right insert-slot chain regardless of which is active.
  get selectedEntity(): MixerChannelData | MasterChannelData | undefined {
    if (this.selectedChannelId === MASTER_TRACK_ID) return this.master;
    return this.selectedChannel;
  }

  selectChannel(id: number) {
    this.selectedChannelId = id;
    this.routingArmed = false;
  }

  toggleRoutingArmed() {
    // Master has no outgoing routes to arm — guarded here too even though
    // the UI never renders an arrow for it, so this stays safe to call
    // unconditionally.
    if (this.selectedChannelId === null || this.selectedChannelId === MASTER_TRACK_ID) return;
    this.routingArmed = !this.routingArmed;
  }

  addChannel(): MixerChannelData {
    const ch = makeChannel(nextChannelId++, this.channels.length + 1);
    this.channels.push(ch);
    if (this.selectedChannelId === null) this.selectChannel(ch.id);
    return ch;
  }

  // The only way to prune a channel now that cable-connect no longer
  // auto-grows the list. Any other channel routed into the removed one has
  // that edge dropped — falling back to [MASTER_TRACK_ID] if that leaves it
  // with no routes at all, so nothing goes silently unrouted.
  removeChannel(id: number) {
    this.channels = this.channels.filter(c => c.id !== id);
    for (const c of this.channels) {
      if (c.routeTo.includes(id)) {
        c.routeTo = c.routeTo.filter(t => t !== id);
        if (c.routeTo.length === 0) c.routeTo.push(MASTER_TRACK_ID);
      }
    }
    if (this.selectedChannelId === id) {
      this.selectedChannelId = this.channels[0]?.id ?? null;
      this.routingArmed = false;
    }
  }

  // Would adding a fromId -> targetId edge create a routing cycle? Walks
  // forward from targetId through existing .routeTo edges (a general graph
  // now, not a single chain) — if fromId is ever reached, the new edge
  // would loop back on itself.
  private canRoute(fromId: number, targetId: number): boolean {
    if (fromId === targetId) return false;
    const seen = new Set<number>();
    const stack = [targetId];
    while (stack.length) {
      const cur = stack.pop()!;
      if (cur === fromId) return false;
      if (seen.has(cur)) continue;
      seen.add(cur);
      if (cur === MASTER_TRACK_ID) continue; // master has no outgoing edges — dead end
      const ch = this.channels.find(c => c.id === cur);
      if (!ch) continue; // dangling id — treat as terminating
      for (const next of ch.routeTo) stack.push(next);
    }
    return true;
  }

  // Toggles a single edge in the routing graph: removes it if already
  // present, otherwise validates (self-route + cycle check) and adds it.
  // Returns whether the resulting state has the edge or not, so a caller
  // driving a UI dot's color doesn't need to re-derive it separately.
  toggleRoute(fromId: number, targetId: number): boolean {
    const channel = this.channels.find(c => c.id === fromId);
    if (!channel) return false;
    if (channel.routeTo.includes(targetId)) {
      channel.routeTo = channel.routeTo.filter(t => t !== targetId);
      return false;
    }
    if (!this.canRoute(fromId, targetId)) return false;
    channel.routeTo = [...channel.routeTo, targetId];
    return true;
  }

  // Standard multi-solo mixer semantics: soloing any channel silences every
  // non-soloed channel; an explicit mute always wins regardless of solo.
  get anySoloed(): boolean {
    return this.channels.some(c => c.soloed);
  }

  isAudible(channel: MixerChannelData): boolean {
    return !channel.muted && (!this.anySoloed || channel.soloed);
  }
}

export const mixerStore = new MixerStore();
