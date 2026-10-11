<script lang="ts">
  import Modal from "./Modal.svelte";
  import DateField from "./DateField.svelte";
  import ProjectPicker from "./ProjectPicker.svelte";
  import { Bell, Trash } from "../lib/icons";
  import { api } from "../lib/api";
  import { addDays, parseTime } from "../lib/dates";
  import { REMIND_OPTIONS } from "../lib/events";
  import { autosize, focusOnMount } from "../lib/portal";
  import { app, deleteWithUndo } from "../lib/store.svelte";
  import { toast } from "../lib/toast.svelte";

  const dialog = app.eventDialog!;
  const editing = dialog.event ?? null;

  let title = $state(editing?.title ?? "");
  let description = $state(editing?.description ?? "");
  let startDate = $state(editing?.startDate ?? dialog.date ?? app.today);
  let endDate = $state<string | null>(editing?.endDate ?? null);
  let period = $state(!!editing?.endDate);
  let allDay = $state(editing ? !editing.startTime : true);
  let startTime = $state(editing?.startTime ?? "09:00");
  let endTime = $state(editing?.endTime ?? "");
  let remindBefore = $state<number | null>(editing ? editing.remindBefore : 1440);
  let projectId = $state(editing?.projectId ?? dialog.projectId);
  let busy = $state(false);
  let timeError = $state<"start" | "end" | null>(null);

  const options = $derived(allDay ? REMIND_OPTIONS.filter((o) => o.value !== 15 && o.value !== 60) : REMIND_OPTIONS);

  // Aviso de 15 min/1 h não faz sentido em evento de dia inteiro.
  $effect(() => {
    if (allDay && (remindBefore === 15 || remindBefore === 60)) remindBefore = 0;
  });

  function togglePeriod() {
    period = !period;
    if (period && (!endDate || endDate <= startDate)) endDate = addDays(startDate, 1);
  }

  function close() {
    app.eventDialog = null;
  }

  function normalizeTime(which: "start" | "end") {
    const raw = which === "start" ? startTime : endTime;
    if (!raw.trim()) {
      if (which === "end") endTime = "";
      return true;
    }
    const t = parseTime(raw);
    if (!t) {
      timeError = which;
      return false;
    }
    if (which === "start") startTime = t;
    else endTime = t;
    return true;
  }

  async function submit() {
    if (busy) return;
    if (!title.trim()) {
      toast("Dê um nome ao evento", { kind: "error" });
      return;
    }
    if (!allDay && (!normalizeTime("start") || !normalizeTime("end"))) return;
    if (!allDay && !startTime) {
      timeError = "start";
      return;
    }
    if (!allDay && !period && endTime && endTime <= startTime) {
      timeError = "end";
      toast("A hora de término precisa ser depois da hora de início", { kind: "error" });
      return;
    }
    busy = true;
    try {
      await api.eventSave({
        id: editing?.id ?? null,
        projectId,
        title,
        description,
        startDate,
        endDate: period ? endDate : null,
        startTime: allDay ? null : startTime,
        endTime: allDay || !endTime ? null : endTime,
        remindBefore,
      });
      app.touch();
      if (!editing) toast("Evento criado");
      close();
    } catch {
      busy = false;
    }
  }

  async function remove() {
    if (!editing) return;
    close();
    await deleteWithUndo(`Evento "${editing.title}" excluído`, () => api.eventDelete(editing.id));
  }
</script>

