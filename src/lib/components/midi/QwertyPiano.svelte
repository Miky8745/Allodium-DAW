<script lang="ts">
  // No markup — window-level key listeners for the Toolbar's "Typing
  // keyboard to piano" toggle (see qwertyKeyboard.svelte.ts). Mounted once
  // in the project view. Modifier combos pass through untouched so
  // Ctrl+Z/C/V etc. keep working while the toggle is on.
  import { qwertyKeyboard } from '$lib/midi/qwertyKeyboard.svelte';
  import { preferencesUi } from '$lib/preferencesUi.svelte';

  function isTyping(target: EventTarget | null) {
    const el = target as HTMLElement | null;
    return el?.tagName === 'INPUT' || el?.tagName === 'TEXTAREA' || el?.tagName === 'SELECT' || !!el?.isContentEditable;
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.ctrlKey || e.metaKey || e.altKey || isTyping(e.target)) return;
    if (preferencesUi.open) return;
    if (qwertyKeyboard.keyDown(e)) e.preventDefault();
  }

  function onKeyup(e: KeyboardEvent) {
    qwertyKeyboard.keyUp(e);
  }
</script>

<svelte:window onkeydown={onKeydown} onkeyup={onKeyup} onblur={() => qwertyKeyboard.releaseAll()} />
