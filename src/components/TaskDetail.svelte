<script lang="ts">
  import { onDestroy } from "svelte";
  import Checkbox from "./Checkbox.svelte";
  import DateField from "./DateField.svelte";
  import Menu from "./Menu.svelte";
  import ProjectPicker from "./ProjectPicker.svelte";
  import { ArrowRight, CornerDownRight, Ellipsis, Plus, Trash, X } from "../lib/icons";
  import { api } from "../lib/api";
  import { toggleTask } from "../lib/actions";
  import { autosize } from "../lib/portal";
  import { shortDate } from "../lib/dates";
  import { app, deleteWithUndo, registerFlusher } from "../lib/store.svelte";
  import type { Task } from "../lib/types";

  let { taskId }: { taskId: number } = $props();

  let task = $state<Task | null>(null);
  let title = $state("");
  let description = $state("");
  let subs = $state<Task[]>([]);
  let newSub = $state("");

  let rev = 0;
  let savedRev = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let saving: Promise<void> | null = null;

  let menuAnchor: HTMLButtonElement | undefined = $state();
  let menuOpen = $state(false);

  async function load(full: boolean) {
    try {
      const t = await api.task(taskId);
      const s = t.parentId ? [] : await api.subtasks(taskId);
      if (full || rev === savedRev) {
        task = t;
        title = t.title;
        description = t.description;
      } else if (task) {
        task.done = t.done;
      }
      subs = s;
    } catch {
      app.selectedTaskId = null;
    }
  }
  // taskId é fixo por instância (o pai usa {#key}).
  void load(true);

  // Outra parte da tela marcou esta tarefa (ou uma subtarefa) como concluída.
  let lastSignal = app.taskSignal.n;
  $effect(() => {
    const sig = app.taskSignal;
    if (sig.n === lastSignal) return;
    lastSignal = sig.n;
    if (sig.id === taskId || subs.some((s) => s.id === sig.id)) void load(false);
  });

  function patch(over: Partial<Task> = {}) {
    const t = { ...task!, title, description, ...over };
    return {
      id: t.id,
      title: t.title,
      description: t.description,
      dueDate: t.dueDate,
      remindAt: t.remindAt,
      projectId: t.projectId,
      sectionId: t.sectionId,
    };
  }

  function edited() {
    rev++;
    clearTimeout(timer);
    timer = setTimeout(save, 500);
  }

  async function save(): Promise<void> {
    clearTimeout(timer);
    if (!task || rev === savedRev) return;
    if (saving) {
      await saving;
      return save();
    }
    const target = rev;
    saving = (async () => {
      try {
        const t = await api.taskUpdate(patch());
        if (task) {
          task.dueDate = t.dueDate;
          task.remindAt = t.remindAt;
        }
        savedRev = target;
        app.touch();
      } catch {
        timer = setTimeout(save, 3000);
      }
    })();
    await saving;
    saving = null;
  }

  async function setProp(over: Partial<Task>) {
    if (!task) return;
    Object.assign(task, over);
    rev++;
    await save();
  }

  const unregister = registerFlusher(save);
  onDestroy(() => {
    unregister();
    void save();
  });

  async function addSub() {
    const t = newSub.trim();
    if (!t || !task) return;
    newSub = "";
    try {
      const s = await api.taskCreate({ title: t, parentId: task.id });
      subs = [...subs, s];
      app.touch();
    } catch {
      newSub = t;
    }
  }

  async function renameSub(s: Task, value: string) {
    const v = value.trim();
    if (!v || v === s.title) return;
    s.title = v;
    await api
      .taskUpdate({ id: s.id, title: v, description: s.description, dueDate: s.dueDate, remindAt: s.remindAt, projectId: s.projectId, sectionId: s.sectionId })
      .catch(() => {});
    app.touch();
  }

  async function deleteSub(s: Task) {
    const ok = await deleteWithUndo("Subtarefa excluída", () => api.taskDelete(s.id));
    if (ok) subs = subs.filter((x) => x.id !== s.id);
  }

  const doneCount = $derived(subs.filter((s) => s.done).length);
  const created = $derived(task ? shortDate(task.createdAt.slice(0, 10)) : "");
  const createdTime = $derived(task ? task.createdAt.slice(11, 16) : "");
