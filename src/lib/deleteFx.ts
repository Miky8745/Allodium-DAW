// Deletion effect shared by Piano Roll notes and Playlist placements: the
// deleted element fades out (FADE_MS) while a red outline of the same shape
// expands from its center and fades (RING_MS), both starting together.
//
// Used as a Svelte `out:` transition on keyed {#each} items. An element can
// leave its list for reasons other than deletion (a placement moved to
// another track, a pattern switch, undo), so the effect only plays for
// elements explicitly marked right before they're removed:
//
//   markDeleted(key);          // then remove the item from its array
//   <div data-del-key={key} out:deleteOut>
//
// Anything removed without a mark disappears instantly, as before.

const FADE_MS = 240;
const RING_MS = 750;
const RING_SCALE = 1.07;
const RING_COLOR = 'rgba(240, 92, 92, 0.95)';

const pending = new Set<string>();

export function markDeleted(key: string) {
  pending.add(key);
  // A mark whose element never actually leaves (e.g. the delete was a
  // no-op) mustn't fire later on an unrelated removal.
  setTimeout(() => pending.delete(key), 1000);
}

// The ring is a plain DOM node (not reactive state) dropped next to the
// element in its offset parent, animated with the Web Animations API and
// removed when done — it has to outlive the element's own transition.
function spawnRing(node: HTMLElement) {
  const parent = node.offsetParent as HTMLElement | null;
  if (!parent) return;
  const cs = getComputedStyle(node);
  const ring = document.createElement('div');
  Object.assign(ring.style, {
    position: 'absolute',
    left: `${node.offsetLeft}px`,
    top: `${node.offsetTop}px`,
    width: `${node.offsetWidth}px`,
    height: `${node.offsetHeight}px`,
    boxSizing: 'border-box',
    border: `2px solid ${RING_COLOR}`,
    borderRadius: cs.borderRadius,
    pointerEvents: 'none',
    zIndex: '4',
  } satisfies Partial<CSSStyleDeclaration>);
  parent.appendChild(ring);
  const anim = ring.animate(
    [
      { transform: 'scale(1)', opacity: 1 },
      { transform: `scale(${RING_SCALE})`, opacity: 0 },
    ],
    { duration: RING_MS, easing: 'cubic-bezier(0.22, 1, 0.36, 1)' }, // ease-out
  );
  anim.onfinish = () => ring.remove();
  anim.oncancel = () => ring.remove();
}

export function deleteOut(node: HTMLElement) {
  const key = node.dataset.delKey;
  if (!key || !pending.delete(key)) return { duration: 0 };
  spawnRing(node);
  return {
    duration: FADE_MS,
    css: (t: number) => `opacity: ${t}; pointer-events: none;`,
  };
}
