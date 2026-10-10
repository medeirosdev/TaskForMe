<script lang="ts">
  import type { Component } from "svelte";
  import { CalendarRange, Inbox, Plus, Settings, Sun, Sunrise } from "../lib/icons";
  import { app } from "../lib/store.svelte";
  import type { View } from "../lib/types";

  const isActive = (v: View) => {
    const c = app.view;
    if (v.kind !== c.kind) return false;
    return v.kind !== "project" || (c.kind === "project" && c.id === v.id);
  };

  const inboxCount = $derived(app.inbox ? app.projectCount(app.inbox.id) : 0);
</script>

{#snippet nav(view: View, Icon: Component<any>, label: string, count: number, alert = false)}
  <button type="button" class="nav" class:active={isActive(view)} onclick={() => app.navigate(view)}>
    <Icon size={16} strokeWidth={1.75} />
    <span class="label">{label}</span>
    {#if count > 0}<span class="count" class:alert>{count}</span>{/if}
  </button>
{/snippet}

<nav class="sidebar">
  <div class="brand">
    <span class="logo">
      <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12.5l4.2 4.2L19 7" /></svg>
    </span>
    TaskForMe
  </div>

  <div class="group">
    {#if app.inbox}
      {@render nav({ kind: "project", id: app.inbox.id, tab: "tasks" }, Inbox, "Entrada", inboxCount)}
    {/if}
    {@render nav({ kind: "today" }, Sun, "Hoje", app.counts.today + app.counts.overdue, app.counts.overdue > 0)}
    {@render nav({ kind: "tomorrow" }, Sunrise, "Amanhã", app.counts.tomorrow)}
    {@render nav({ kind: "week" }, CalendarRange, "Esta semana", app.counts.week)}
  </div>

  <div class="section-head">
    <span class="eyebrow">Projetos</span>
    <button type="button" class="icon-btn sm" title="Novo projeto" onclick={() => (app.projectDialog = { mode: "create" })}>
      <Plus size={15} />
    </button>
  </div>

  <div class="projects">
    {#each app.userProjects as p (p.id)}
      <button type="button"
        class="nav"
        class:active={isActive({ kind: "project", id: p.id, tab: "tasks" })}
        onclick={() => app.navigate({ kind: "project", id: p.id, tab: "tasks" })}
      >
        <span class="pdot" style:background={p.color}></span>
        <span class="label">{p.name}</span>
        {#if app.projectCount(p.id)}<span class="count">{app.projectCount(p.id)}</span>{/if}
      </button>
    {:else}
      <button type="button" class="hint" onclick={() => (app.projectDialog = { mode: "create" })}>
        <Plus size={14} /> Criar primeiro projeto
      </button>
    {/each}
  </div>

  <div class="bottom">
    <button type="button" class="nav" onclick={() => (app.settingsOpen = true)}>
      <Settings size={16} strokeWidth={1.75} />
      <span class="label">Configurações</span>
    </button>
  </div>
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    width: var(--sidebar-w);
    height: 100%;
    padding: 14px 10px 10px;
    background: var(--bg-sidebar);
    border-right: 1px solid var(--hairline);
    flex-shrink: 0;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
    height: 32px;
    padding: 0 8px;
    margin-bottom: 14px;
    font-size: 14px;
    font-weight: 600;
    letter-spacing: -0.2px;
  }
  .logo {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 6px;
    background: var(--primary);
    color: #fff;
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .nav {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 32px;
    padding: 0 8px;
    border-radius: var(--r-sm);
    font-size: 13.5px;
    color: var(--ink-muted);
    text-align: left;
    transition: background 0.1s, color 0.1s;
  }
  .nav :global(svg) {
    color: var(--ink-subtle);
    flex-shrink: 0;
  }
  .nav:hover {
    background: var(--hover);
    color: var(--ink);
  }
  .nav.active {
    background: var(--active);
    color: var(--ink);
    font-weight: 500;
  }
  .nav.active :global(svg) {
    color: var(--primary-ink);
  }
  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    font-size: 12px;
    color: var(--ink-tertiary);
    font-variant-numeric: tabular-nums;
  }
  .count.alert {
    color: var(--danger);
    font-weight: 600;
  }
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 4px 0 8px;
    margin: 22px 0 4px;
  }
  .projects {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin: 0 -4px;
    padding: 0 4px;
  }
  .pdot {
    width: 10px;
    height: 10px;
    margin: 0 3px;
    border-radius: 3px;
    flex-shrink: 0;
  }
  .hint {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 32px;
    padding: 0 8px;
    font-size: 13px;
    color: var(--ink-tertiary);
    border-radius: var(--r-sm);
  }
  .hint:hover {
    color: var(--primary-ink);
    background: var(--hover);
  }
  .bottom {
    padding-top: 8px;
    border-top: 1px solid var(--hairline);
  }
</style>
