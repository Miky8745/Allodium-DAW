import { channelStore } from '$lib/channelStore.svelte';
import { windowFocus } from '$lib/windowFocusStore.svelte';

export const INSTRUMENT_WINDOW_ID = 'instrument';

// Open/closed state of the Bloom instrument window (ephemeral UI state, like
// the other windows' show flags). Lives in a store rather than +page.svelte
// so the plugin registry (sample-field double-click / plugin menu) can open
// it directly.
class InstrumentWindowStore {
  show = $state(false);

  // Shows the window for `channelId` and raises it.
  open(channelId: number) {
    channelStore.selectedChannelId = channelId;
    this.show = true;
    windowFocus.focus(INSTRUMENT_WINDOW_ID);
  }
}

export const instrumentWindow = new InstrumentWindowStore();
