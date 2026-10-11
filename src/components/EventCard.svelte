<script lang="ts">
  import { Bell, Inbox } from "../lib/icons";
  import { parseIso } from "../lib/dates";
  import { eventOnDay, eventWhen, isHappening, isPast, remindLabel } from "../lib/events";
  import { app } from "../lib/store.svelte";
  import type { EventItem } from "../lib/types";

  /** `day` preenchido = modo compacto da agenda (Hoje/Amanhã/Semana). */
  let { event, day, showProject = false }: { event: EventItem; day?: string; showProject?: boolean } = $props();

  const fmtMonth = new Intl.DateTimeFormat("pt-BR", { month: "short" });
  const month = $derived(fmtMonth.format(parseIso(event.startDate)).replace(".", ""));
  const happening = $derived(isHappening(event, app.today));
  const past = $derived(isPast(event, app.today));
  const isInbox = $derived(app.project(event.projectId)?.isInbox);

  function open() {
    app.eventDialog = { projectId: event.projectId, event };
  }
</script>

{#if day}
  <button type="button" class="row" onclick={open}>
    <span class="bar" style:background={event.projectColor}></span>
    <span class="r-title">{event.title}</span>
    <span class="r-when">{eventOnDay(event, day)}</span>
    {#if event.remindAt}<span class="r-bell" title="Com aviso"><Bell size={12} /></span>{/if}
    {#if showProject}
      <span class="r-proj">
        {#if isInbox}<Inbox size={12} />{:else}<span class="dot" style:background={event.projectColor}></span>{/if}
        {event.projectName}
      </span>
    {/if}
  </button>
{:else}
  <button type="button" class="card" class:past onclick={open}>
    <span class="date" style:--c={event.projectColor}>
      <span class="d">{Number(event.startDate.slice(8))}</span>
      <span class="m">{month}</span>
    </span>
    <span class="body">
      <span class="top">
        <span class="title">{event.title}</span>
        {#if happening}<span class="badge">{event.endDate ? "Acontecendo" : "Hoje"}</span>{/if}
      </span>
      <span class="when">
        {eventWhen(event, app.today)}
        {#if showProject}
          <span class="proj">
            · {#if isInbox}<Inbox size={11} />{:else}<span class="dot" style:background={event.projectColor}></span>{/if}
            {event.projectName}
          </span>
        {/if}
      </span>
      {#if event.description}<span class="desc">{event.description}</span>{/if}
    </span>
    {#if event.remindBefore !== null}
      <span class="remind" title="Aviso">
        <Bell size={12} />
        {remindLabel(event.remindBefore, !event.startTime)}
      </span>
    {/if}
  </button>
{/if}

<style>
  /* ----- cartão (aba Eventos) */
  .card {
    display: flex;
    align-items: flex-start;
    gap: 14px;
    width: 100%;
    padding: 12px 14px;
    border-radius: var(--r-lg);
    background: var(--surface-1);
    border: 1px solid var(--hairline);
    box-shadow: var(--edge-highlight);
    text-align: left;
    transition: border-color 0.12s, background 0.12s;
  }
  .card:hover {
    background: var(--surface-2);
    border-color: var(--hairline-strong);
  }
  .card.past {
    opacity: 0.6;
  }
  .date {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    width: 46px;
    height: 48px;
    flex-shrink: 0;
    border-radius: var(--r-md);
    background: color-mix(in srgb, var(--c) 14%, transparent);
    color: var(--c);
  }
  .d {
    font-size: 18px;
    font-weight: 700;
    line-height: 1.1;
    font-variant-numeric: tabular-nums;
  }
  .m {
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 3px;
    flex: 1;
    min-width: 0;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .title {
    font-size: 14px;
    font-weight: 500;
    color: var(--ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .badge {
    flex-shrink: 0;
    padding: 1px 7px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 600;
    color: var(--primary-ink);
    background: var(--primary-soft);
  }
  .when {
    font-size: 12.5px;
    color: var(--ink-subtle);
  }
  .proj {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .proj .dot {
    width: 7px;
    height: 7px;
  }
  .desc {
    font-size: 12.5px;
    line-height: 1.45;
    color: var(--ink-tertiary);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .remind {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
    font-size: 11.5px;
    color: var(--ink-subtle);
    white-space: nowrap;
    padding-top: 2px;
  }

  /* ----- linha compacta (agenda) */
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    min-height: 40px;
    padding: 6px 12px 6px 14px;
    border-radius: var(--r-md);
    text-align: left;
  }
  .row:hover {
    background: var(--hover);
  }
  .bar {
    width: 4px;
    height: 20px;
    margin: 0 9px 0 11px;
    border-radius: 4px;
    flex-shrink: 0;
  }
  .r-title {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 500;
  }
  .r-when {
    flex: 1;
    min-width: 0;
    font-size: 12.5px;
    color: var(--ink-subtle);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .r-bell {
    display: grid;
    color: var(--ink-tertiary);
  }
  .r-proj {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: var(--ink-subtle);
    white-space: nowrap;
  }
  .r-proj .dot {
    width: 7px;
    height: 7px;
  }
</style>
