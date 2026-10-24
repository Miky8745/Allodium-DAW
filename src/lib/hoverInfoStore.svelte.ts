// Svelte 5 class-based singleton (ephemeral, session-only — not saved/undo-
// tracked, same treatment as windowFocusStore/mixerStore) backing the
// Toolbar's info panel (Toolbar.svelte's `.tb-info` box). Whatever's
// currently under the mouse writes its own name/value here via the
// `hoverInfoAction` action (src/lib/hoverInfo.ts) — the panel keeps
// showing the last-hovered control's name/value after the mouse moves off
// rather than reverting to any idle placeholder, so there's no `clear()`
// here, only `set()`.
class HoverInfoStore {
  name  = $state<string | null>(null);
  value = $state<string | null>(null);

  set(name: string, value: string | null = null) {
    this.name = name;
    this.value = value;
  }
}

export const hoverInfo = new HoverInfoStore();
