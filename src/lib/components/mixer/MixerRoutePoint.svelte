<script lang="ts">
  // The click-to-route control at the bottom of every mixer strip. The
  // selected ("active") channel shows an arrow — clicking it arms routing
  // mode. Every other strip (including Master) shows a dot instead — while
  // armed, clicking a dot toggles a route from the selected channel to that
  // strip, turning it green. See "Mixer" in CLAUDE.md for the full gesture.
  let {
    kind,
    active = false,
    disabled = false,
    onClick,
    title,
  }: {
    kind: 'arrow' | 'dot';
    active?: boolean;
    disabled?: boolean;
    onClick?: () => void;
    title?: string;
  } = $props();

  function handleClick(e: MouseEvent) {
    // Always stop propagation, even when disabled/no-op — a route point
    // sits inside a strip that itself selects on click, and a dot/arrow
    // click must never also re-trigger that (routing a channel shouldn't
    // change which channel is selected).
    e.stopPropagation();
    if (disabled) return;
    onClick?.();
  }
</script>

<button
  class="route-point route-point--{kind}"
  class:route-point--active={active}
  class:route-point--disabled={disabled}
  onclick={handleClick}
  {title}
  aria-pressed={active}
  aria-disabled={disabled}
>
  {#if kind === 'arrow'}
    <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
      <path d="M2 7h9M8 3.5 11.5 7 8 10.5" />
    </svg>
  {/if}
</button>

<style>
  .route-point {
    border: none;
    padding: 0;
    cursor: pointer;
    flex-shrink: 0;
  }

  .route-point--dot {
    width: 11px;
    height: 11px;
    border-radius: 50%;
    background: #43454d;
    transition: background 0.1s, box-shadow 0.1s;
  }

  .route-point--dot.route-point--active {
    background: var(--accent, #e6b07f);
    box-shadow: 0 0 4px rgba(230,176,127, 0.7);
  }

  .route-point--dot.route-point--disabled {
    cursor: default;
    opacity: 0.6;
  }

  .route-point--arrow {
    width: 22px;
    height: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 5px;
    color: rgba(255, 255, 255, 0.6);
    transition: color 0.1s, background 0.1s, border-color 0.1s;
  }

  .route-point--arrow svg {
    width: 11px;
    height: 11px;
  }

  .route-point--arrow:hover {
    color: rgba(255, 255, 255, 0.85);
  }

  .route-point--arrow.route-point--active {
    background: rgba(230,176,127, 0.18);
    border-color: var(--accent, #e6b07f);
    color: var(--accent, #e6b07f);
  }
</style>
