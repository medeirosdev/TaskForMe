<script lang="ts">
  import Menu from "./Menu.svelte";
  import { ChevronRight, Ellipsis, Pencil, Plus, Trash } from "../lib/icons";
  import { api } from "../lib/api";
  import { app, deleteWithUndo } from "../lib/store.svelte";
  import type { Section } from "../lib/types";

  let {
    section,
    count,
    editing = $bindable(false),
    onadd,
    addLabel = "Adicionar tarefa",
  }: {
    section: Section;
    count: number;
    editing?: boolean;
    onadd: () => void;
    addLabel?: string;
  } = $props();

  let name = $state("");
  let menuAnchor: HTMLButtonElement | undefined = $state();
  let menuOpen = $state(false);
  let input: HTMLInputElement | undefined = $state();

  $effect(() => {
    if (app.pendingRename === section.id) {
      app.pendingRename = null;
      editing = true;
    }
  });

  $effect(() => {
    if (editing) {
      name = section.name;
      queueMicrotask(() => input?.select());
    }
  });

  async function save() {
    if (!editing) return;
    editing = false;
    const n = name.trim();
    if (!n || n === section.name) return;
    section.name = n;
    await api.sectionUpdate(section.id, n, section.collapsed).catch(() => {});
    await app.refreshMeta().catch(() => {});
  }

  async function toggle() {
    section.collapsed = !section.collapsed;
    await api.sectionUpdate(section.id, section.name, section.collapsed).catch(() => {});
  }
</script>

<div class="sh">
  <button type="button" class="caret" class:open={!section.collapsed} onclick={toggle} aria-label="Recolher divisão">
    <ChevronRight size={15} strokeWidth={2} />
  </button>

  {#if editing}
    <input
      class="rename"
      bind:this={input}
      bind:value={name}
      maxlength="120"
      onblur={save}
      onkeydown={(e) => {
        if (e.key === "Enter") save();
        if (e.key === "Escape") {
          e.stopPropagation();
          editing = false;
        }
      }}
    />
  {:else}
    <button type="button" class="name" ondblclick={() => (editing = true)} title="Clique duas vezes para renomear">{section.name}</button>
    <span class="count">{count}</span>
  {/if}

  <div class="actions">
    <button type="button" class="icon-btn sm" title={addLabel} onclick={onadd}><Plus size={15} /></button>
    <button type="button" class="icon-btn sm" bind:this={menuAnchor} title="Opções" onclick={() => (menuOpen = !menuOpen)}>
      <Ellipsis size={15} />
    </button>
  </div>
</div>

<Menu
  anchor={menuAnchor}
  bind:open={menuOpen}
  items={[
    { label: "Renomear", icon: Pencil, onclick: () => (editing = true) },
    "sep",
    {
      label: "Excluir divisão",
      icon: Trash,
      danger: true,
      onclick: () => deleteWithUndo(`Divisão "${section.name}" excluída`, () => api.sectionDelete(section.id)),
    },
  ]}
/>

<style>
  .sh {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 36px;
    margin-top: 18px;
    padding-bottom: 4px;
    border-bottom: 1px solid var(--hairline);
  }
  .caret {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    margin-left: -2px;
    border-radius: var(--r-xs);
    color: var(--ink-tertiary);
    transition: transform 0.15s var(--ease);
  }
  .caret:hover {
    color: var(--ink);
    background: var(--hover);
  }
  .caret.open {
    transform: rotate(90deg);
  }
  .name {
    font-size: 13.5px;
    font-weight: 600;
    color: var(--ink);
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    font-size: 12px;
    color: var(--ink-tertiary);
    font-variant-numeric: tabular-nums;
  }
  .rename {
    flex: 1;
    height: 28px;
    padding: 0 8px;
    font-size: 13.5px;
    font-weight: 600;
    background: var(--surface-1);
    border: 1px solid var(--primary);
    border-radius: var(--r-sm);
    outline: none;
    box-shadow: 0 0 0 3px var(--primary-soft);
  }
  .actions {
    margin-left: auto;
    display: flex;
    gap: 2px;
    opacity: 0;
    transition: opacity 0.12s;
  }
  .sh:hover .actions,
  .actions:focus-within {
    opacity: 1;
  }
</style>
