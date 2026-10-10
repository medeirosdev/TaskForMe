<script lang="ts">
  import DateField from "./DateField.svelte";
  import ProjectPicker from "./ProjectPicker.svelte";
  import { Plus } from "../lib/icons";
  import { api } from "../lib/api";
  import { app } from "../lib/store.svelte";
  import type { Task } from "../lib/types";

  let {
    projectId = null,
    sectionId = null,
    dueDate = null,
    placeholder = "Adicionar tarefa",
    autofocus = false,
    pickProject = false,
    inline = false,
    onclose,
    oncreated,
  }: {
    projectId?: number | null;
    sectionId?: number | null;
    dueDate?: string | null;
    placeholder?: string;
    autofocus?: boolean;
    pickProject?: boolean;
    inline?: boolean;
    onclose?: () => void;
    oncreated?: (t: Task) => void;
  } = $props();

  let input: HTMLInputElement | undefined = $state();
  let title = $state("");
  let busy = $state(false);
  // Valores escolhidos no próprio campo; voltam ao padrão da tela após criar.
  let date = $state<string | null | undefined>(undefined);
  let project = $state<{ p: number; s: number | null } | undefined>(undefined);

  const effDate = $derived(date === undefined ? dueDate : date);
  const effProject = $derived(project?.p ?? projectId ?? app.inbox?.id ?? null);
  const effSection = $derived(project ? project.s : sectionId);

  let form: HTMLFormElement | undefined = $state();

  $effect(() => {
    if (autofocus) input?.focus();
  });

  // Versão inline fecha ao clicar fora (ignorando popovers abertos por ela).
  $effect(() => {
    if (!inline) return;
    const onDown = (e: PointerEvent) => {
      const t = e.target as HTMLElement;
      if (form?.contains(t) || t.closest?.(".pop")) return;
      if (!title.trim()) onclose?.();
    };
    document.addEventListener("pointerdown", onDown, true);
    return () => document.removeEventListener("pointerdown", onDown, true);
  });

  async function submit() {
    const t = title.trim();
    if (!t || busy) return;
    busy = true;
    try {
      const task = await api.taskCreate({
        title: t,
        projectId: effProject,
        sectionId: effSection,
        dueDate: effDate,
      });
      title = "";
      date = undefined;
      project = undefined;
      app.touch();
      oncreated?.(task);
    } catch {
      /* erro já exibido; o texto continua no campo para tentar de novo */
    } finally {
      busy = false;
      input?.focus();
    }
  }

  export function focus() {
    input?.focus();
  }
</script>

<form
  bind:this={form}
  class="qa"
  class:inline
  onsubmit={(e) => {
    e.preventDefault();
    submit();
  }}
>
  <span class="plus"><Plus size={16} strokeWidth={2} /></span>
  <input
    bind:this={input}
    bind:value={title}
    {placeholder}
    maxlength="500"
    data-quickadd
    onkeydown={(e) => {
      if (e.key === "Escape") {
        title = "";
        input?.blur();
        onclose?.();
      }
    }}
  />
  <div class="tools">
    {#if pickProject}
      <ProjectPicker projectId={effProject} sectionId={effSection} onchange={(p, s) => {
          project = { p, s };
          input?.focus();
        }} />
    {/if}
    <DateField
      value={effDate}
      onchange={(v) => {
        date = v;
        input?.focus();
      }}
      compact={!effDate}
    />
  </div>
</form>

<style>
  .qa {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 44px;
    padding: 0 8px 0 12px;
    border-radius: var(--r-md);
    border: 1px solid var(--hairline);
    background: var(--surface-1);
    box-shadow: var(--edge-highlight);
    transition: border-color 0.15s, box-shadow 0.15s;
  }
  .qa:focus-within {
    border-color: var(--hairline-strong);
    box-shadow: 0 0 0 3px var(--primary-soft);
  }
  .qa.inline {
    height: 38px;
    margin: 2px 0 4px;
  }
  .plus {
    display: grid;
    place-items: center;
    color: var(--primary-ink);
  }
  input {
    flex: 1;
    min-width: 0;
    height: 100%;
    border: 0;
    outline: none;
    background: transparent;
    font-size: 14px;
  }
  input::placeholder {
    color: var(--ink-tertiary);
  }
  .tools {
    display: flex;
    align-items: center;
    gap: 6px;
  }
</style>
