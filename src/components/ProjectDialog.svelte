<script lang="ts">
  import Modal from "./Modal.svelte";
  import { Check } from "../lib/icons";
  import { api } from "../lib/api";
  import { app, PROJECT_COLORS } from "../lib/store.svelte";

  const dialog = app.projectDialog!;
  const editing = dialog.mode === "edit" ? dialog.project : null;

  let name = $state(editing?.name ?? "");
  let color = $state(editing?.color ?? PROJECT_COLORS[app.userProjects.length % PROJECT_COLORS.length]);
  let busy = $state(false);

  function close() {
    app.projectDialog = null;
  }

  async function submit() {
    const n = name.trim();
    if (!n || busy) return;
    busy = true;
    try {
      if (editing) {
        await api.projectUpdate(editing.id, n, color);
        await app.refreshMeta();
      } else {
        const id = await api.projectCreate(n, color);
        await app.refreshMeta();
        await app.navigate({ kind: "project", id, tab: "tasks" });
      }
      app.touch();
      close();
    } catch {
      busy = false;
    }
  }
</script>

<Modal title={editing ? "Editar projeto" : "Novo projeto"} onclose={close}>
  <form
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    <label class="field">
      <span>Nome</span>
      <!-- svelte-ignore a11y_autofocus -->
      <input class="input" bind:value={name} maxlength="120" placeholder="Ex.: Trabalho, Faculdade, Casa" autofocus />
    </label>

    <div class="field">
      <span>Cor</span>
      <div class="colors">
        {#each PROJECT_COLORS as c (c)}
          <button
            type="button"
            class="swatch"
            class:on={c === color}
            style:background={c}
            aria-label="Cor {c}"
            onclick={() => (color = c)}
          >
            {#if c === color}<Check size={13} strokeWidth={3} />{/if}
          </button>
        {/each}
      </div>
    </div>

    <div class="actions">
      <button type="button" class="btn btn-ghost" onclick={close}>Cancelar</button>
      <button type="submit" class="btn btn-primary" disabled={!name.trim() || busy}>
        {editing ? "Salvar" : "Criar projeto"}
      </button>
    </div>
  </form>
</Modal>

<style>
  form {
    padding: 12px 22px 20px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-bottom: 18px;
  }
  .field > span {
    font-size: 12.5px;
    font-weight: 500;
    color: var(--ink-subtle);
  }
  .colors {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .swatch {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    color: #fff;
    transition: transform 0.12s var(--ease), box-shadow 0.12s;
  }
  .swatch:hover {
    transform: scale(1.08);
  }
  .swatch.on {
    box-shadow: 0 0 0 2px var(--surface-pop), 0 0 0 4px var(--ink-subtle);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
</style>
