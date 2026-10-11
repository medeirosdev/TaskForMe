import { api } from "./api";
import { dayName, shortDate } from "./dates";
import { app } from "./store.svelte";
import { toast } from "./toast.svelte";
import type { Task } from "./types";

/**
 * Marca/desmarca uma tarefa com atualização otimista. `all` é a lista local
 * da tela, para refletir na hora as subtarefas concluídas junto com a mãe.
 */
export async function toggleTask(task: Task, done: boolean, all: Task[] = []) {
  const children = done ? all.filter((t) => t.parentId === task.id && !t.done) : [];
  task.done = done;
  children.forEach((c) => (c.done = true));
  if (done) app.recentDone.add(task.id);

  let nextDate: string | null = null;
  try {
    nextDate = (await api.taskSetDone(task.id, done)).nextDate;
  } catch {
    task.done = !done;
    children.forEach((c) => (c.done = false));
    return;
  }

  app.signalTask(task.id);
  if (task.parentId) app.signalTask(task.parentId);
  app.touch();

  if (done) {
    const title = task.title.length > 40 ? `${task.title.slice(0, 40)}…` : task.title;
    const next = nextDate ? ` · próxima: ${dayName(nextDate, app.today)}, ${shortDate(nextDate, app.today)}` : "";
    toast(`“${title}” concluída${next}`, {
      action: {
        label: "Desfazer",
        run: async () => {
          try {
            await api.taskSetDone(task.id, false);
            await Promise.all(children.map((c) => api.taskSetDone(c.id, false)));
          } catch {
            return;
          }
          task.done = false;
          children.forEach((c) => (c.done = false));
          app.signalTask(task.id);
          app.touch();
        },
      },
    });
  }
}

/** Ordem de exibição: abertas e recém-concluídas pela posição; concluídas antigas no fim. */
export function byDisplayOrder(a: Task, b: Task) {
  const ha = a.done && !app.recentDone.has(a.id) ? 1 : 0;
  const hb = b.done && !app.recentDone.has(b.id) ? 1 : 0;
  return ha - hb || a.position - b.position || a.id - b.id;
}

export function isVisible(t: Task) {
  return !t.done || app.showDone || app.recentDone.has(t.id);
}
