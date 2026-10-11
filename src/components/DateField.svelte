<script lang="ts">
  import Popover from "./Popover.svelte";
  import {
    AlarmClock, ArrowRight, Bell, Calendar, CalendarRange, ChevronLeft, ChevronRight, Sun, Sunrise, X,
  } from "../lib/icons";
  import {
    addDays, dueTone, inOneHour, iso, monthLabel, nextWeekday, parseIso, parseTime,
    relativeDate, reminderLabel, shortDate, splitDateTime,
  } from "../lib/dates";
  import { app } from "../lib/store.svelte";

  let {
    value,
    mode = "date",
    onchange,
    placeholder,
    compact = false,
    tone = true,
    clearable = true,
  }: {
    value: string | null;
    mode?: "date" | "datetime";
    onchange: (v: string | null) => void;
    placeholder?: string;
    compact?: boolean;
    tone?: boolean;
    clearable?: boolean;
  } = $props();

  let anchor: HTMLButtonElement | undefined = $state();
  let open = $state(false);

  const today = $derived(app.today);
  const datePart = $derived(value ? value.slice(0, 10) : null);
  const label = $derived(
    value ? (mode === "date" ? relativeDate(value, today) : reminderLabel(value, today)) : (placeholder ?? (mode === "date" ? "Data" : "Lembrete")),
  );
  const toneClass = $derived(tone && mode === "date" ? dueTone(value, today) : "");

  // ---- calendário
  let month = $state(new Date());
  let time = $state("09:00");
  let timeError = $state(false);

  function onOpen() {
    const base = datePart ? parseIso(datePart) : parseIso(today);
    month = new Date(base.getFullYear(), base.getMonth(), 1);
    time = value && mode === "datetime" ? splitDateTime(value)[1] : "09:00";
    timeError = false;
  }

  const cells = $derived.by(() => {
    const first = new Date(month.getFullYear(), month.getMonth(), 1);
    const offset = first.getDay(); // calendário começa no domingo
    const start = iso(new Date(first.getFullYear(), first.getMonth(), 1 - offset));
    return Array.from({ length: 42 }, (_, i) => {
      const d = addDays(start, i);
      return { d, day: Number(d.slice(8)), inMonth: parseIso(d).getMonth() === month.getMonth() };
    });
  });

  function shiftMonth(n: number) {
    month = new Date(month.getFullYear(), month.getMonth() + n, 1);
  }

  function commit(v: string | null) {
    open = false;
    if (v !== value) onchange(v);
  }

  function pickDay(d: string) {
    if (mode === "date") return commit(d);
    const t = parseTime(time);
    if (!t) {
      timeError = true;
      return;
    }
    commit(`${d}T${t}`);
  }

  function applyTime() {
    const t = parseTime(time);
    if (!t) {
      timeError = true;
      return;
    }
    commit(`${datePart ?? today}T${t}`);
  }

  const nowHour = new Date().getHours();
  const weekdayShort = (d: string) =>
    new Intl.DateTimeFormat("pt-BR", { weekday: "short" }).format(parseIso(d)).replace(".", "");
</script>

