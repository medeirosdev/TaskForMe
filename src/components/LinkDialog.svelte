<script lang="ts">
  import Modal from "./Modal.svelte";
  import ProjectPicker from "./ProjectPicker.svelte";
  import { api } from "../lib/api";
  import { autosize, focusOnMount } from "../lib/portal";
  import { app } from "../lib/store.svelte";
  import { toast } from "../lib/toast.svelte";

  const dialog = app.linkDialog!;
  const editing = dialog.link ?? null;

  let url = $state(editing?.url ?? "");
  let title = $state(editing?.title ?? "");
  let description = $state(editing?.description ?? "");
  let projectId = $state(editing?.projectId ?? dialog.projectId);
  let sectionId = $state<number | null>(editing ? editing.sectionId : dialog.sectionId);
  let busy = $state(false);

  /** Sugestão de nome enquanto o campo está vazio: o domínio do endereço. */
  const suggestion = $derived.by(() => {
    const raw = url.trim();
    if (!raw) return "Ex.: Portal do aluno";
    try {
      const u = new URL(/^https?:\/\//i.test(raw) ? raw : `https://${raw}`);
      return u.hostname.replace(/^www\./, "") || "Nome do link";
    } catch {
      return "Nome do link";
    }
  });

  function close() {
    app.linkDialog = null;
  }

  async function submit() {
    if (!url.trim() || busy) return;
    busy = true;
    try {
      await api.linkSave({ id: editing?.id ?? null, projectId, sectionId, title, description, url });
      app.touch();
      if (!editing) toast("Link adicionado");
      close();
    } catch {
      busy = false; // erro já exibido; o formulário continua preenchido
    }
  }
</script>

<Modal title={editing ? "Editar link" : "Novo link"} onclose={close} width={480}>
  <form
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    <label class="field">
      <span>Endereço</span>
      <input
        class="input"
        bind:value={url}
        placeholder="https://site.com.br"
        maxlength="4000"
        use:focusOnMount
        spellcheck="false"
        autocomplete="off"
      />
    </label>

    <label class="field">
      <span>Nome</span>
      <input class="input" bind:value={title} placeholder={suggestion} maxlength="500" />
    </label>

    <label class="field">
      <span>Descrição <em>opcional</em></span>
      <textarea
        class="input area"
        rows="2"
        bind:value={description}
        use:autosize={description}
        maxlength="2000"
        placeholder="Para que serve, login usado, observações…"
        onkeydown={(e) => {
          // Enter salva; Shift+Enter quebra linha.
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            submit();
          }
        }}
      ></textarea>
    </label>

    <div class="field">
      <span>Onde fica</span>
      <div>
        <ProjectPicker
          {projectId}
          {sectionId}
          onchange={(p, s) => {
            projectId = p;
            sectionId = s;
          }}
        />
      </div>
    </div>

    <div class="actions">
      <button type="button" class="btn btn-ghost" onclick={close}>Cancelar</button>
      <button type="submit" class="btn btn-primary" disabled={!url.trim() || busy}>
        {editing ? "Salvar" : "Adicionar link"}
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
    margin-bottom: 16px;
  }
  .field > span {
    font-size: 12.5px;
    font-weight: 500;
    color: var(--ink-subtle);
  }
  .field em {
    font-style: normal;
    font-weight: 400;
    color: var(--ink-tertiary);
    margin-left: 4px;
  }
  .area {
    height: auto;
    min-height: 60px;
    padding: 8px 12px;
    line-height: 1.5;
    resize: none;
    overflow: hidden;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
</style>
