export interface ToastItem {
  id: number;
  message: string;
  kind: "info" | "error";
  action?: { label: string; run: () => void };
}

export const toasts = $state<ToastItem[]>([]);
let seq = 0;

export function dismiss(id: number) {
  const i = toasts.findIndex((t) => t.id === id);
  if (i >= 0) toasts.splice(i, 1);
}

export function toast(
  message: string,
  opts: { kind?: ToastItem["kind"]; action?: ToastItem["action"]; duration?: number } = {},
) {
  const id = ++seq;
  const kind = opts.kind ?? "info";
  toasts.push({ id, message, kind, action: opts.action });
  if (toasts.length > 4) toasts.shift();
  setTimeout(() => dismiss(id), opts.duration ?? (kind === "error" ? 8000 : 5000));
  return id;
}
