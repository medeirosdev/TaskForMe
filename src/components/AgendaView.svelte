<script lang="ts">
  import QuickAdd from "./QuickAdd.svelte";
  import TaskRow from "./TaskRow.svelte";
  import EmptyState from "./EmptyState.svelte";
  import { CalendarRange, Eye, EyeOff, Inbox, Plus, StickyNote, Sun, Sunrise } from "../lib/icons";
  import { api } from "../lib/api";
  import { addDays, longDate, relativeDate, shortDate, weekRange } from "../lib/dates";
  import { isVisible, toggleTask } from "../lib/actions";
  import { app } from "../lib/store.svelte";
  import type { Agenda, FilterKind, Note, Task } from "../lib/types";

  let { kind }: { kind: FilterKind } = $props();

  let data = $state<Agenda>({ tasks: [], notes: [], overdue: [] });
  let loaded = $state(false);
  let addingOn = $state<string | null>(null);

  const range = $derived.by((): { from: string; to: string; overdue: string | null } => {
    const t = app.today;
    if (kind === "today") return { from: t, to: t, overdue: t };
    if (kind === "tomorrow") return { from: addDays(t, 1), to: addDays(t, 1), overdue: null };
    const [a, b] = weekRange(t);
    return { from: a, to: b, overdue: a };
  });

  $effect(() => {
    const r = range;
    void app.version;
    api
      .agenda(r.from, r.to, r.overdue)
      .then((d) => {
        if (r === range) {
          data = d;
          loaded = true;
        }
      })
      .catch(() => {});
  });

  const days = $derived.by(() => {
    const out: string[] = [];
    for (let d = range.from; d <= range.to; d = addDays(d, 1)) out.push(d);
    return out;
  });

  const all = $derived([...data.tasks, ...data.overdue]);
  const tasksOn = (d: string) => data.tasks.filter((t) => t.dueDate === d && isVisible(t));
  const notesOn = (d: string) => data.notes.filter((n) => n.noteDate === d);
  const overdue = $derived(data.overdue.filter(isVisible));
  const total = $derived(data.tasks.filter((t) => !t.done).length + data.overdue.filter((t) => !t.done).length);

  const heading = $derived(
    kind === "today"
      ? { icon: Sun, title: "Hoje", sub: longDate(app.today) }
      : kind === "tomorrow"
        ? { icon: Sunrise, title: "Amanhã", sub: longDate(addDays(app.today, 1)) }
        : {
            icon: CalendarRange,
            title: "Esta semana",
            sub: `${shortDate(range.from, app.today)} – ${shortDate(range.to, app.today)}`,
          },
  );

  async function rescheduleOverdue() {
    const list = data.overdue.filter((t) => !t.done);
    try {
      for (const t of list) {
        await api.taskUpdate({
          id: t.id,
          title: t.title,
          description: t.description,
          dueDate: app.today,
          remindAt: t.remindAt,
          projectId: t.projectId,
          sectionId: t.sectionId,
        });
      }
    } finally {
      app.touch();
    }
  }

  function openNote(n: Note) {
    app.navigate({ kind: "project", id: n.projectId, tab: "notes", noteId: n.id });
  }

  const toggle = (t: Task, v: boolean) => toggleTask(t, v, all);
</script>

