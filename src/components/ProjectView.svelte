<script lang="ts">
  import Menu from "./Menu.svelte";
  import EventsPane from "./EventsPane.svelte";
  import LinksPane from "./LinksPane.svelte";
  import NotesPane from "./NotesPane.svelte";
  import ProjectTasks from "./ProjectTasks.svelte";
  import { ArrowDown, ArrowUp, Ellipsis, Eye, EyeOff, Inbox, Layers, Pencil, Trash } from "../lib/icons";
  import { api } from "../lib/api";
  import { app, deleteWithUndo } from "../lib/store.svelte";
  import type { Project, ProjectTab } from "../lib/types";

  let { project, tab, noteId }: { project: Project; tab: ProjectTab; noteId?: number } = $props();

  const tabs: { id: ProjectTab; label: string }[] = [
    { id: "tasks", label: "Tarefas" },
    { id: "notes", label: "Notas" },
    { id: "events", label: "Eventos" },
    { id: "links", label: "Links" },
  ];

  let menuAnchor: HTMLButtonElement | undefined = $state();
  let menuOpen = $state(false);

  async function newSection() {
    try {
      const id = await api.sectionCreate(project.id, "Nova divisão");
      await app.refreshMeta();
      app.pendingRename = id;
    } catch {
      /* avisado */
    }
  }

  async function move(dir: -1 | 1) {
    const ids = app.userProjects.map((p) => p.id);
    const i = ids.indexOf(project.id);
    const j = i + dir;
    if (i < 0 || j < 0 || j >= ids.length) return;
    [ids[i], ids[j]] = [ids[j], ids[i]];
    await api.projectMove(ids).catch(() => {});
    await app.refreshMeta().catch(() => {});
  }

  const idx = $derived(app.userProjects.findIndex((p) => p.id === project.id));
</script>

<header class="head">
  <div class="title">
    {#if project.isInbox}
      <Inbox size={20} strokeWidth={1.75} />
    {:else}
      <span class="pdot" style:background={project.color}></span>
    {/if}
    <h1>{project.name}</h1>
    <button type="button" class="icon-btn" bind:this={menuAnchor} onclick={() => (menuOpen = !menuOpen)} aria-label="Opções do projeto">
      <Ellipsis size={17} />
    </button>
  </div>

  <div class="bar">
    <div class="seg" role="tablist">
      {#each tabs as t (t.id)}
        <button
          type="button"
          role="tab"
          aria-selected={tab === t.id}
          class:on={tab === t.id}
          onclick={() => app.navigate({ kind: "project", id: project.id, tab: t.id })}>{t.label}</button
        >
      {/each}
    </div>

    <div class="tools">
      {#if tab === "tasks"}
        <button type="button" class="btn btn-ghost" onclick={() => app.setShowDone(!app.showDone)}>
          {#if app.showDone}<EyeOff size={15} /> Ocultar concluídas{:else}<Eye size={15} /> Mostrar concluídas{/if}
        </button>
      {/if}
      {#if tab !== "events"}
        <button type="button" class="btn btn-secondary" onclick={newSection}><Layers size={15} /> Nova divisão</button>
      {/if}
    </div>
  </div>
</header>

<div class="content" class:notes={tab === "notes"}>
  {#if tab === "tasks"}
    <div class="inner"><ProjectTasks {project} /></div>
  {:else if tab === "notes"}
    <NotesPane {project} initialNoteId={noteId} />
  {:else if tab === "events"}
    <div class="inner"><EventsPane {project} /></div>
  {:else}
    <div class="inner"><LinksPane {project} /></div>
  {/if}
</div>

<Menu
  anchor={menuAnchor}
  bind:open={menuOpen}
  align="start"
  items={project.isInbox
    ? [{ label: "Nova divisão", icon: Layers, onclick: newSection }]
    : [
        { label: "Editar projeto", icon: Pencil, onclick: () => (app.projectDialog = { mode: "edit", project }) },
        { label: "Nova divisão", icon: Layers, onclick: newSection },
        "sep",
        { label: "Mover para cima", icon: ArrowUp, disabled: idx <= 0, onclick: () => move(-1) },
        { label: "Mover para baixo", icon: ArrowDown, disabled: idx >= app.userProjects.length - 1, onclick: () => move(1) },
        "sep",
        {
          label: "Excluir projeto",
          icon: Trash,
          danger: true,
          onclick: () =>
            deleteWithUndo(`Projeto "${project.name}" excluído`, () => api.projectDelete(project.id)),
        },
      ]}
/>

<style>
  .head {
    padding: 22px 32px 0;
    border-bottom: 1px solid var(--hairline);
    flex-shrink: 0;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--ink-subtle);
  }
  .pdot {
    width: 12px;
    height: 12px;
    border-radius: 4px;
  }
  h1 {
    margin: 0;
    font-size: 22px;
    font-weight: 600;
    letter-spacing: -0.4px;
    color: var(--ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .bar {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 12px;
    margin-top: 14px;
  }
  .seg {
    display: flex;
    gap: 18px;
  }
  .seg button {
    position: relative;
    height: 36px;
    font-size: 13.5px;
    font-weight: 500;
    color: var(--ink-subtle);
  }
  .seg button:hover {
    color: var(--ink);
  }
  .seg button.on {
    color: var(--ink);
  }
  .seg button.on::after {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    bottom: -1px;
    height: 2px;
    border-radius: 2px;
    background: var(--primary);
  }
  .tools {
    display: flex;
    gap: 6px;
    padding-bottom: 6px;
  }
  .content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .content.notes {
    overflow: hidden;
  }
  .inner {
    max-width: 860px;
    margin: 0 auto;
    padding: 20px 32px 0;
  }
</style>
