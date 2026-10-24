<script lang="ts">
  import type { MenuItem } from '$lib/types';
  import { portal } from '$lib/portal';

  let {
    menus,
  }: {
    menus: { name: string; items: MenuItem[] }[];
  } = $props();

  let openMenu = $state<string | null>(null);
  // Dropdown is portalled onto <body> (the Toolbar's overflow:hidden would
  // otherwise clip it), so it's positioned from the button's viewport rect.
  let dropdownPos = $state({ x: 0, y: 0 });

  function toggleMenu(e: MouseEvent, name: string) {
    e.stopPropagation();
    if (openMenu === name) { openMenu = null; return; }
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    dropdownPos = { x: r.left, y: r.bottom + 2 };
    openMenu = name;
  }

  $effect(() => {
    if (openMenu === null) return;
    const close = () => { openMenu = null; };
    document.addEventListener('click', close);
    return () => document.removeEventListener('click', close);
  });
</script>

<svg class="menu-logo" width="16" height="16" viewBox="0 0 30 30" fill="none" aria-hidden="true">
  <path d="M15 3C15 3 5 9.5 5 19C5 24.523 9.2 26.5 15 26.5C20.8 26.5 25 24.523 25 19C25 9.5 15 3 15 3Z" fill="#e6b07f"/>
  <line x1="15" y1="26.5" x2="15" y2="13" stroke="#b58a63" stroke-width="1.6" stroke-linecap="round"/>
  <line x1="15" y1="20" x2="19.5" y2="15.5" stroke="#b58a63" stroke-width="1.6" stroke-linecap="round"/>
  <line x1="15" y1="16.5" x2="10.5" y2="13" stroke="#b58a63" stroke-width="1.6" stroke-linecap="round"/>
</svg>

{#each menus as menu}
  <div class="menu-entry">
    <button
      class="menu-btn"
      class:menu-btn--open={openMenu === menu.name}
      onclick={(e) => toggleMenu(e, menu.name)}
    >{menu.name}</button>

    {#if openMenu === menu.name && menu.items.length > 0}
      <div
        use:portal
        class="menu-dropdown"
        style="left: {dropdownPos.x}px; top: {dropdownPos.y}px;"
        role="menu"
        tabindex="-1"
        onclick={(e) => e.stopPropagation()}
        onkeydown={(e) => e.stopPropagation()}
      >
        {#each menu.items as item}
          <button class="menu-item" onclick={() => { openMenu = null; item.action(); }}>
            {item.label}
          </button>
        {/each}
      </div>
    {/if}
  </div>
{/each}

<style>
  .menu-logo { margin-right: 6px; flex-shrink: 0; }

  .menu-entry { position: relative; }

  .menu-btn {
    background: transparent;
    border: none;
    color: var(--sidebar-text);
    font-family: inherit;
    font-size: 12.5px;
    padding: 3px 8px;
    border-radius: 7px;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
    line-height: 1;
  }

  .menu-btn:hover, .menu-btn--open {
    background: rgba(255,255,255,0.08);
    color: #c9cacd;
  }

  .menu-dropdown {
    position: fixed;
    min-width: 160px;
    background: var(--explorer-bg);
    border: 1px solid var(--explorer-border);
    border-radius: 9px;
    box-shadow: 0 6px 20px rgba(0,0,0,0.25);
    z-index: 10000; /* above every floating window and the file explorer */
    padding: 3px;
  }

  .menu-item {
    display: block;
    width: 100%;
    padding: 6px 10px;
    text-align: left;
    background: transparent;
    border: none;
    border-radius: 7px;
    color: var(--sidebar-text);
    font-family: inherit;
    font-size: 12.5px;
    cursor: pointer;
    transition: background 0.1s;
  }

  .menu-item:hover { background: rgba(255,255,255,0.06); color: #c9cacd; }
</style>