{#snippet noteRow(n: Note)}
  {@const isInbox = app.project(n.projectId)?.isInbox}
  <button type="button" class="note-row" onclick={() => openNote(n)}>
    <span class="nicon"><StickyNote size={15} strokeWidth={1.75} /></span>
    <span class="ntitle">{n.title || "Sem título"}</span>
    <span class="npreview">{n.body.split("\n").find((l) => l.trim())?.slice(0, 100) ?? ""}</span>
    <span class="nproj">
      {#if isInbox}<Inbox size={12} />{:else}<span class="dot" style:background={n.projectColor}></span>{/if}
      {n.projectName}
    </span>
  </button>
{/snippet}

<header class="head">
  <div class="title">
    <heading.icon size={20} strokeWidth={1.75} />
    <h1>{heading.title}</h1>
    <span class="sub">{heading.sub}</span>
  </div>
  <div class="tools">
    {#if total > 0}<span class="total">{total} {total === 1 ? "pendente" : "pendentes"}</span>{/if}
    <button type="button" class="btn btn-ghost" onclick={() => app.setShowDone(!app.showDone)}>
      {#if app.showDone}<EyeOff size={15} /> Ocultar concluídas{:else}<Eye size={15} /> Mostrar concluídas{/if}
    </button>
  </div>
</header>

<div class="content">
  <div class="inner">
    <QuickAdd
      dueDate={kind === "tomorrow" ? addDays(app.today, 1) : app.today}
      pickProject
      placeholder={kind === "tomorrow" ? "Adicionar tarefa para amanhã" : "Adicionar tarefa para hoje"}
    />

    {#if overdue.length}
      <section>
        <div class="gh overdue">
          <span class="gname">Atrasadas</span>
          <span class="gcount">{overdue.filter((t) => !t.done).length}</span>
          <button type="button" class="gaction" onclick={rescheduleOverdue}>Mover tudo para hoje</button>
        </div>
        {#each overdue as t (t.id)}
          <TaskRow task={t} showProject ontoggle={toggle} />
        {/each}
      </section>
    {/if}

    {#each days as d (d)}
      {@const ts = tasksOn(d)}
      {@const ns = notesOn(d)}
      {#if ts.length || ns.length || addingOn === d || (kind === "week" && d >= app.today)}
        <section class:past={d < app.today}>
          <div class="gh">
            <span class="gname">
              {#if kind === "week"}
                {relativeDate(d, app.today)}
                <span class="gdate">{shortDate(d, app.today)}</span>
              {:else}
                {kind === "today" ? "Hoje" : "Amanhã"}
              {/if}
            </span>
            <span class="gcount">{ts.filter((t) => !t.done).length || ""}</span>
            {#if kind === "week"}
              <button type="button" class="icon-btn sm add" title="Adicionar neste dia" onclick={() => (addingOn = d)}>
                <Plus size={15} />
              </button>
            {/if}
          </div>

          {#each ts as t (t.id)}
            <TaskRow task={t} showProject showDate={false} ontoggle={toggle} />
          {/each}
          {#each ns as n (n.id)}
            {@render noteRow(n)}
          {/each}

          {#if addingOn === d}
            <QuickAdd dueDate={d} inline autofocus pickProject onclose={() => (addingOn = null)} />
          {:else if kind === "week" && !ts.length && !ns.length}
            <button type="button" class="nothing" onclick={() => (addingOn = d)}>Nada planejado</button>
          {/if}
        </section>
      {/if}
    {/each}

    {#if loaded && kind !== "week" && !data.tasks.length && !data.notes.length && !overdue.length}
      <EmptyState icon={heading.icon} title={kind === "today" ? "Tudo livre por hoje" : "Nada para amanhã"}>
        Tarefas e notas com data aparecem aqui automaticamente, de todos os projetos.
      </EmptyState>
    {/if}
  </div>
</div>

<style>
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 22px 32px 18px;
    border-bottom: 1px solid var(--hairline);
    flex-shrink: 0;
  }
  .title {
    display: flex;
    align-items: baseline;
    gap: 10px;
    min-width: 0;
    color: var(--ink-subtle);
  }
  .title :global(svg) {
    align-self: center;
  }
  h1 {
    margin: 0;
    font-size: 22px;
    font-weight: 600;
    letter-spacing: -0.4px;
    color: var(--ink);
  }
  .sub {
    font-size: 13px;
    color: var(--ink-subtle);
    white-space: nowrap;
  }
  .tools {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .total {
    font-size: 12.5px;
    color: var(--ink-tertiary);
    white-space: nowrap;
  }
  .content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .inner {
    max-width: 860px;
    margin: 0 auto;
    padding: 20px 32px 48px;
  }
  section {
    margin-top: 18px;
  }
  section.past {
    opacity: 0.72;
  }
  .gh {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    margin-bottom: 4px;
    border-bottom: 1px solid var(--hairline);
  }
  .gname {
    font-size: 13.5px;
    font-weight: 600;
  }
  .gdate {
    margin-left: 6px;
    font-weight: 400;
    color: var(--ink-tertiary);
    font-size: 12.5px;
  }
  .gcount {
    font-size: 12px;
    color: var(--ink-tertiary);
  }
  .gh.overdue .gname {
    color: var(--danger);
  }
  .gaction {
    margin-left: auto;
    font-size: 12.5px;
    font-weight: 500;
    color: var(--primary-ink);
    padding: 3px 8px;
    border-radius: var(--r-sm);
  }
  .gaction:hover {
    background: var(--primary-soft);
  }
  .add {
    margin-left: auto;
    opacity: 0;
  }
  .gh:hover .add {
    opacity: 1;
  }
  .nothing {
    height: 32px;
    padding: 0 28px;
    font-size: 12.5px;
    color: var(--ink-tertiary);
  }
  .nothing:hover {
    color: var(--primary-ink);
  }
  .note-row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    min-height: 40px;
    padding: 6px 12px 6px 18px;
    border-radius: var(--r-md);
    text-align: left;
  }
  .note-row:hover {
    background: var(--hover);
  }
  .nicon {
    display: grid;
    place-items: center;
    width: 18px;
    color: var(--warning);
    margin-left: 10px;
  }
  .ntitle {
    flex-shrink: 0;
    max-width: 40%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .npreview {
    flex: 1;
    min-width: 0;
    font-size: 12.5px;
    color: var(--ink-tertiary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .nproj {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: var(--ink-subtle);
    white-space: nowrap;
  }
  .nproj .dot {
    width: 7px;
    height: 7px;
  }
</style>
