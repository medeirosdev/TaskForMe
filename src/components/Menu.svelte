<script lang="ts" module>
  import type { Component } from "svelte";
  export type MenuItem =
    | { label: string; icon?: Component<any>; danger?: boolean; disabled?: boolean; onclick: () => void }
    | "sep";
</script>

<script lang="ts">
  import Popover from "./Popover.svelte";

  let {
    anchor,
    open = $bindable(false),
    items,
    align = "end",
  }: { anchor: HTMLElement | undefined; open?: boolean; items: MenuItem[]; align?: "start" | "end" } = $props();
</script>

<Popover {anchor} bind:open {align}>
  <div class="menu" role="menu">
    {#each items as item, i (i)}
      {#if item === "sep"}
        <div class="sep"></div>
      {:else}
        <button type="button"
          class="item"
          class:danger={item.danger}
          role="menuitem"
          disabled={item.disabled}
          onclick={() => {
            open = false;
            item.onclick();
          }}
        >
          {#if item.icon}<item.icon size={15} strokeWidth={1.75} />{/if}
          <span>{item.label}</span>
        </button>
      {/if}
    {/each}
  </div>
</Popover>

<style>
  .menu {
    display: flex;
    flex-direction: column;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 9px;
    height: 32px;
    padding: 0 10px;
    border-radius: var(--r-sm);
    font-size: 13px;
    color: var(--ink-muted);
    text-align: left;
  }
  .item :global(svg) {
    color: var(--ink-subtle);
  }
  .item:hover:not(:disabled) {
    background: var(--hover);
    color: var(--ink);
  }
  .item.danger,
  .item.danger :global(svg) {
    color: var(--danger);
  }
  .item.danger:hover {
    background: var(--danger-soft);
  }
  .sep {
    height: 1px;
    margin: 4px 6px;
    background: var(--hairline);
  }
</style>
