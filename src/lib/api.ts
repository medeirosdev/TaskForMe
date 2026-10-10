import { invoke } from "@tauri-apps/api/core";
import { toast } from "./toast.svelte";
import type { Agenda, Bootstrap, Counts, Note, Project, Section, Task } from "./types";

/** Erro já mostrado ao usuário: o handler global não precisa avisar de novo. */
export class HandledError extends Error {
  handled = true;
}

/** Toda chamada ao backend passa por aqui: erros viram aviso na tela, nunca tela branca. */
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    const msg = typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
    console.error(`[${cmd}]`, e);
    toast(msg || "Algo deu errado", { kind: "error" });
    throw new HandledError(msg);
  }
}

export interface TaskPatch {
  id: number;
  title: string;
  description: string;
  dueDate: string | null;
  remindAt: string | null;
  projectId: number;
  sectionId: number | null;
}

export interface NotePatch {
  id: number;
  title: string;
  body: string;
  noteDate: string | null;
  remindAt: string | null;
  projectId: number;
  sectionId: number | null;
}

export const api = {
  bootstrap: () => call<Bootstrap>("bootstrap"),
  appReady: () => call<void>("app_ready"),
  quit: () => call<void>("quit_app"),
  setSetting: (key: string, value: string) => call<void>("setting_set", { key, value }),
  autostartGet: () => call<boolean>("autostart_get"),
  autostartSet: (enabled: boolean) => call<void>("autostart_set", { enabled }),
  backup: () => call<string>("backup_create"),
  openDataDir: () => call<void>("open_data_dir"),

  projects: () => call<Project[]>("projects_list"),
  projectCreate: (name: string, color: string) => call<number>("project_create", { name, color }),
  projectUpdate: (id: number, name: string, color: string) => call<void>("project_update", { id, name, color }),
  projectMove: (ids: number[]) => call<void>("project_move", { ids }),
  projectDelete: (id: number) => call<string>("project_delete", { id }),
  restore: (token: string) => call<void>("restore", { token }),

  sections: () => call<Section[]>("sections_list"),
  sectionCreate: (projectId: number, name: string) => call<number>("section_create", { projectId, name }),
  sectionUpdate: (id: number, name: string, collapsed: boolean) =>
    call<void>("section_update", { id, name, collapsed }),
  sectionDelete: (id: number) => call<string>("section_delete", { id }),

  taskCreate: (input: {
    title: string;
    projectId?: number | null;
    sectionId?: number | null;
    parentId?: number | null;
    dueDate?: string | null;
  }) => call<Task>("task_create", { input }),
  taskUpdate: (input: TaskPatch) => call<Task>("task_update", { input }),
  taskSetDone: (id: number, done: boolean) => call<void>("task_set_done", { id, done }),
  taskDelete: (id: number) => call<string>("task_delete", { id }),
  task: (id: number) => call<Task>("task_get", { id }),
  tasksByProject: (projectId: number) => call<Task[]>("tasks_by_project", { projectId }),
  subtasks: (parentId: number) => call<Task[]>("subtasks", { parentId }),

  noteCreate: (projectId: number, sectionId: number | null, noteDate: string | null = null) =>
    call<Note>("note_create", { projectId, sectionId, noteDate }),
  noteUpdate: (input: NotePatch) => call<Note>("note_update", { input }),
  noteDelete: (id: number) => call<string>("note_delete", { id }),
  note: (id: number) => call<Note>("note_get", { id }),
  notesByProject: (projectId: number) => call<Note[]>("notes_by_project", { projectId }),

  agenda: (from: string, to: string, overdueBefore: string | null) =>
    call<Agenda>("agenda", { from, to, overdueBefore }),
  counts: (today: string, tomorrow: string, weekStart: string, weekEnd: string) =>
    call<Counts>("counts", { today, tomorrow, weekStart, weekEnd }),
};
