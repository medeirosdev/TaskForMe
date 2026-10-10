<script lang="ts">
  import { onDestroy } from "svelte";
  import DateField from "./DateField.svelte";
  import ProjectPicker from "./ProjectPicker.svelte";
  import Menu from "./Menu.svelte";
  import { Check, CircleAlert, Ellipsis, Trash } from "../lib/icons";
  import { api } from "../lib/api";
  import { timeAgo } from "../lib/dates";
  import { app, deleteWithUndo, registerFlusher } from "../lib/store.svelte";
  import type { Note } from "../lib/types";

  let { noteId, ondeleted }: { noteId: number; ondeleted: () => void } = $props();

  let note = $state<Note | null>(null);
  let title = $state("");
  let body = $state("");
  let noteDate = $state<string | null>(null);
  let remindAt = $state<string | null>(null);
  let projectId = $state(0);
  let sectionId = $state<number | null>(null);

  let rev = 0;
  let savedRev = 0;
  let status = $state<"saved" | "dirty" | "saving" | "error">("saved");
  let timer: ReturnType<typeof setTimeout> | undefined;
  let saving: Promise<void> | null = null;

  let menuAnchor: HTMLButtonElement | undefined = $state();
  let menuOpen = $state(false);
  let titleInput: HTMLInputElement | undefined = $state();

  // noteId é fixo por instância (o pai usa {#key}).
  // svelte-ignore state_referenced_locally
  api
    .note(noteId)
    .then((n) => {
      note = n;
      title = n.title;
      body = n.body;
      noteDate = n.noteDate;
      remindAt = n.remindAt;
      projectId = n.projectId;
      sectionId = n.sectionId;
      if (!n.title && !n.body) queueMicrotask(() => titleInput?.focus());
    })
    .catch(() => {});

  function edited() {
    rev++;
    status = "dirty";
    clearTimeout(timer);
    timer = setTimeout(save, 450);
  }

  async function save(): Promise<void> {
    clearTimeout(timer);
    if (!note || rev === savedRev) return;
    if (saving) {
      await saving;
      return save();
    }
    const target = rev;
    status = "saving";
    saving = (async () => {
      try {
        const n = await api.noteUpdate({ id: noteId, title, body, noteDate, remindAt, projectId, sectionId });
        note = n;
        savedRev = target;
        status = rev === savedRev ? "saved" : "dirty";
        app.touch();
      } catch {
        status = "error";
        // Tenta de novo sozinho: o texto continua na tela, nada se perde.
        timer = setTimeout(save, 3000);
      }
    })();
    await saving;
    saving = null;
    if (rev !== savedRev && (status as string) !== "error") timer = setTimeout(save, 450);
  }

  const unregister = registerFlusher(save);
  onDestroy(() => {
    unregister();
    // Nota criada e abandonada sem nada escrito não fica poluindo a lista.
    const empty = note && !title.trim() && !body.trim() && !noteDate && !remindAt;
    if (empty) {
      clearTimeout(timer);
      api.noteDelete(noteId).then(() => app.touch()).catch(() => {});
    } else {
      void save();
    }
  });

  function setField(fn: () => void) {
    fn();
    edited();
    void save();
  }
</script>

<div class="editor">
  {#if note}
    <div class="meta">
      <ProjectPicker
        {projectId}
        {sectionId}
        onchange={async (p, s) => {
          const moved = p !== projectId;
          setField(() => {
            projectId = p;
            sectionId = s;
          });
          // Nota mudou de projeto: acompanha ela até lá.
          if (moved) {
            await save();
            await app.navigate({ kind: "project", id: p, tab: "notes", noteId });
          }
        }}
      />
      <DateField value={noteDate} onchange={(v) => setField(() => (noteDate = v))} placeholder="Data" tone={false} />
      <DateField value={remindAt} mode="datetime" onchange={(v) => setField(() => (remindAt = v))} />

      <span class="status" class:error={status === "error"}>
        {#if status === "error"}
          <CircleAlert size={13} /> Erro ao salvar — tentando de novo
        {:else if status === "saved"}
          <Check size={13} /> Salvo · {timeAgo(note.updatedAt)}
        {:else}
          Salvando…
        {/if}
      </span>

      <button type="button" class="icon-btn" bind:this={menuAnchor} onclick={() => (menuOpen = !menuOpen)} aria-label="Opções">
        <Ellipsis size={16} />
      </button>
    </div>

    <input
      class="title"
      bind:this={titleInput}
      bind:value={title}
      oninput={edited}
      placeholder="Sem título"
      maxlength="500"
      onkeydown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          (e.currentTarget.nextElementSibling as HTMLElement | null)?.focus();
        }
      }}
    />
    <textarea
      class="body"
      bind:value={body}
      oninput={edited}
      onblur={save}
      placeholder="Escreva aqui…"
      spellcheck="true"
      onkeydown={(e) => {
        if (e.key === "Tab") {
          e.preventDefault();
          const el = e.currentTarget;
          el.setRangeText("\t", el.selectionStart, el.selectionEnd, "end");
          body = el.value;
          edited();
        }
      }}
    ></textarea>
  {/if}
</div>

<svelte:window
  onkeydown={(e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
      e.preventDefault();
      void save();
    }
  }}
/>

<Menu
  anchor={menuAnchor}
  bind:open={menuOpen}
  items={[
    {
      label: "Excluir nota",
      icon: Trash,
      danger: true,
      onclick: async () => {
        clearTimeout(timer);
        await save();
        const ok = await deleteWithUndo("Nota excluída", () => api.noteDelete(noteId));
        if (ok) ondeleted();
      },
    },
  ]}
/>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    height: 100%;
    max-width: 820px;
    margin: 0 auto;
    padding: 20px 40px 0;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    padding-bottom: 14px;
  }
  .status {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    color: var(--ink-tertiary);
    white-space: nowrap;
  }
  .status.error {
    color: var(--danger);
  }
  .title {
    width: 100%;
    border: 0;
    outline: none;
    background: transparent;
    padding: 6px 0 10px;
    font-size: 26px;
    font-weight: 600;
    letter-spacing: -0.5px;
    line-height: 1.25;
  }
  .title::placeholder {
    color: var(--ink-tertiary);
  }
  .body {
    flex: 1;
    width: 100%;
    min-height: 200px;
    padding: 4px 0 48px;
    border: 0;
    outline: none;
    resize: none;
    background: transparent;
    font-size: 15px;
    line-height: 1.75;
    color: var(--ink-muted);
    tab-size: 4;
  }
  .body:focus {
    color: var(--ink);
  }
</style>
