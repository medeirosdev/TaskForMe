import { SvelteSet } from "svelte/reactivity";
import { api } from "./api";
import { addDays, todayIso, weekRange } from "./dates";
import { toast } from "./toast.svelte";
import type { Counts, Project, Section, View } from "./types";

export const PROJECT_COLORS = [
  "#5e6ad2", // lavanda (Linear)
  "#4ea7fc",
  "#26b5ce",
  "#4cb782",
  "#f2c94c",
  "#f2994a",
  "#eb5757",
  "#e46ead",
  "#9b8afb",
  "#8a8f98",
];

export type ThemeSetting = "system" | "dark" | "light";

class AppStore {
  ready = $state(false);
  projects = $state<Project[]>([]);
  sections = $state<Section[]>([]);
  counts = $state<Counts>({ today: 0, overdue: 0, tomorrow: 0, week: 0, projects: [] });

  view = $state<View>({ kind: "today" });
  selectedTaskId = $state<number | null>(null);

  theme = $state<ThemeSetting>("system");
  closeToTray = $state(true);
  showDone = $state(false);
  dataDir = $state("");
  today = $state(todayIso());

  /** Incrementado sempre que dados mudam; as telas recarregam ao observar. */
  version = $state(0);
  /** Avisa painéis abertos que uma tarefa específica mudou em outro lugar. */
  taskSignal = $state({ id: 0, n: 0 });

  /** Tarefas concluídas nesta tela: continuam visíveis (riscadas) até sair dela. */
  recentDone = new SvelteSet<number>();

  /** Divisão recém-criada que deve abrir já em modo de renomear. */
  pendingRename = $state<number | null>(null);

  settingsOpen = $state(false);
  projectDialog = $state<{ mode: "create" } | { mode: "edit"; project: Project } | null>(null);

  inbox = $derived(this.projects.find((p) => p.isInbox));
  userProjects = $derived(this.projects.filter((p) => !p.isInbox));

  async init() {
    const b = await api.bootstrap();
    this.projects = b.projects;
    this.sections = b.sections;
    this.dataDir = b.dataDir;
    const s = new Map(b.settings);
    const theme = s.get("theme");
    if (theme === "dark" || theme === "light" || theme === "system") this.theme = theme;
    this.closeToTray = s.get("closeToTray") !== "false";
    this.showDone = s.get("showDone") === "true";
    await this.refreshCounts();
    this.ready = true;
    if (b.notice) toast(b.notice, { kind: "error", duration: 20000 });
  }

  async refreshMeta() {
    const [projects, sections] = await Promise.all([api.projects(), api.sections()]);
    this.projects = projects;
    this.sections = sections;
    // Projeto aberto foi excluído? Volta para Hoje.
    const v = this.view;
    if (v.kind === "project" && !projects.some((p) => p.id === v.id)) {
      this.view = { kind: "today" };
    }
  }

  async refreshCounts() {
    try {
      const [ws, we] = weekRange(this.today);
      this.counts = await api.counts(this.today, addDays(this.today, 1), ws, we);
    } catch {
      /* já avisado pelo api */
    }
  }

  /** Dados mudaram: telas recarregam e contadores atualizam. */
  touch() {
    this.version++;
    void this.refreshCounts();
  }

  signalTask(id: number) {
    this.taskSignal = { id, n: this.taskSignal.n + 1 };
  }

  async navigate(view: View) {
    await flushAll();
    this.selectedTaskId = null;
    this.recentDone.clear();
    this.view = view;
  }

  async openTask(id: number) {
    if (this.selectedTaskId === id) return;
    await flushAll();
    this.selectedTaskId = id;
  }

  async closeTask() {
    await flushAll();
    this.selectedTaskId = null;
  }

  project(id: number | null | undefined) {
    return this.projects.find((p) => p.id === id);
  }

  sectionsOf(projectId: number) {
    return this.sections.filter((s) => s.projectId === projectId);
  }

  projectCount(id: number) {
    return this.counts.projects.find(([pid]) => pid === id)?.[1] ?? 0;
  }

  async setTheme(t: ThemeSetting) {
    this.theme = t;
    await api.setSetting("theme", t).catch(() => {});
  }

  async setCloseToTray(v: boolean) {
    this.closeToTray = v;
    await api.setSetting("closeToTray", String(v)).catch(() => {});
  }

  async setShowDone(v: boolean) {
    this.showDone = v;
    await api.setSetting("showDone", String(v)).catch(() => {});
  }
}

export const app = new AppStore();

// ---------- salvamento pendente (autosave com debounce)

const flushers = new Set<() => Promise<void>>();

/** Editores registram aqui como salvar o que ainda está em espera. */
export function registerFlusher(fn: () => Promise<void>) {
  flushers.add(fn);
  return () => flushers.delete(fn);
}

export async function flushAll() {
  await Promise.allSettled([...flushers].map((f) => f()));
}

// ---------- exclusão com "Desfazer"

export async function deleteWithUndo(message: string, run: () => Promise<string>) {
  let token: string;
  try {
    token = await run();
  } catch {
    return false;
  }
  await app.refreshMeta().catch(() => {});
  app.touch();
  toast(message, {
    action: {
      label: "Desfazer",
      run: async () => {
        try {
          await api.restore(token);
          await app.refreshMeta();
          app.touch();
        } catch {
          /* já avisado */
        }
      },
    },
    duration: 7000,
  });
  return true;
}
