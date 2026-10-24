<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  let {
    show = $bindable(),
    onCreated,
  }: {
    show: boolean;
    onCreated: (name: string) => void;
  } = $props();

  let projectName = $state('');
  let creating = $state(false);
  let modalError = $state('');
  let inputEl = $state<HTMLInputElement | null>(null);

  $effect(() => {
    if (show) {
      projectName = '';
      modalError = '';
      setTimeout(() => inputEl?.focus(), 30);
    }
  });

  function close() {
    if (!creating) show = false;
  }

  async function handleCreate() {
    const name = projectName.trim();
    if (!name) { modalError = 'Please enter a project name.'; return; }
    creating = true;
    modalError = '';
    try {
      await invoke('create_project', { name });
      show = false;
      onCreated(name);
    } catch (err) {
      modalError = String(err);
    } finally {
      creating = false;
    }
  }
</script>

{#if show}
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="modal-backdrop" onclick={close} role="presentation">
  <div
    class="modal"
    onclick={(e) => e.stopPropagation()}
    role="dialog"
    aria-modal="true"
    aria-labelledby="modal-title"
    tabindex="-1"
  >
    <h2 id="modal-title" class="modal-title">New Project</h2>

    <div class="modal-field">
      <label for="project-name" class="modal-label">Project name</label>
      <input
        id="project-name"
        type="text"
        class="modal-input"
        bind:this={inputEl}
        bind:value={projectName}
        placeholder="My Project"
        autocomplete="off"
        onkeydown={(e) => { if (e.key === 'Enter') handleCreate(); if (e.key === 'Escape') close(); }}
      />
      {#if modalError}
        <p class="modal-error">{modalError}</p>
      {/if}
    </div>

    <div class="modal-actions">
      <button class="btn btn-secondary" onclick={close} disabled={creating}>Cancel</button>
      <button class="btn btn-primary" onclick={handleCreate} disabled={creating}>
        {creating ? 'Creating…' : 'Create'}
      </button>
    </div>
  </div>
</div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    backdrop-filter: blur(2px);
  }

  .modal {
    background: var(--main-bg);
    border: 1px solid var(--main-border);
    border-radius: 14px;
    padding: 24px;
    width: 360px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.22);
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .modal-title {
    font-family: 'Playfair Display', Georgia, serif;
    font-size: 16px;
    font-weight: 600;
    color: var(--main-text);
  }

  .modal-field { display: flex; flex-direction: column; gap: 6px; }

  .modal-label { font-size: 12px; font-weight: 500; color: var(--main-text-muted); letter-spacing: 0.03em; }

  .modal-input {
    padding: 8px 12px;
    background: var(--search-bg);
    border: 1px solid var(--main-border);
    border-radius: 10px;
    font-family: inherit;
    font-size: 13.5px;
    color: var(--main-text);
    outline: none;
    transition: border-color 0.12s;
  }

  .modal-input:focus { border-color: var(--accent); }

  .modal-error { font-size: 12px; color: #b7493e; }

  .modal-actions { display: flex; justify-content: flex-end; gap: 8px; }

  .btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 6px 12px;
    border-radius: 9px;
    font-family: inherit;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.12s, border-color 0.12s;
    border: 1px solid transparent;
    line-height: 1;
  }

  .btn:active { transform: translateY(1px); }
  .btn:disabled { opacity: 0.6; cursor: not-allowed; }

  .btn-primary { background: var(--accent); color: var(--accent-text); border-color: var(--accent); }
  .btn-primary:hover:not(:disabled) { background: var(--accent-hover); border-color: var(--accent-hover); }

  .btn-secondary { background: var(--btn-bg); color: var(--btn-text); border-color: var(--btn-border); box-shadow: var(--shadow-sm); }
  .btn-secondary:hover:not(:disabled) { background: var(--btn-hover); }
</style>
