import { hoverInfo } from './hoverInfoStore.svelte';

// Svelte action that reports the hovered element's name/value into
// hoverInfoStore.svelte.ts, driving the Toolbar's info panel. Used on
// Dial/Knob/ScrollField/Slider (their own `name` prop) and directly on the
// Toolbar's own buttons/toggles/dropdowns.
//
// `params.value` is read fresh on every mouseenter, and Svelte re-invokes
// this action's `update()` whenever the reactive expression passed as
// `params` changes (the normal action lifecycle) — so a value that changes
// while still hovered (e.g. dragging a Dial without moving the mouse off
// it) keeps the info panel in sync, not just a one-time snapshot taken at
// mouseenter.
//
// mouseleave deliberately does NOT clear the store — the info panel is
// meant to keep showing the last-hovered control's name/value rather than
// reverting to an idle state, so leaving just stops this instance from
// writing any further updates (via the `hovering` flag below) and leaves
// whatever it last wrote in place for the next thing to overwrite.
//
// mouseenter/mouseleave (not mouseover/mouseout) are used because they
// don't bubble — each element fires its own pair exactly once per
// enter/exit of its own box, so nested hoverInfo-using elements wouldn't
// interfere with each other (not actually relied on today, since this is
// only used on leaf-level controls, but it's the correct choice either way).
export interface HoverInfoParams {
  name: string;
  value?: string | null;
}

export function hoverInfoAction(node: HTMLElement, params: HoverInfoParams) {
  let current = params;
  let hovering = false;

  function apply() {
    if (!current.name) return;
    hoverInfo.set(current.name, current.value ?? null);
  }

  function onEnter() {
    hovering = true;
    apply();
  }

  function onLeave() {
    hovering = false;
  }

  node.addEventListener('mouseenter', onEnter);
  node.addEventListener('mouseleave', onLeave);

  return {
    update(newParams: HoverInfoParams) {
      current = newParams;
      if (hovering) apply();
    },
    destroy() {
      node.removeEventListener('mouseenter', onEnter);
      node.removeEventListener('mouseleave', onLeave);
      hovering = false;
    },
  };
}
