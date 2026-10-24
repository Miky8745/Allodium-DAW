// Ephemeral open/closed + active-tab state for the Preferences modal
// (opened from Edit > Preferences…, the home Sidebar's Settings button,
// or Ctrl+,).
export type PreferencesTab = 'general' | 'midi';

class PreferencesUi {
  open = $state(false);
  tab = $state<PreferencesTab>('general');

  show(tab?: PreferencesTab) {
    if (tab) this.tab = tab;
    this.open = true;
  }

  close() {
    this.open = false;
  }
}

export const preferencesUi = new PreferencesUi();
