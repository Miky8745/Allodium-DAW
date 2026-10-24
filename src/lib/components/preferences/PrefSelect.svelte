<script lang="ts" generics="T extends string | number">
  // Native <select> styled for the dusk theme. Generic over the option
  // value type so a numeric pref (e.g. sample rate) stays a number.
  let {
    value = $bindable(),
    options,
    width = 140,
  }: {
    value: T;
    options: { value: T; label: string }[];
    width?: number;
  } = $props();

  function onChange(e: Event) {
    const idx = (e.currentTarget as HTMLSelectElement).selectedIndex;
    value = options[idx].value;
  }
</script>

<select class="pref-select" style="width:{width}px;" onchange={onChange}>
  {#each options as opt (opt.value)}
    <option value={String(opt.value)} selected={opt.value === value}>{opt.label}</option>
  {/each}
</select>

<style>
  .pref-select {
    appearance: none;
    padding: 4px 24px 4px 9px;
    background: #171a23 url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='8' height='5'%3E%3Cpath d='M0 0l4 5 4-5z' fill='%237e849a'/%3E%3C/svg%3E") no-repeat right 9px center;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 5px;
    color: var(--main-text);
    font-family: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .pref-select:hover { border-color: rgba(255, 255, 255, 0.2); }
  .pref-select:focus { border-color: var(--accent); }
  .pref-select option { background: var(--main-bg); }
</style>
