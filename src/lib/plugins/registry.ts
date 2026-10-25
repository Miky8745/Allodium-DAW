import type { ChannelData } from '$lib/types';
import { channelStore } from '$lib/channelStore.svelte';
import { defaultInstrument } from '$lib/instrument/types';
import { instrumentWindow } from '$lib/instrument/instrumentWindow.svelte';

// Plugins that can be assigned to a channel's sample field (right-click the
// sample field → plugin menu; double-click opens the assigned one). Built-in
// for now — the shape is meant to also fit external plugins later.
// A channel stores the plugin's `id` in ChannelData.plugin ('' = none).
export interface PluginDef {
  id: string;
  name: string;
  kind: 'instrument';
  // Turns the plugin on for a freshly assigned channel.
  assign: (ch: ChannelData) => void;
  // Clears the plugin's per-channel state when it's removed.
  remove: (ch: ChannelData) => void;
  // Shows the plugin's window for a channel.
  open: (channelId: number) => void;
}

export const BLOOM_ID = 'bloom';

export const PLUGINS: PluginDef[] = [
  {
    id: BLOOM_ID,
    name: 'Bloom',
    kind: 'instrument',
    assign: ch => { ch.instrument.enabled = true; },
    remove: ch => { ch.instrument = defaultInstrument(); },
    open: id => instrumentWindow.open(id),
  },
];

export function pluginById(id: string): PluginDef | null {
  return PLUGINS.find(p => p.id === id) ?? null;
}

export function channelPlugin(ch: Pick<ChannelData, 'plugin'>): PluginDef | null {
  return ch.plugin ? pluginById(ch.plugin) : null;
}

// Assigns (replacing any other) plugin to a channel. Looks the channel up by
// id so the write goes through the store's reactive proxy.
export function assignPlugin(channelId: number, pluginId: string): void {
  const ch = channelStore.channels.find(c => c.id === channelId);
  const plugin = pluginById(pluginId);
  if (!ch || !plugin || ch.plugin === pluginId) return;
  channelPlugin(ch)?.remove(ch);
  ch.plugin = pluginId;
  plugin.assign(ch);
}

export function removePlugin(channelId: number): void {
  const ch = channelStore.channels.find(c => c.id === channelId);
  if (!ch) return;
  channelPlugin(ch)?.remove(ch);
  ch.plugin = '';
}
