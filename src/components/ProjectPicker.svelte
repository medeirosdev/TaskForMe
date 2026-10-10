<script lang="ts">
  import Popover from "./Popover.svelte";
  import { Check, Inbox, Layers } from "../lib/icons";
  import { app } from "../lib/store.svelte";

  let {
    projectId,
    sectionId = null,
    onchange,
    sections = true,
  }: {
    projectId: number | null;
    sectionId?: number | null;
    onchange: (projectId: number, sectionId: number | null) => void;
    sections?: boolean;
  } = $props();

  let anchor: HTMLButtonElement | undefined = $state();
  let open = $state(false);

  const project = $derived(app.project(projectId) ?? app.inbox);
  const section = $derived(app.sections.find((s) => s.id === sectionId));

  function pick(p: number, s: number | null) {
    open = false;
    if (p !== projectId || s !== sectionId) onchange(p, s);
  }
</script>

<button type="button"
  class="chip"
  bind:this={anchor}
  onclick={(e) => {
    e.stopPropagation();
    open = !open;
  }}
>
  {#if project?.isInbox}
    <Inbox size={13} strokeWidth={2} />
  {:else}
    <span class="dot" style:background={project?.color}></span>
  {/if}
  <span class="name">{project?.name ?? "Projeto"}{#if section}<span class="sec"> / {section.name}</span>{/if}</span>
</button>

<Popover {anchor} bind:open width={240}>
  <div class="list">
    {#each app.projects as p (p.id)}
      <button type="button" class="row" onclick={() => pick(p.id, null)}>
        {#if p.isInbox}<Inbox size={14} />{:else}<span class="dot" style:background={p.color}></span>{/if}
        <span class="label">{p.name}</span>
        {#if p.id === projectId && !sectionId}<Check size={14} />{/if}
      </button>
      {#if sections}
        {#each app.sectionsOf(p.id) as s (s.id)}
          <button type="button" class="row sub" onclick={() => pick(p.id, s.id)}>
            <Layers size={13} />
            <span class="label">{s.name}</span>
            {#if s.id === sectionId}<Check size={14} />{/if}
          </button>
        {/each}
      {/if}
    {/each}
  </div>
</Popover>

<style>
  .name {
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sec {
    color: var(--ink-tertiary);
  }
  .list {
    display: flex;
    flex-direction: column;
    max-height: 320px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 9px;
    height: 30px;
    padding: 0 10px;
    border-radius: var(--r-sm);
    font-size: 13px;
    color: var(--ink-muted);
    text-align: left;
  }
  .row :global(svg) {
    color: var(--ink-subtle);
    flex-shrink: 0;
  }
  .row:hover {
    background: var(--hover);
    color: var(--ink);
  }
  .row.sub {
    padding-left: 28px;
    font-size: 12.5px;
  }
  .label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