</script>

<aside class="detail">
  {#if task}
    <div class="top">
      {#if task.parentId}
        <button type="button" class="crumb" onclick={() => app.openTask(task!.parentId!)}>
          <CornerDownRight size={13} /> Subtarefa de <b>{task.parentTitle}</b>
        </button>
      {:else}
        <ProjectPicker
          projectId={task.projectId}
          sectionId={task.sectionId}
          onchange={(p, s) => setProp({ projectId: p, sectionId: s })}
        />
      {/if}
      <div class="spacer"></div>
      <button type="button" class="icon-btn" bind:this={menuAnchor} onclick={() => (menuOpen = !menuOpen)} aria-label="Opções">
        <Ellipsis size={16} />
      </button>
      <button type="button" class="icon-btn" onclick={() => app.closeTask()} aria-label="Fechar" title="Fechar (Esc)">
        <X size={17} />
      </button>
    </div>

    <div class="scroll">
      <div class="title-row">
        <Checkbox checked={task.done} size={20} onchange={(v) => toggleTask(task!, v, subs)} />
        <textarea
          class="title"
          class:done={task.done}
          rows="1"
          bind:value={title}
          use:autosize={title}
          oninput={edited}
          onblur={save}
          maxlength="500"
          placeholder="Título da tarefa"
          onkeydown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              e.currentTarget.blur();
            }
          }}
        ></textarea>
      </div>

      <div class="props">
        <span class="label">Data</span>
        <div><DateField value={task.dueDate} onchange={(v) => setProp({ dueDate: v })} placeholder="Sem data" /></div>
        <span class="label">Lembrete</span>
        <div>
          <DateField value={task.remindAt} mode="datetime" onchange={(v) => setProp({ remindAt: v })} placeholder="Sem lembrete" />
        </div>
      </div>

      <textarea
        class="desc"
        rows="3"
        bind:value={description}
        use:autosize={description}
        oninput={edited}
        onblur={save}
        placeholder="Adicionar descrição…"
      ></textarea>

      {#if !task.parentId}
        <div class="subs">
          <div class="subs-head">
            <span class="eyebrow">Subtarefas</span>
            {#if subs.length}
              <span class="progress">
                <span class="bar"><span style:width="{(doneCount / subs.length) * 100}%"></span></span>
                {doneCount}/{subs.length}
              </span>
            {/if}
          </div>

          {#each subs as s (s.id)}
            <div class="sub" class:done={s.done}>
              <Checkbox checked={s.done} size={16} onchange={(v) => toggleTask(s, v)} />
              <input
                value={s.title}
                maxlength="500"
                onblur={(e) => renameSub(s, e.currentTarget.value)}
                onkeydown={(e) => {
                  if (e.key === "Enter") e.currentTarget.blur();
                }}
              />
              {#if s.dueDate}<span class="sub-date">{shortDate(s.dueDate, app.today)}</span>{/if}
              <button type="button" class="icon-btn sm act" title="Abrir subtarefa" onclick={() => app.openTask(s.id)}>
                <ArrowRight size={14} />
              </button>
              <button type="button" class="icon-btn sm act" title="Excluir" onclick={() => deleteSub(s)}>
                <Trash size={14} />
              </button>
            </div>
          {/each}

          <form
            class="sub add"
            onsubmit={(e) => {
              e.preventDefault();
              addSub();
            }}
          >
            <span class="plus"><Plus size={15} /></span>
            <input bind:value={newSub} placeholder="Adicionar subtarefa" maxlength="500" />
          </form>
        </div>
      {/if}
    </div>

    <div class="foot">Criada em {created} às {createdTime}</div>
  {/if}
</aside>

<Menu
  anchor={menuAnchor}
  bind:open={menuOpen}
  items={[
    {
      label: task?.parentId ? "Excluir subtarefa" : "Excluir tarefa",
      icon: Trash,
      danger: true,
      onclick: async () => {
        clearTimeout(timer);
        rev = savedRev;
        const ok = await deleteWithUndo("Tarefa excluída", () => api.taskDelete(taskId));
        if (ok) app.selectedTaskId = null;
      },
    },
  ]}
/>

<style>
  .detail {
    display: flex;
    flex-direction: column;
    width: var(--detail-w);
    height: 100%;
    background: var(--bg);
    border-left: 1px solid var(--hairline);
  }
  .top {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 52px;
    padding: 0 10px 0 16px;
    border-bottom: 1px solid var(--hairline);
    flex-shrink: 0;
  }
  .spacer {
    flex: 1;
  }
  .crumb {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    font-size: 12.5px;
    color: var(--ink-subtle);
    padding: 4px 6px;
    border-radius: var(--r-sm);
    white-space: nowrap;
    overflow: hidden;
  }
  .crumb b {
    font-weight: 500;
    color: var(--ink-muted);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .crumb:hover {
    background: var(--hover);
  }
  .scroll {
    flex: 1;
    overflow-y: auto;
    padding: 18px 20px 24px;
  }
  .title-row {
    display: flex;
    align-items: flex-start;
    gap: 12px;
  }
  .title-row :global(.cb) {
    margin-top: 1px;
  }
  textarea {
    width: 100%;
    border: 0;
    outline: none;
    resize: none;
    background: transparent;
    overflow: hidden;
  }
  .title {
    padding: 0;
    font-size: 18px;
    font-weight: 600;
    letter-spacing: -0.3px;
    line-height: 1.4;
  }
  .title.done {
    color: var(--ink-subtle);
    text-decoration: line-through;
  }
  .props {
    display: grid;
    grid-template-columns: 84px 1fr;
    align-items: center;
    row-gap: 8px;
    margin: 18px 0 6px;
    padding: 12px 0;
    border-top: 1px solid var(--hairline);
    border-bottom: 1px solid var(--hairline);
  }
  .label {
    font-size: 12.5px;
    color: var(--ink-subtle);
  }
  .desc {
    min-height: 72px;
    margin-top: 10px;
    padding: 6px 0;
    font-size: 14px;
    line-height: 1.65;
    color: var(--ink-muted);
  }
  .subs {
    margin-top: 16px;
  }
  .subs-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 6px;
  }
  .progress {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--ink-subtle);
    font-variant-numeric: tabular-nums;
  }
  .bar {
    width: 64px;
    height: 4px;
    border-radius: 4px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--primary);
    border-radius: 4px;
    transition: width 0.3s var(--ease);
  }
  .sub {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 36px;
    padding: 0 4px 0 8px;
    border-radius: var(--r-md);
  }
  .sub:hover {
    background: var(--hover);
  }
  .sub input {
    flex: 1;
    min-width: 0;
    height: 28px;
    border: 0;
    outline: none;
    background: transparent;
    font-size: 13.5px;
  }
  .sub.done input {
    color: var(--ink-tertiary);
    text-decoration: line-through;
  }
  .sub-date {
    font-size: 11.5px;
    color: var(--ink-tertiary);
  }
  .sub .act {
    opacity: 0;
  }
  .sub:hover .act {
    opacity: 1;
  }
  .sub.add .plus {
    display: grid;
    place-items: center;
    width: 16px;
    color: var(--primary-ink);
  }
  .sub.add input::placeholder {
    color: var(--ink-tertiary);
  }
  .foot {
    padding: 10px 20px;
    font-size: 11.5px;
    color: var(--ink-tertiary);
    border-top: 1px solid var(--hairline);
  }
</style>
