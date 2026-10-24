// Svelte action that moves a node onto <body> so it escapes an ancestor's
// overflow:hidden/auto clipping (position:fixed alone doesn't help, since
// clipping applies to descendants regardless of their own positioning
// scheme). Shared by every small floating overlay that needs to render
// outside its normal DOM position — DragTooltip, ChannelRow's own context
// menu, KnobContextMenu, etc.
export function portal(node: HTMLElement) {
  document.body.appendChild(node);
  return { destroy: () => node.remove() };
}