{#if compact && !value}
  <button type="button"
    class="icon-btn sm"
    bind:this={anchor}
    title={placeholder ?? (mode === "date" ? "Definir data" : "Definir lembrete")}
    onclick={(e) => {
      e.stopPropagation();
      open = !open;
      if (open) onOpen();
    }}
  >
    {#if mode === "date"}<Calendar size={15} strokeWidth={1.75} />{:else}<Bell size={15} strokeWidth={1.75} />{/if}
  </button>
{:else}
  <button type="button"
    class="chip {toneClass}"
    class:empty={!value}
    bind:this={anchor}
    onclick={(e) => {
      e.stopPropagation();
      open = !open;
      if (open) onOpen();
    }}
  >
    {#if mode === "date"}<Calendar size={13} strokeWidth={2} />{:else}<AlarmClock size={13} strokeWidth={2} />{/if}
    {label}
  </button>
{/if}

<Popover {anchor} bind:open width={264}>
  <div class="picker">
    <div class="quick">
      {#if mode === "date"}
        <button type="button" onclick={() => commit(today)}><Sun size={15} /> Hoje <em>{weekdayShort(today)}</em></button>
        <button type="button" onclick={() => commit(addDays(today, 1))}>
          <Sunrise size={15} /> Amanhã <em>{weekdayShort(addDays(today, 1))}</em>
        </button>
        <button type="button" onclick={() => commit(nextWeekday(today, 1))}>
          <CalendarRange size={15} /> Próxima segunda <em>{shortDate(nextWeekday(today, 1), today)}</em>
        </button>
        <button type="button" onclick={() => commit(addDays(today, 7))}>
          <ArrowRight size={15} /> Em uma semana <em>{shortDate(addDays(today, 7), today)}</em>
        </button>
      {:else}
        <button type="button" onclick={() => commit(inOneHour())}>
          <AlarmClock size={15} /> Em 1 hora <em>{inOneHour().slice(11)}</em>
        </button>
        {#if nowHour < 18}
          <button type="button" onclick={() => commit(`${today}T18:00`)}><Sun size={15} /> Hoje <em>18:00</em></button>
        {/if}
        <button type="button" onclick={() => commit(`${addDays(today, 1)}T09:00`)}>
          <Sunrise size={15} /> Amanhã <em>09:00</em>
        </button>
        <button type="button" onclick={() => commit(`${nextWeekday(today, 1)}T09:00`)}>
          <CalendarRange size={15} /> Segunda <em>09:00</em>
        </button>
      {/if}
    </div>

    <div class="cal">
      <div class="cal-head">
        <span>{monthLabel(month)}</span>
        <div>
          <button type="button" class="icon-btn sm" aria-label="Mês anterior" onclick={() => shiftMonth(-1)}>
            <ChevronLeft size={15} />
          </button>
          <button type="button" class="icon-btn sm" aria-label="Próximo mês" onclick={() => shiftMonth(1)}>
            <ChevronRight size={15} />
          </button>
        </div>
      </div>
      <div class="grid">
        {#each ["D", "S", "T", "Q", "Q", "S", "S"] as w, i (i)}<span class="wd">{w}</span>{/each}
        {#each cells as c (c.d)}
          <button type="button"
            class="day"
            class:out={!c.inMonth}
            class:past={c.d < today}
            class:is-today={c.d === today}
            class:selected={c.d === datePart}
            onclick={() => pickDay(c.d)}>{c.day}</button
          >
        {/each}
      </div>
    </div>

    {#if mode === "datetime"}
      <form
        class="time"
        onsubmit={(e) => {
          e.preventDefault();
          applyTime();
        }}
      >
        <AlarmClock size={15} />
        <input
          class="input"
          class:error={timeError}
          bind:value={time}
          oninput={() => (timeError = false)}
          placeholder="09:00"
          maxlength="5"
          aria-label="Horário"
        />
        <button class="btn btn-primary" type="submit">Definir</button>
      </form>
    {/if}

    {#if value && clearable}
      <div class="sep"></div>
      <button type="button" class="remove" onclick={() => commit(null)}>
        <X size={15} />
        {mode === "date" ? "Remover data" : "Remover lembrete"}
      </button>
    {/if}
  </div>
</Popover>

<style>
  .chip.empty {
    color: var(--ink-tertiary);
    border-style: dashed;
  }
  .picker {
    display: flex;
    flex-direction: column;
  }
  .quick {
    display: flex;
    flex-direction: column;
    padding-bottom: 4px;
    border-bottom: 1px solid var(--hairline);
  }
  .quick button,
  .remove {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 32px;
    padding: 0 10px;
    border-radius: var(--r-sm);
    font-size: 13px;
    color: var(--ink-muted);
  }
  .quick button :global(svg),
  .remove :global(svg) {
    color: var(--ink-subtle);
  }
  .quick button:hover,
  .remove:hover {
    background: var(--hover);
    color: var(--ink);
  }
  .quick em {
    margin-left: auto;
    font-style: normal;
    font-size: 12px;
    color: var(--ink-tertiary);
  }
  .cal {
    padding: 8px 4px 4px;
  }
  .cal-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 2px 6px 6px;
    font-size: 13px;
    font-weight: 600;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 2px;
  }
  .wd {
    text-align: center;
    font-size: 11px;
    font-weight: 600;
    color: var(--ink-tertiary);
    padding: 2px 0 4px;
  }
  .day {
    height: 30px;
    border-radius: var(--r-sm);
    font-size: 12.5px;
    font-variant-numeric: tabular-nums;
    color: var(--ink-muted);
  }
  .day:hover {
    background: var(--hover);
    color: var(--ink);
  }
  .day.past {
    color: var(--ink-tertiary);
  }
  .day.out {
    opacity: 0.4;
  }
  .day.is-today {
    color: var(--primary-ink);
    font-weight: 700;
  }
  .day.selected {
    background: var(--primary);
    color: var(--on-primary);
    font-weight: 600;
  }
  .time {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 6px 4px;
    border-top: 1px solid var(--hairline);
    color: var(--ink-subtle);
  }
  .time .input {
    height: 30px;
    flex: 1;
    font-variant-numeric: tabular-nums;
  }
  .time .input.error {
    border-color: var(--danger);
    box-shadow: 0 0 0 3px var(--danger-soft);
  }
  .sep {
    height: 1px;
    margin: 4px 0;
    background: var(--hairline);
  }
</style>
