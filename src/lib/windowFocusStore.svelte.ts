// Ephemeral, session-only UI state — not part of projectSerializer.ts's
// saved/undo-tracked state, same treatment as channelStore.selectedChannelId.
// Tracks which floating window (Channel Rack / Piano Roll / Playlist,
// identified by a stable string id) was interacted with most recently, so
// FloatingWindow.svelte can render them in that order instead of fixed DOM
// mount order.
class WindowFocusStore {
  order = $state<string[]>([]); // back-to-front; last element = frontmost

  focus(id: string) {
    this.order = [...this.order.filter((w) => w !== id), id];
  }

  isFrontmost(id: string): boolean {
    return this.order.length > 0 && this.order[this.order.length - 1] === id;
  }

  zIndexOf(id: string): number {
    const idx = this.order.indexOf(id);
    return idx === -1 ? 100 : 101 + idx;
  }
}

export const windowFocus = new WindowFocusStore();
