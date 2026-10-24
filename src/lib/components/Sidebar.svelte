<script lang="ts">
  import { preferencesUi } from '$lib/preferencesUi.svelte';
  type NavKey = 'projects' | 'customize' | 'plugins' | 'learn';
  let { activeNav = $bindable() }: { activeNav: NavKey } = $props();
</script>

<aside class="sidebar">
  <div class="sidebar-header">
    <div class="logo">
      <svg class="logo-leaf" width="30" height="30" viewBox="0 0 30 30" fill="none" aria-hidden="true">
        <path d="M15 3C15 3 5 9.5 5 19C5 24.523 9.2 26.5 15 26.5C20.8 26.5 25 24.523 25 19C25 9.5 15 3 15 3Z" fill="#e6b07f"/>
        <line x1="15" y1="26.5" x2="15" y2="13" stroke="#b58a63" stroke-width="1.6" stroke-linecap="round"/>
        <line x1="15" y1="20" x2="19.5" y2="15.5" stroke="#b58a63" stroke-width="1.6" stroke-linecap="round"/>
        <line x1="15" y1="16.5" x2="10.5" y2="13" stroke="#b58a63" stroke-width="1.6" stroke-linecap="round"/>
      </svg>
      <div class="logo-text">
        <span class="logo-name">Allodium</span>
        <span class="logo-version">v 0.1.0</span>
      </div>
    </div>
  </div>

  <nav class="sidebar-nav" aria-label="Main navigation">
    <button class="nav-item" class:active={activeNav === 'projects'} onclick={() => activeNav = 'projects'}>
      <svg width="15" height="15" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <rect x="1" y="4.5" width="13" height="9" rx="1.2"/>
        <path d="M1 7.5h13"/>
        <path d="M5 4.5V3a.5.5 0 0 1 .5-.5h4a.5.5 0 0 1 .5.5v1.5"/>
      </svg>
      <span>Projects</span>
    </button>

    <button class="nav-item" class:active={activeNav === 'customize'} onclick={() => activeNav = 'customize'}>
      <svg width="15" height="15" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M7.5 1.5a6 6 0 1 0 0 12c.83 0 1.25-.37 1.25-.87 0-.22-.08-.4-.17-.58-.09-.17-.17-.36-.17-.58 0-.5.41-.87.87-.87h1a3.29 3.29 0 0 0 3.27-3.5A6 6 0 0 0 7.5 1.5z"/>
        <circle cx="4.5" cy="6" r=".75" fill="currentColor" stroke="none"/>
        <circle cx="7.5" cy="3.75" r=".75" fill="currentColor" stroke="none"/>
        <circle cx="10.5" cy="6" r=".75" fill="currentColor" stroke="none"/>
      </svg>
      <span>Customize</span>
    </button>

    <button class="nav-item" class:active={activeNav === 'plugins'} onclick={() => activeNav = 'plugins'}>
      <svg width="15" height="15" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <rect x="1.5" y="1.5" width="5" height="5" rx="1"/>
        <rect x="8.5" y="1.5" width="5" height="5" rx="1"/>
        <rect x="1.5" y="8.5" width="5" height="5" rx="1"/>
        <path d="M11 8.5v4.5M8.5 11H13"/>
      </svg>
      <span>Plugins</span>
    </button>

    <button class="nav-item" class:active={activeNav === 'learn'} onclick={() => activeNav = 'learn'}>
      <svg width="15" height="15" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M2 2.5h9.5a1 1 0 0 1 1 1v8a1 1 0 0 1-1 1H2a1 1 0 0 1-1-1v-8a1 1 0 0 1 1-1z"/>
        <path d="M7.5 2.5v10"/>
        <path d="M4 5.5h2M4 7.5h2"/>
      </svg>
      <span>Learn</span>
    </button>
  </nav>

  <div class="sidebar-footer">
    <button class="footer-btn" title="Settings (Ctrl+,)" aria-label="Settings" onclick={() => preferencesUi.show()}>
      <svg width="15" height="15" viewBox="0 0 15 15" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" aria-hidden="true">
        <circle cx="7.5" cy="7.5" r="2"/>
        <path d="M7.5 1v1.2M7.5 12.8V14M1 7.5h1.2M12.8 7.5H14M2.93 2.93l.85.85M11.22 11.22l.85.85M2.93 12.07l.85-.85M11.22 3.78l.85-.85"/>
      </svg>
      <span>Settings</span>
    </button>
  </div>
</aside>

<style>
  .sidebar {
    width: 220px;
    min-width: 220px;
    background: var(--sidebar-bg);
    display: flex;
    flex-direction: column;
    border-right: 1px solid var(--sidebar-border);
    user-select: none;
  }

  .sidebar-header {
    padding: 20px 16px 18px;
    border-bottom: 1px solid var(--sidebar-border);
  }

  .logo { display: flex; align-items: center; gap: 10px; }
  .logo-leaf { flex-shrink: 0; }
  .logo-text { display: flex; flex-direction: column; gap: 1px; }

  .logo-name {
    font-family: 'Playfair Display', Georgia, 'Times New Roman', serif;
    font-size: 15px;
    font-weight: 600;
    color: #c9cacd;
    letter-spacing: -0.01em;
    line-height: 1.2;
  }

  .logo-version {
    font-size: 10.5px;
    color: var(--sidebar-text);
    letter-spacing: 0.04em;
    opacity: 0.8;
  }

  .sidebar-nav {
    flex: 1;
    padding: 8px 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .nav-item {
    position: relative;
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 8px 16px;
    border: none;
    background: transparent;
    color: var(--sidebar-text);
    font-family: inherit;
    font-size: 13.5px;
    font-weight: 400;
    cursor: pointer;
    text-align: left;
    width: 100%;
    transition: background 0.12s ease, color 0.12s ease;
  }

  .nav-item:hover { background: var(--sidebar-hover); color: #a9abae; }

  .nav-item.active {
    background: var(--sidebar-active-bg);
    color: var(--sidebar-active-text);
    font-weight: 500;
  }

  .nav-item.active::before {
    content: '';
    position: absolute;
    left: 0; top: 5px; bottom: 5px;
    width: 3px;
    background: var(--accent);
    border-radius: 0 4px 4px 0;
  }

  .nav-item svg { flex-shrink: 0; opacity: 0.7; transition: opacity 0.12s; }
  .nav-item:hover svg, .nav-item.active svg { opacity: 1; }

  .sidebar-footer { padding: 10px; border-top: 1px solid var(--sidebar-border); }

  .footer-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 8px;
    border: none;
    background: transparent;
    color: var(--sidebar-text);
    font-family: inherit;
    font-size: 13px;
    cursor: pointer;
    border-radius: 9px;
    width: 100%;
    transition: background 0.12s, color 0.12s;
  }

  .footer-btn:hover { background: var(--sidebar-hover); color: #a9abae; }
</style>
