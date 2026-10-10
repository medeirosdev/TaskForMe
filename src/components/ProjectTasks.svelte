<script lang="ts">
  import { SvelteSet } from "svelte/reactivity";
  import QuickAdd from "./QuickAdd.svelte";
  import SectionHeader from "./SectionHeader.svelte";
  import TaskRow from "./TaskRow.svelte";
  import EmptyState from "./EmptyState.svelte";
  import { ListChecks } from "../lib/icons";
  import { api } from "../lib/api";
  import { byDisplayOrder, isVisible, toggleTask } from "../lib/actions";
  import { app } from "../lib/store.svelte";
  import type { Project, Task } from "../lib/types";

  let { project }: { project: Project } = $props();

  let tasks = $state<Task[]>([]);
  let loaded = $state(false);
  let addingIn = $state<number | null>(null);
  const collapsed = new SvelteSet<number>();

  $effect(() => {
    const id = project.id;
    void app.version;
    api
      .tasksByProject(id)
      .then((r) => {
        if (id === project.id) {
          tasks = r;
          loaded = true;
        }
      })
      .catch(() => {});
  });

  const sections = $derived(app.sectionsOf(project.id));
  const parents = $derived(tasks.filter((t) => !t.parentId && isVisible(t)).sort(byDisplayOrder));
  const childrenOf = $derived.by(() => {
    const m = new Map<number, Task[]>();
    for (const t of tasks) {
      if (!t.parentId) continue;
      if (!m.has(t.parentId)) m.set(t.parentId, []);
      m.get(t.parentId)!.push(t);
    }
    for (const list of m.values()) list.sort((a, b) => a.position - b.position || a.id - b.id);
    return m;
  });
  const hiddenDone = $derived(tasks.filter((t) => !t.parentId && !isVisible(t)).length);
  const openCount = (sid: number) => tasks.filter((t) => !t.parentId && t.sectionId === sid && !t.done).length;
  const inSection = (sid: number | null) => parents.filter((t) => t.sectionId === sid);

  function toggleExpand(id: number) {
    if (collapsed.has(id)) collapsed.delete(id);
    else collapsed.add(id);
  }
</script>

{#snippet group(list: Task[])}
  {#each list as t (t.id)}
    {@const kids = childrenOf.get(t.id) ?? []}
    <TaskRow
      task={t}
      progress={kids.length ? { done: kids.filter((k) => k.done).length, total: kids.length } : null}
      expanded={!collapsed.has(t.id)}
      onexpand={() => toggleExpand(t.id)}
      ontoggle={(task, v) => toggleTask(task, v, tasks)}
    />
    {#if kids.length && !collapsed.has(t.id)}
      {#each kids as k (k.id)}
        <TaskRow task={k} nested ontoggle={(task, v) => toggleTask(task, v, tasks)} />
      {/each}
    {/if}
  {/each}
{/snippet}

<div class="wrap">
  <QuickAdd projectId={project.id} autofocus placeholder="Adicionar tarefa em {project.name}" />

  <div class="list">
    {@render group(inSection(null))}
  </div>

  {#each sections as s (s.id)}
    <section>
      <SectionHeader section={s} count={openCount(s.id)} onadd={() => (addingIn = s.id)} />
      {#if !s.collapsed}
        <div class="list">
          {@render group(inSection(s.id))}
          {#if addingIn === s.id}
            <QuickAdd
              projectId={project.id}
              sectionId={s.id}
              inline
              autofocus
              placeholder="Nova tarefa em {s.name}"
              onclose={() => (addingIn = null)}
            />
          {:else}
            <button type="button" class="add-row" onclick={() => (addingIn = s.id)}>+ Adicionar tarefa</button>
          {/if}
        </div>
      {/if}
    </section>
  {/each}

  {#if loaded && tasks.length === 0 && sections.length === 0}
    <EmptyState icon={ListChecks} title="Nenhuma tarefa por aqui">
      Digite no campo acima e pressione <span class="kbd">Enter</span>. Use <b>Nova divisão</b> para separar
      as tarefas em grupos.
    </EmptyState>
  {/if}

  {#if hiddenDone > 0}
    <button type="button" class="hidden-done" onclick={() => app.setShowDone(true)}>
      {hiddenDone}
      {hiddenDone === 1 ? "tarefa concluída oculta" : "tarefas concluídas ocultas"} · Mostrar
    </button>
  {/if}
</div>

<style>
  .wrap {
    display: flex;
    flex-direction: column;
    padding-bottom: 48px;
  }
  .list {
    display: flex;
    flex-direction: column;
    margin-top: 6px;
  }
  .add-row {
    align-self: flex-start;
    height: 32px;
    padding: 0 10px 0 28px;
    font-size: 13px;
    color: var(--ink-tertiary);
    border-radius: var(--r-sm);
  }
  .add-row:hover {
    color: var(--primary-ink);
  }
  .hidden-done {
    align-self: flex-start;
    margin-top: 20px;
    font-size: 12.5px;
    color: var(--ink-tertiary);
    padding: 4px 8px;
    border-radius: var(--r-sm);
  }
  .hidden-done:hover {
    color: var(--ink);
    background: var(--hover);
  }
</style>
