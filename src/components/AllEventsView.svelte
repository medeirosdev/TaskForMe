<script lang="ts">
  import EventsPane from "./EventsPane.svelte";
  import { CalendarClock } from "../lib/icons";
  import { app } from "../lib/store.svelte";
</script>

<header class="head">
  <div class="title">
    <CalendarClock size={20} strokeWidth={1.75} />
    <h1>Todos os eventos</h1>
    <span class="sub">de todos os projetos</span>
  </div>
  {#if app.counts.events > 0}
    <span class="total">{app.counts.events} {app.counts.events === 1 ? "próximo" : "próximos"}</span>
  {/if}
</header>

<div class="content">
  <div class="inner"><EventsPane /></div>
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
  }
  .total {
    font-size: 12.5px;
    color: var(--ink-tertiary);
  }
  .content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .inner {
    max-width: 860px;
    margin: 0 auto;
    padding: 20px 32px 0;
  }
</style>
