<script lang="ts">
  let {
    projects,
    onNewProject,
    onOpenProject,
  }: {
    projects: string[];
    onNewProject: () => void;
    onOpenProject: (name: string) => void;
  } = $props();
</script>

<div class="projects-panel">
  <div class="panel-header">
    <h1 class="panel-title">Projects</h1>
    <div class="action-group">
      <button class="btn btn-primary" aria-label="New Project" onclick={onNewProject}>
        <svg width="13" height="13" viewBox="0 0 13 13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true">
          <path d="M6.5 1v11M1 6.5h11"/>
        </svg>
        New Project
      </button>
      <button class="btn btn-secondary" aria-label="Open project folder">
        <svg width="13" height="13" viewBox="0 0 13 13" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M1 3.5h4l1.5 1.5H12v6H1V3.5z"/>
        </svg>
        Open
      </button>
      <button class="btn btn-secondary" aria-label="Get from version control">
        <svg width="13" height="13" viewBox="0 0 13 13" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <circle cx="3.5" cy="3" r="1.5"/>
          <circle cx="9.5" cy="3" r="1.5"/>
          <circle cx="6.5" cy="10" r="1.5"/>
          <path d="M3.5 4.5v2.5L6.5 8.5M9.5 4.5v2.5L6.5 8.5"/>
        </svg>
        Get from VCS
      </button>
    </div>
  </div>

  <div class="search-bar">
    <svg class="search-icon" width="13" height="13" viewBox="0 0 13 13" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" aria-hidden="true">
      <circle cx="5.5" cy="5.5" r="4"/>
      <path d="M8.5 8.5l3 3"/>
    </svg>
    <input type="search" class="search-input" placeholder="Search projects…" aria-label="Search projects" />
  </div>

  {#if projects.length === 0}
    <div class="empty-state" aria-label="No recent projects">
      <svg class="empty-illustration" width="88" height="88" viewBox="0 0 88 88" fill="none" aria-hidden="true">
        <circle cx="44" cy="44" r="42" fill="currentColor" opacity="0.04"/>
        <circle cx="44" cy="44" r="30" fill="currentColor" opacity="0.04"/>
        <line x1="44" y1="70" x2="44" y2="38" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"/>
        <path d="M44 52C44 52 32 50 27 42C27 42 36 37 44 44" fill="currentColor" opacity="0.2" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
        <path d="M44 45C44 45 56 40 61 32C61 32 52 28 44 37" fill="currentColor" opacity="0.2" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
        <circle cx="36" cy="72" r="1.5" fill="currentColor" opacity="0.2"/>
        <circle cx="44" cy="74" r="1.5" fill="currentColor" opacity="0.2"/>
        <circle cx="52" cy="72" r="1.5" fill="currentColor" opacity="0.2"/>
      </svg>
      <h2 class="empty-title">No projects yet</h2>
      <p class="empty-sub">Create a new project to get started.</p>
    </div>
  {:else}
    <ul class="project-list" role="list">
      {#each projects as name}
        <li>
          <button class="project-item" onclick={() => onOpenProject(name)}>
            <svg class="project-icon" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M2 4.5h5l1.5 1.5H14v7H2V4.5z"/>
            </svg>
            <span class="project-name">{name}</span>
            <svg class="project-arrow" width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M4 2.5l4 3.5-4 3.5"/>
            </svg>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .projects-panel { display: flex; flex-direction: column; height: 100%; }

  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 20px;
    border-bottom: 1px solid var(--main-border);
    gap: 16px;
  }

  .panel-title {
    font-family: 'Playfair Display', Georgia, 'Times New Roman', serif;
    font-size: 17px;
    font-weight: 600;
    color: var(--main-text);
    white-space: nowrap;
  }

  .action-group { display: flex; align-items: center; gap: 6px; }

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
    white-space: nowrap;
    line-height: 1;
  }

  .btn:active { transform: translateY(1px); }

  .btn-primary { background: var(--accent); color: var(--accent-text); border-color: var(--accent); }
  .btn-primary:hover { background: var(--accent-hover); border-color: var(--accent-hover); }

  .btn-secondary {
    background: var(--btn-bg);
    color: var(--btn-text);
    border-color: var(--btn-border);
    box-shadow: var(--shadow-sm);
  }
  .btn-secondary:hover { background: var(--btn-hover); }

  .search-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 12px 20px 0;
    padding: 7px 12px;
    background: var(--search-bg);
    border: 1px solid var(--main-border);
    border-radius: 10px;
    box-shadow: var(--shadow-sm);
  }

  .search-icon { color: var(--main-text-muted); flex-shrink: 0; }

  .search-input {
    flex: 1;
    border: none;
    background: transparent;
    font-family: inherit;
    font-size: 13px;
    color: var(--main-text);
    outline: none;
  }

  .search-input::placeholder { color: var(--main-text-muted); }
  .search-input::-webkit-search-cancel-button { opacity: 0.4; cursor: pointer; }

  .empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 48px 32px;
    text-align: center;
    color: var(--main-text-muted);
  }

  .empty-illustration { margin-bottom: 8px; color: var(--main-text-muted); }

  .empty-title {
    font-family: 'Playfair Display', Georgia, 'Times New Roman', serif;
    font-size: 15px;
    font-weight: 600;
    color: var(--main-text);
    opacity: 0.5;
    margin-bottom: 2px;
  }

  .empty-sub { font-size: 13px; line-height: 1.55; max-width: 260px; color: var(--main-text-muted); }

  .project-list { list-style: none; padding: 10px 16px; overflow-y: auto; flex: 1; }

  .project-item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 9px 12px;
    border: none;
    background: transparent;
    border-radius: 10px;
    font-family: inherit;
    font-size: 13.5px;
    color: var(--main-text);
    cursor: pointer;
    text-align: left;
    transition: background 0.1s;
  }

  .project-item:hover { background: var(--btn-hover); }

  .project-icon { flex-shrink: 0; color: var(--accent); opacity: 0.8; }

  .project-name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .project-arrow { flex-shrink: 0; color: var(--main-text-muted); opacity: 0; transition: opacity 0.1s; }
  .project-item:hover .project-arrow { opacity: 1; }
</style>
