<script lang="ts">
  import NoteEditor from "./NoteEditor.svelte";
  import SectionHeader from "./SectionHeader.svelte";
  import EmptyState from "./EmptyState.svelte";
  import { AlarmClock, Calendar, Plus, StickyNote } from "../lib/icons";
  import { api } from "../lib/api";
  import { relativeDate, timeAgo } from "../lib/dates";
  import { app } from "../lib/store.svelte";
  import type { Note, Project } from "../lib/types";

  let { project, initialNoteId }: { project: Project; initialNoteId?: number } = $props();

  let notes = $state<Note[]>([]);
  let loaded = $state(false);
  // svelte-ignore state_referenced_locally
  let selected = $state<number | null>(initialNoteId ?? null);

  $effect(() => {
    const id = project.id;
    void app.version;
    api
      .notesByProject(id)
      .then((r) => {
        if (id !== project.id) return;
        notes = r;
        loaded = true;
        if (selected === null && r.length) selected = r[0].id;
      })
      .catch(() => {});
  });

  // Ctrl+N na aba Notas.
  $effect(() => {
    const onNew = () => create(null);
    window.addEventListener("tfm:new-note", onNew);
    return () => window.removeEventListener("tfm:new-note", onNew);
  });

  const sections = $derived(app.sectionsOf(project.id));
  const inSection = (sid: number | null) => notes.filter((n) => n.sectionId === sid);

  async function create(sectionId: number | null) {
    try {
      const n = await api.noteCreate(project.id, sectionId);
      notes = [n, ...notes];
      selected = n.id;
      app.touch();
    } catch {
      /* avisado */
    }
  }

  function preview(n: Note) {
    const line = n.body.split("\n").find((l) => l.trim()) ?? "";
    return line.trim().slice(0, 140);
  }
</script>

{#snippet list(items: Note[])}
  {#each items as n (n.id)}
    <button type="button" class="note" class:active={selected === n.id} onclick={() => (selected = n.id)}>
      <span class="n-title" class:untitled={!n.title}>{n.title || "Sem título"}</span>
      <span class="n-preview">{preview(n) || "Nota vazia"}</span>
      <span class="n-meta">
        {timeAgo(n.updatedAt)}
        {#if n.noteDate}<span class="tag"><Calendar size={11} />{relativeDate(n.noteDate, app.today)}</span>{/if}
        {#if n.remindAt}<span class="tag"><AlarmClock size={11} /></span>{/if}
      </span>
    </button>
  {/each}
{/snippet}

<div class="pane">
  <aside class="side">
    <button type="button" class="new" onclick={() => create(null)}><Plus size={15} strokeWidth={2} /> Nova nota</button>

    <div class="items">
      {@render list(inSection(null))}
      {#each sections as s (s.id)}
        <SectionHeader section={s} count={inSection(s.id).length} onadd={() => create(s.id)} addLabel="Nova nota" />
        {#if !s.collapsed}
          {@render list(inSection(s.id))}
        {/if}
      {/each}
    </div>
  </aside>

  <div class="main">
    {#if selected !== null && notes.some((n) => n.id === selected)}
      {#key selected}
        <NoteEditor noteId={selected} ondeleted={() => (selected = null)} />
      {/key}
    {:else if loaded}
      <EmptyState icon={StickyNote} title={notes.length ? "Selecione uma nota" : "Nenhuma nota ainda"}>
        {#if notes.length === 0}
          Notas são salvas automaticamente enquanto você escreve.
          <br /><button type="button" class="link" onclick={() => create(null)}>Criar a primeira nota</button>
        {/if}
      </EmptyState>
    {/if}
  </div>
</div>

<style>
  .pane {
    display: grid;
    grid-template-columns: 272px 1fr;
    height: 100%;
    min-height: 0;
  }
  .side {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 1px solid var(--hairline);
    padding: 12px 10px 0 20px;
  }
  .new {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 10px;
    border-radius: var(--r-md);
    font-size: 13px;
    font-weight: 500;
    color: var(--primary-ink);
    border: 1px dashed var(--hairline-strong);
    margin-bottom: 8px;
    margin-right: 6px;
  }
  .new:hover {
    background: var(--primary-soft);
    border-color: var(--primary);
  }
  .items {
    flex: 1;
    overflow-y: auto;
    padding: 0 6px 24px 0;
  }
  .items :global(.sh) {
    margin-top: 14px;
  }
  .note {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    padding: 9px 10px;
    margin-top: 2px;
    border-radius: var(--r-md);
    text-align: left;
    transition: background 0.1s;
  }
  .note:hover {
    background: var(--hover);
  }
  .note.active {
    background: var(--active);
  }
  .n-title {
    font-size: 13.5px;
    font-weight: 500;
    color: var(--ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .n-title.untitled {
    color: var(--ink-subtle);
    font-style: italic;
  }
  .n-preview {
    font-size: 12.5px;
    color: var(--ink-subtle);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .n-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 2px;
    font-size: 11.5px;
    color: var(--ink-tertiary);
  }
  .tag {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
  .main {
    min-width: 0;
    min-height: 0;
    overflow-y: auto;
  }
  .link {
    margin-top: 10px;
    color: var(--primary-ink);
    font-weight: 500;
  }
  .link:hover {
    text-decoration: underline;
  }
</style>
