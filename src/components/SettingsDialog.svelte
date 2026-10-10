<script lang="ts">
  import Modal from "./Modal.svelte";
  import { Monitor, Moon, Sun } from "../lib/icons";
  import { api } from "../lib/api";
  import { app, type ThemeSetting } from "../lib/store.svelte";
  import { toast } from "../lib/toast.svelte";

  let autostart = $state(false);
  api.autostartGet().then((v) => (autostart = v)).catch(() => {});

  const themes: { id: ThemeSetting; label: string; icon: typeof Sun }[] = [
    { id: "system", label: "Sistema", icon: Monitor },
    { id: "dark", label: "Escuro", icon: Moon },
    { id: "light", label: "Claro", icon: Sun },
  ];

  async function toggleAutostart() {
    const next = !autostart;
    try {
      await api.autostartSet(next);
      autostart = next;
    } catch {
      /* avisado */
    }
  }

  async function backup() {
    try {
      const path = await api.backup();
      toast(`Backup criado: ${path.split(/[\\/]/).pop()}`);
    } catch {
      /* avisado */
    }
  }

  const shortcuts: [string, string][] = [
    ["Ctrl N", "Nova tarefa (ou nota, na aba Notas)"],
    ["Ctrl 1 – 4", "Entrada, Hoje, Amanhã, Esta semana"],
    ["Esc", "Fechar painel"],
    ["Ctrl S", "Salvar nota agora (já salva sozinho)"],
  ];
</script>

{#snippet toggle(on: boolean, onclick: () => void, label: string)}
  <button type="button" class="switch" class:on role="switch" aria-checked={on} aria-label={label} {onclick}><span></span></button>
{/snippet}

<Modal title="Configurações" onclose={() => (app.settingsOpen = false)} width={480}>
  <div class="body">
    <div class="row col">
      <span class="name">Tema</span>
      <div class="themes">
        {#each themes as t (t.id)}
          <button type="button" class="theme" class:on={app.theme === t.id} onclick={() => app.setTheme(t.id)}>
            <t.icon size={16} strokeWidth={1.75} />
            {t.label}
          </button>
        {/each}
      </div>
    </div>

    <div class="row">
      <div>
        <span class="name">Manter na bandeja ao fechar</span>
        <p>Os lembretes continuam funcionando com a janela fechada.</p>
      </div>
      {@render toggle(app.closeToTray, () => app.setCloseToTray(!app.closeToTray), "Manter na bandeja")}
    </div>

    <div class="row">
      <div>
        <span class="name">Iniciar com o sistema</span>
        <p>Abre minimizado na bandeja quando você liga o computador.</p>
      </div>
      {@render toggle(autostart, toggleAutostart, "Iniciar com o sistema")}
    </div>

    <div class="row">
      <div>
        <span class="name">Seus dados</span>
        <p>Backup automático diário (os últimos 14 ficam guardados).</p>
      </div>
      <div class="btns">
        <button type="button" class="btn btn-secondary" onclick={backup}>Fazer backup</button>
        <button type="button" class="btn btn-ghost" onclick={() => api.openDataDir().catch(() => {})}>Abrir pasta</button>
      </div>
    </div>

    <div class="row col">
      <span class="name">Atalhos</span>
      <div class="keys">
        {#each shortcuts as [k, d] (k)}
          <span class="kbd">{k}</span><span class="kd">{d}</span>
        {/each}
      </div>
    </div>

    <div class="path" title={app.dataDir}>{app.dataDir}</div>
  </div>
</Modal>

<style>
  .body {
    padding: 8px 22px 18px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    padding: 14px 0;
    border-bottom: 1px solid var(--hairline);
  }
  .row.col {
    flex-direction: column;
    align-items: stretch;
    gap: 10px;
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
  .themes {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }
  .theme {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    height: 38px;
    border-radius: var(--r-md);
    border: 1px solid var(--hairline);
    font-size: 13px;
    color: var(--ink-subtle);
  }
  .theme:hover {
    color: var(--ink);
    background: var(--hover);
  }
  .theme.on {
    color: var(--ink);
    border-color: var(--primary);
    background: var(--primary-soft);
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
  .btns {
    display: flex;
    gap: 6px;
    flex-shrink: 0;
  }
  .keys {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 8px 12px;
    align-items: center;
  }
  .keys .kbd {
    justify-self: start;
    padding: 0 6px;
  }
  .kd {
    font-size: 12.5px;
    color: var(--ink-subtle);
  }
  .path {
    margin-top: 12px;
    font-family: var(--mono);
    font-size: 11px;
    color: var(--ink-tertiary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    user-select: text;
  }
</style>
