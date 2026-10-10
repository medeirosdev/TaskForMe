<script lang="ts">
  import Checkbox from "./Checkbox.svelte";
  import { AlarmClock, Calendar, ChevronRight, CornerDownRight, Inbox, ListChecks, TextAlign } from "../lib/icons";
  import { dueTone, relativeDate, reminderLabel } from "../lib/dates";
  import { app } from "../lib/store.svelte";
  import type { Task } from "../lib/types";

  let {
    task,
    progress = null,
    expanded = true,
    onexpand,
    ontoggle,
    showProject = false,
    showDate = true,
    nested = false,
  }: {
    task: Task;
    progress?: { done: number; total: number } | null;
    expanded?: boolean;
    onexpand?: () => void;
    ontoggle: (task: Task, done: boolean) => void;
    showProject?: boolean;
    showDate?: boolean;
    nested?: boolean;
  } = $props();

  const selected = $derived(app.selectedTaskId === task.id);
  const tone = $derived(task.done ? "" : dueTone(task.dueDate, app.today));
  const isInbox = $derived(app.project(task.projectId)?.isInbox);
</script>

<div
  class="row"
  class:selected
  class:done={task.done}
  class:nested
  role="button"
  tabindex="0"
  onclick={() => app.openTask(task.id)}
  onkeydown={(e) => {
    if (e.key === "Enter") app.openTask(task.id);
    if (e.key === " ") {
      e.preventDefault();
      ontoggle(task, !task.done);
    }
  }}
>
  <span class="lead">
    {#if progress && onexpand}
      <button type="button"
        class="caret"
        class:open={expanded}
        aria-label={expanded ? "Recolher subtarefas" : "Expandir subtarefas"}
        onclick={(e) => {
          e.stopPropagation();
          onexpand();
        }}
      >
        <ChevronRight size={14} strokeWidth={2} />
      </button>
    {/if}
  </span>

  <Checkbox checked={task.done} onchange={(v) => ontoggle(task, v)} />

  <div class="body">
    <span class="title">{task.title}</span>
    {#if task.parentTitle && !nested}
      <span class="parent"><CornerDownRight size={12} />{task.parentTitle}</span>
    {/if}
  </div>

  <div class="meta">
    {#if task.description.trim()}
      <span class="m" title="Tem descrição"><TextAlign size={13} /></span>
    {/if}
    {#if progress && progress.total > 0}
      <span class="m" class:complete={progress.done === progress.total} title="Subtarefas">
        <ListChecks size={13} />{progress.done}/{progress.total}
      </span>
    {/if}
    {#if task.remindAt && !task.done}
      <span class="m" title="Lembrete"><AlarmClock size={13} />{reminderLabel(task.remindAt, app.today)}</span>
    {/if}
    {#if showDate && task.dueDate}
      <span class="m due {tone}"><Calendar size={13} />{relativeDate(task.dueDate, app.today)}</span>
    {/if}
    {#if showProject}
      <span class="m project">
        {#if isInbox}<Inbox size={12} />{:else}<span class="dot" style:background={task.projectColor}></span>{/if}
        {task.projectName}
      </span>
    {/if}
  </div>
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 40px;
    padding: 6px 12px 6px 0;
    border-radius: var(--r-md);
    outline: none;
    transition: background 0.1s;
  }
  .row:hover {
    background: var(--hover);
  }
  .row:focus-visible {
    box-shadow: inset 0 0 0 1.5px color-mix(in srgb, var(--primary) 60%, transparent);
  }
  .row.selected {
    background: var(--active);
  }
  .row.nested {
    padding-left: 28px;
    min-height: 36px;
  }
  .lead {
    width: 18px;
    display: grid;
    place-items: center;
    flex-shrink: 0;
  }
  .caret {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: var(--r-xs);
    color: var(--ink-tertiary);
    transition: transform 0.15s var(--ease), color 0.1s;
  }
  .caret:hover {
    color: var(--ink);
    background: var(--hover);
  }
  .caret.open {
    transform: rotate(90deg);
  }
  .body {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink);
    transition: color 0.2s;
  }
  .done .title {
    color: var(--ink-tertiary);
    text-decoration: line-through;
    text-decoration-color: var(--ink-tertiary);
  }
  .parent {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: var(--ink-tertiary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-shrink: 0;
  }
  .m {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: var(--ink-subtle);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .m.complete {
    color: var(--success);
  }
  .due.overdue {
    color: var(--danger);
  }
  .due.today {
    color: var(--primary-ink);
  }
  .due.tomorrow {
    color: var(--warning);
  }
  .project {
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .project .dot {
    width: 7px;
    height: 7px;
  }
  .done .meta {
    opacity: 0.6;
  }
</style>
