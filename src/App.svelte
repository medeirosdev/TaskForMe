<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import Sidebar from "./components/Sidebar.svelte";
  import AgendaView from "./components/AgendaView.svelte";
  import ProjectView from "./components/ProjectView.svelte";
  import TaskDetail from "./components/TaskDetail.svelte";
  import Toasts from "./components/Toasts.svelte";
  import ProjectDialog from "./components/ProjectDialog.svelte";
  import SettingsDialog from "./components/SettingsDialog.svelte";
  import { api } from "./lib/api";
  import { todayIso } from "./lib/dates";
  import { app, flushAll } from "./lib/store.svelte";
  import { toast } from "./lib/toast.svelte";

  let systemDark = $state(matchMedia("(prefers-color-scheme: dark)").matches);
  let bootError = $state<string | null>(null);

  // ---- tema
  $effect(() => {
    const resolved = app.theme === "system" ? (systemDark ? "dark" : "light") : app.theme;
    document.documentElement.dataset.theme = resolved;
  });

  onMount(() => {
    const mq = matchMedia("(prefers-color-scheme: dark)");
    const onScheme = (e: MediaQueryListEvent) => (systemDark = e.matches);
    mq.addEventListener("change", onScheme);

    // ---- erros inesperados viram aviso, nunca tela branca
    const onError = (e: ErrorEvent) => {
      console.error(e.error ?? e.message);
      toast("Ocorreu um erro inesperado. Seus dados estão salvos.", { kind: "error" });
    };
    const onRejection = (e: PromiseRejectionEvent) => {
      if ((e.reason as { handled?: boolean })?.handled) {
        e.preventDefault();
        return;
      }
      console.error(e.reason);
      toast("Ocorreu um erro inesperado. Seus dados estão salvos.", { kind: "error" });
    };
    window.addEventListener("error", onError);
    window.addEventListener("unhandledrejection", onRejection);

    // ---- virada do dia (app aberto à meia-noite, ou PC que voltou da suspensão)
    const tick = setInterval(() => {
      const t = todayIso();
      if (t !== app.today) {
        app.today = t;
        app.touch();
      }
    }, 30_000);
    const onFocus = () => {
      if (todayIso() !== app.today) app.today = todayIso();
      app.touch();
    };
    window.addEventListener("focus", onFocus);

    // ---- eventos do backend
    const unlisten: Promise<() => void>[] = [];
    unlisten.push(listen("reminders-fired", () => app.touch()));
    unlisten.push(
      listen("request-quit", async () => {
        await flushAll();
        await api.quit().catch(() => {});
      }),
    );

    app
      .init()
      .catch((e) => (bootError = String(e?.message ?? e)))
      .finally(() => {
        // Mostra a janela assim que os dados carregaram. Não esperar por
        // requestAnimationFrame/tick: com a janela escondida o WebKit não os dispara.
        api.appReady().catch(() => {});
      });

    return () => {
      mq.removeEventListener("change", onScheme);
      window.removeEventListener("error", onError);
      window.removeEventListener("unhandledrejection", onRejection);
      window.removeEventListener("focus", onFocus);
      clearInterval(tick);
      unlisten.forEach((p) => p.then((f) => f()).catch(() => {}));
    };
  });

  function onKey(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    if (e.key === "Escape" && app.selectedTaskId !== null && !app.settingsOpen && !app.projectDialog) {
      app.closeTask();
      return;
    }
    if (!mod) return;
    const k = e.key.toLowerCase();
    if (k === "n") {
      e.preventDefault();
      const v = app.view;
      if (v.kind === "project" && v.tab === "notes") {
        window.dispatchEvent(new CustomEvent("tfm:new-note"));
      } else {
        document.querySelector<HTMLInputElement>("[data-quickadd]")?.focus();
      }
    }
    const views = [
      app.inbox ? { kind: "project" as const, id: app.inbox.id, tab: "tasks" as const } : null,
      { kind: "today" as const },
      { kind: "tomorrow" as const },
      { kind: "week" as const },
    ];
    const n = Number(k);
    if (n >= 1 && n <= 4 && views[n - 1]) {
      e.preventDefault();
      app.navigate(views[n - 1]!);
    }
  }

  const viewKey = $derived(
    app.view.kind === "project" ? `p${app.view.id}-${app.view.tab}-${app.view.noteId ?? ""}` : app.view.kind,
  );
  const currentProject = $derived(app.view.kind === "project" ? app.project(app.view.id) : undefined);
</script>

<svelte:window onkeydown={onKey} />

<svelte:boundary onerror={(e) => console.error(e)}>
  {#if bootError}
    <div class="fatal">
      <h1>Não foi possível abrir seus dados</h1>
      <p>{bootError}</p>
      <button type="button" class="btn btn-primary" onclick={() => location.reload()}>Tentar de novo</button>
    </div>
  {:else if app.ready}
    <div class="shell" class:with-detail={app.selectedTaskId !== null}>
      <Sidebar />
      <main>
        {#key viewKey}
          {#if app.view.kind === "project"}
            {#if currentProject}
              <ProjectView project={currentProject} tab={app.view.tab} noteId={app.view.noteId} />
            {/if}
          {:else}
            <AgendaView kind={app.view.kind} />
          {/if}
        {/key}
      </main>
      {#if app.selectedTaskId !== null}
        <div class="detail-slot">
          {#key app.selectedTaskId}
            <TaskDetail taskId={app.selectedTaskId} />
          {/key}
        </div>
      {/if}
    </div>
  {/if}

  {#snippet failed(error, reset)}
    <div class="fatal">
      <h1>Algo deu errado nesta tela</h1>
      <p>Seus dados estão salvos. {String((error as Error)?.message ?? error)}</p>
      <button type="button" class="btn btn-primary" onclick={reset}>Recarregar tela</button>
    </div>
  {/snippet}
</svelte:boundary>

{#if app.projectDialog}<ProjectDialog />{/if}
{#if app.settingsOpen}<SettingsDialog />{/if}
<Toasts />

<style>
  .shell {
    display: flex;
    height: 100%;
    position: relative;
  }
  main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
  .detail-slot {
    height: 100%;
    flex-shrink: 0;
    animation: slide 0.2s var(--ease);
  }
  /* Janela estreita: o painel flutua sobre a lista em vez de espremê-la. */
  @media (max-width: 1100px) {
    .detail-slot {
      position: absolute;
      top: 0;
      right: 0;
      bottom: 0;
      z-index: 50;
      box-shadow: var(--shadow-panel);
    }
  }
  @keyframes slide {
    from {
      opacity: 0;
      transform: translateX(16px);
    }
  }
  .fatal {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    height: 100%;
    padding: 32px;
    text-align: center;
  }
  .fatal h1 {
    margin: 0;
    font-size: 18px;
  }
  .fatal p {
    margin: 0 0 8px;
    max-width: 480px;
    color: var(--ink-subtle);
  }
</style>