<Modal title={editing ? "Editar evento" : "Novo evento"} onclose={close} width={500}>
  <form
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    <input
      class="title"
      bind:value={title}
      placeholder="Nome do evento"
      maxlength="500"
      use:focusOnMount
    />

    <div class="row">
      <span class="label">Data</span>
      <div class="ctl">
        <DateField
          value={startDate}
          tone={false}
          clearable={false}
          onchange={(v) => {
            if (!v) return;
            startDate = v;
            if (endDate && endDate <= v) endDate = addDays(v, 1);
          }}
        />
        {#if period}
          <span class="sep">até</span>
          <DateField
            value={endDate}
            tone={false}
            placeholder="Fim"
            onchange={(v) => {
              if (v && v <= startDate) {
                toast("O fim precisa ser depois do início", { kind: "error" });
                return;
              }
              endDate = v;
              if (!v) period = false;
            }}
          />
        {/if}
        <button type="button" class="toggle" class:on={period} onclick={togglePeriod}>
          Período
        </button>
      </div>
    </div>

    <div class="row">
      <span class="label">Horário</span>
      <div class="ctl">
        <button type="button" class="toggle" class:on={allDay} onclick={() => (allDay = !allDay)}>Dia inteiro</button>
        {#if !allDay}
          <input
            class="input time"
            class:error={timeError === "start"}
            bind:value={startTime}
            oninput={() => (timeError = null)}
            onblur={() => normalizeTime("start")}
            placeholder="09:00"
            maxlength="5"
            aria-label="Hora de início"
          />
          <span class="sep">até</span>
          <input
            class="input time"
            class:error={timeError === "end"}
            bind:value={endTime}
            oninput={() => (timeError = null)}
            onblur={() => normalizeTime("end")}
            placeholder="opcional"
            maxlength="5"
            aria-label="Hora de término"
          />
        {/if}
      </div>
    </div>

    <div class="row top">
      <span class="label"><Bell size={13} /> Aviso</span>
      <div class="ctl wrap">
        {#each options as o (o.value)}
          <button
            type="button"
            class="pill"
            class:on={remindBefore === o.value}
            onclick={() => (remindBefore = o.value)}
          >
            {allDay ? (o.allDayLabel ?? o.label) : o.label}
          </button>
        {/each}
      </div>
    </div>

    <div class="row">
      <span class="label">Projeto</span>
      <div class="ctl">
        <ProjectPicker {projectId} sections={false} onchange={(p) => (projectId = p)} />
      </div>
    </div>

    <textarea
      class="input desc"
      rows="2"
      bind:value={description}
      use:autosize={description}
      maxlength="5000"
      placeholder="Local, link da reunião, observações… (opcional)"
    ></textarea>

    <div class="actions">
      {#if editing}
        <button type="button" class="btn btn-danger" onclick={remove}><Trash size={15} /> Excluir</button>
      {/if}
      <span class="grow"></span>
      <button type="button" class="btn btn-ghost" onclick={close}>Cancelar</button>
      <button type="submit" class="btn btn-primary" disabled={busy}>{editing ? "Salvar" : "Criar evento"}</button>
    </div>
  </form>
</Modal>

<style>
  form {
    padding: 8px 22px 20px;
  }
  .title {
    width: 100%;
    border: 0;
    outline: none;
    background: transparent;
    padding: 4px 0 14px;
    font-size: 19px;
    font-weight: 600;
    letter-spacing: -0.3px;
    border-bottom: 1px solid var(--hairline);
    margin-bottom: 8px;
  }
  .title::placeholder {
    color: var(--ink-tertiary);
  }
  .row {
    display: grid;
    grid-template-columns: 78px 1fr;
    align-items: center;
    gap: 10px;
    min-height: 40px;
    padding: 4px 0;
  }
  .row.top {
    align-items: start;
  }
  .row.top .label {
    padding-top: 6px;
  }
  .label {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 12.5px;
    color: var(--ink-subtle);
  }
  .ctl {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .ctl.wrap {
    flex-wrap: wrap;
    gap: 6px;
  }
  .sep {
    font-size: 12.5px;
    color: var(--ink-tertiary);
  }
  .toggle,
  .pill {
    height: 26px;
    padding: 0 10px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 500;
    color: var(--ink-subtle);
    border: 1px solid var(--hairline);
    white-space: nowrap;
    transition: background 0.12s, color 0.12s, border-color 0.12s;
  }
  .toggle:hover,
  .pill:hover {
    color: var(--ink);
    background: var(--hover);
  }
  .toggle.on,
  .pill.on {
    color: var(--primary-ink);
    border-color: color-mix(in srgb, var(--primary) 60%, transparent);
    background: var(--primary-soft);
  }
  .time {
    width: 82px;
    height: 30px;
    text-align: center;
    font-variant-numeric: tabular-nums;
  }
  .time.error {
    border-color: var(--danger);
    box-shadow: 0 0 0 3px var(--danger-soft);
  }
  .desc {
    height: auto;
    min-height: 60px;
    margin-top: 10px;
    padding: 8px 12px;
    line-height: 1.5;
    resize: none;
    overflow: hidden;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 16px;
  }
  .grow {
    flex: 1;
  }
</style>
