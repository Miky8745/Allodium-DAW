<script lang="ts">
  import { historyStore } from '$lib/historyStore.svelte';
  import { saveProject } from '$lib/projectSerializer';

  function handleKeydown(e: KeyboardEvent) {
    const target = e.target as HTMLElement | null;
    if (target?.tagName === 'INPUT' || target?.tagName === 'TEXTAREA' || target?.isContentEditable) return;

    const mod = e.ctrlKey || e.metaKey;
    if (!mod) return;

    const key = e.key.toLowerCase();
    if (key === 'z' && !e.shiftKey) {
      e.preventDefault();
      historyStore.undo();
    } else if (key === 'y' || (key === 'z' && e.shiftKey)) {
      e.preventDefault();
      historyStore.redo();
    } else if (key === 's') {
      e.preventDefault();
      saveProject(historyStore.projectName);
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />
