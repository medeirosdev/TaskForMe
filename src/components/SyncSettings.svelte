<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { Check, CircleAlert } from "../lib/icons";
  import { api } from "../lib/api";
  import { relativeDate } from "../lib/dates";
  import { app } from "../lib/store.svelte";
  import { toast } from "../lib/toast.svelte";
  import type { SyncStatus } from "../lib/types";

  let status = $state<SyncStatus | null>(null);
  let repo = $state("");
  let busy = $state(false);
  let confirmRestore = $state(false);
  let restoring = $state(false);

  onMount(() => {
    api
      .syncStatus()
      .then((s) => {
        status = s;
        repo = s.repo;
      })
      .catch(() => {});
    const un = listen<SyncStatus>("sync-status", (e) => apply(e.payload));
    return () => {
      un.then((f) => f()).catch(() => {});
    };
  });

  /** Atualiza o status; se o app trocou o endereço (ex.: HTTPS → SSH) e o
   *  campo não tinha edição pendente, mostra o endereço novo. */
  function apply(next: SyncStatus) {
    const clean = repo.trim() === (status?.repo ?? "");
    status = next;
    if (clean) repo = next.repo;
  }

  const dirty = $derived(status !== null && repo.trim() !== status.repo);
  const running = $derived(busy || !!status?.running);

  function lastLabel(last: string) {
    return `${relativeDate(last.slice(0, 10), app.today)} às ${last.slice(11, 16)}`;
  }

  async function save(enabled: boolean) {
    try {
      status = await api.syncConfigure(repo, enabled);
      repo = status.repo;
      return true;
    } catch {
      return false;
    }
  }

  async function sendNow() {
    if (running) return;
    // Salva o endereço editado sem mudar o liga/desliga do envio automático.
    if (dirty && !(await save(status?.enabled ?? false))) return;
    busy = true;
    try {
      const msg = await api.syncNow();
      toast(msg);
    } catch {
      /* erro já exibido e registrado no status */
    } finally {
      busy = false;
      const next = await api.syncStatus().catch(() => null);
      if (next) apply(next);
    }
  }

  async function restore() {
    if (restoring) return;
    if (dirty && !(await save(status?.enabled ?? false))) return;
    restoring = true;
    try {
      const msg = await api.syncRestore();
      toast(msg, { duration: 8000 });
      // Recarrega a interface inteira com os dados restaurados.
      setTimeout(() => location.reload(), 1200);
    } catch {
      restoring = false;
      confirmRestore = false;
    }
  }

  async function toggle() {
    if (!status) return;
    const next = !status.enabled;
    if (next && !repo.trim()) {
      toast("Informe o endereço do repositório primeiro", { kind: "error" });
      return;
    }
    if (await save(next)) {
      if (next) void sendNow();
    }
  }
</script>

<div class="sync">
  <div class="head">
    <div>
      <span class="name">Backup no GitHub</span>
      <p>Envia seus dados para um repositório privado ao abrir o computador e uma vez por dia.</p>
    </div>
    <button
      type="button"
      class="switch"
      class:on={status?.enabled}
      role="switch"
      aria-checked={!!status?.enabled}
      aria-label="Backup no GitHub"
      onclick={toggle}><span></span></button
    >
  </div>

  <form
    class="repo"
    onsubmit={(e) => {
      e.preventDefault();
      sendNow();
    }}
  >
    <input
      class="input"
      bind:value={repo}
      placeholder="git@github.com:usuario/taskforme-dados.git"
      spellcheck="false"
      autocomplete="off"
      maxlength="500"
    />
    <button type="submit" class="btn btn-secondary" disabled={running || !repo.trim()}>
      {running ? "Enviando…" : dirty ? "Salvar e enviar" : "Enviar agora"}
    </button>
  </form>

  {#if status}
    {#if status.error}
      <div class="state error"><CircleAlert size={14} /> {status.error}</div>
    {:else if status.last}
      <div class="state ok"><Check size={14} /> Último envio: {lastLabel(status.last)}</div>
    {/if}
  {/if}
  {#if status?.repo}
    {#if confirmRestore}
      <div class="confirm">
        <p>
          Isso <b>substitui todos os dados atuais</b> pelos do último backup no GitHub. Antes, uma cópia dos dados
          atuais é guardada nos backups locais.
        </p>
        <div class="confirm-actions">
          <button type="button" class="btn btn-ghost" disabled={restoring} onclick={() => (confirmRestore = false)}>
            Cancelar
          </button>
          <button type="button" class="btn btn-danger-solid" disabled={restoring} onclick={restore}>
            {restoring ? "Restaurando…" : "Restaurar agora"}
          </button>
        </div>
      </div>
    {:else}
      <button type="button" class="restore-link" disabled={running} onclick={() => (confirmRestore = true)}>
        Restaurar do GitHub…
      </button>
    {/if}
  {/if}

  <p class="hint">
    Use um repositório <b>privado</b>. Precisa do Git instalado e de acesso ao repositório (chave SSH ou login
    salvo do Git).
  </p>
</div>

<style>
  .sync {
    padding: 14px 0;
    border-bottom: 1px solid var(--hairline);
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
  }
  .name {
    font-size: 13.5px;
    font-weight: 500;
  }
  p {
    margin: 2px 0 0;
    font-size: 12.5px;
    color: var(--ink-subtle);
  }
  .repo {
    display: flex;
    gap: 8px;
    margin-top: 12px;
  }
  .repo .input {
    flex: 1;
    min-width: 0;
    font-family: var(--mono);
    font-size: 12.5px;
  }
  .repo .btn {
    height: 34px;
  }
  .state {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin-top: 10px;
    font-size: 12.5px;
    line-height: 1.45;
  }
  .state :global(svg) {
    flex-shrink: 0;
    margin-top: 2px;
  }
  .state.ok {
    color: var(--success);
  }
  .state.error {
    color: var(--danger);
  }
  .restore-link {
    margin-top: 10px;
    font-size: 12.5px;
    color: var(--ink-subtle);
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .restore-link:hover {
    color: var(--ink);
  }
  .confirm {
    margin-top: 12px;
    padding: 12px 14px;
    border-radius: var(--r-md);
    border: 1px solid color-mix(in srgb, var(--danger) 45%, transparent);
    background: var(--danger-soft);
  }
  .confirm p {
    margin: 0;
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--ink-muted);
  }
  .confirm b {
    color: var(--ink);
  }
  .confirm-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 10px;
  }
  .btn-danger-solid {
    background: var(--danger);
    color: #fff;
  }
  .btn-danger-solid:hover {
    filter: brightness(1.08);
  }
  .hint {
    margin-top: 10px;
    font-size: 12px;
    color: var(--ink-tertiary);
  }
  .hint b {
    color: var(--ink-subtle);
    font-weight: 600;
  }
  .switch {
    position: relative;
    width: 36px;
    height: 20px;
    border-radius: 20px;
    background: var(--surface-2);
    border: 1px solid var(--hairline-strong);
    flex-shrink: 0;
    transition: background 0.15s, border-color 0.15s;
  }
  .switch span {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--ink-subtle);
    transition: transform 0.18s var(--ease), background 0.15s;
  }
  .switch.on {
    background: var(--primary);
    border-color: var(--primary);
  }
  .switch.on span {
    transform: translateX(16px);
    background: #fff;
  }
</style>
