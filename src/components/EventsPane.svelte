<script lang="ts">
  import EventCard from "./EventCard.svelte";
  import EmptyState from "./EmptyState.svelte";
  import { CalendarClock, ChevronRight, Plus } from "../lib/icons";
  import { api } from "../lib/api";
  import { monthLabel, parseIso } from "../lib/dates";
  import { isHappening, isPast } from "../lib/events";
  import { app } from "../lib/store.svelte";
  import type { EventItem, Project } from "../lib/types";

  /** Sem `project`: eventos de todos os projetos. */
  let { project = null }: { project?: Project | null } = $props();

  let events = $state<EventItem[]>([]);
  let loaded = $state(false);
  let showPast = $state(false);

  $effect(() => {
    const id = project?.id ?? null;
    void app.version;
    (id === null ? api.eventsAll() : api.eventsByProject(id))
      .then((r) => {
        if (id !== (project?.id ?? null)) return;
        events = r;
        loaded = true;
      })
      .catch(() => {});
  });

  // Ctrl+N cria evento.
  $effect(() => {
    const onNew = () => add();
    window.addEventListener("tfm:new-event", onNew);
    return () => window.removeEventListener("tfm:new-event", onNew);
  });

  function add() {
    const pid = project?.id ?? app.inbox?.id;
    if (pid !== undefined) app.eventDialog = { projectId: pid };
  }

  const happening = $derived(events.filter((e) => isHappening(e, app.today) && e.startDate < app.today));
  const upcoming = $derived(events.filter((e) => !isPast(e, app.today) && !happening.includes(e)));
  // Passados: o mais recente primeiro.
  const past = $derived(events.filter((e) => isPast(e, app.today)).reverse());

  /** Próximos agrupados por mês de início. */
  const byMonth = $derived.by(() => {
    const groups: { key: string; label: string; items: EventItem[] }[] = [];
    for (const e of upcoming) {
      const key = e.startDate.slice(0, 7);
      let g = groups.find((x) => x.key === key);
      if (!g) {
        g = { key, label: monthLabel(parseIso(e.startDate)), items: [] };
        groups.push(g);
      }
      g.items.push(e);
    }
    return groups;
  });
</script>

<div class="wrap">
  <button type="button" class="new" onclick={add}><Plus size={15} strokeWidth={2} /> Novo evento</button>

  {#if happening.length}
    <div class="month">Acontecendo agora</div>
    <div class="list">
      {#each happening as e (e.id)}
        <EventCard event={e} showProject={!project} />
      {/each}
    </div>
  {/if}

  {#each byMonth as g (g.key)}
    <div class="month">{g.label}</div>
    <div class="list">
      {#each g.items as e (e.id)}
        <EventCard event={e} showProject={!project} />
      {/each}
    </div>
  {/each}

  {#if past.length}
    <button type="button" class="past-toggle" onclick={() => (showPast = !showPast)}>
      <span class="caret" class:open={showPast}><ChevronRight size={14} /></span>
      Eventos passados <span class="n">{past.length}</span>
    </button>
    {#if showPast}
      <div class="list">
        {#each past as e (e.id)}
          <EventCard event={e} showProject={!project} />
        {/each}
      </div>
    {/if}
  {/if}

  {#if loaded && events.length === 0}
    <EmptyState icon={CalendarClock} title="Nenhum evento ainda">
      Provas, reuniões, viagens, prazos: cadastre eventos de um dia ou de um período e receba um aviso antes.
    </EmptyState>
  {:else if loaded && upcoming.length === 0 && happening.length === 0}
    <p class="none">Nenhum evento pela frente.</p>
  {/if}
</div>

<style>
  .wrap {
    display: flex;
    flex-direction: column;
    padding-bottom: 48px;
  }
  .new {
    align-self: flex-start;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 14px;
    border-radius: var(--r-md);
    font-size: 13px;
    font-weight: 500;
    color: var(--primary-ink);
    border: 1px dashed var(--hairline-strong);
  }
  .new:hover {
    background: var(--primary-soft);
    border-color: var(--primary);
  }
  .month {
    margin: 22px 0 0;
    padding-bottom: 6px;
    font-size: 13.5px;
    font-weight: 600;
    border-bottom: 1px solid var(--hairline);
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 10px;
  }
  .past-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    align-self: flex-start;
    margin-top: 22px;
    padding: 4px 8px 4px 2px;
    font-size: 13px;
    font-weight: 600;
    color: var(--ink-subtle);
    border-radius: var(--r-sm);
  }
  .past-toggle:hover {
    color: var(--ink);
  }
  .caret {
    display: grid;
    transition: transform 0.15s var(--ease);
  }
  .caret.open {
    transform: rotate(90deg);
  }
  .n {
    font-weight: 400;
    color: var(--ink-tertiary);
  }
  .none {
    margin: 16px 0 0;
    font-size: 13px;
    color: var(--ink-tertiary);
  }
</style>
