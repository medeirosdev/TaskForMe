// Datas são sempre "locais" no formato YYYY-MM-DD (sem fuso), igual ao banco.

const pad = (n: number) => String(n).padStart(2, "0");

export function iso(d: Date): string {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

export function parseIso(s: string): Date {
  const [y, m, d] = s.slice(0, 10).split("-").map(Number);
  return new Date(y, (m || 1) - 1, d || 1);
}

export function todayIso(): string {
  return iso(new Date());
}

export function addDays(s: string, n: number): string {
  const d = parseIso(s);
  d.setDate(d.getDate() + n);
  return iso(d);
}

export function diffDays(a: string, b: string): number {
  return Math.round((parseIso(a).getTime() - parseIso(b).getTime()) / 86_400_000);
}

/** Semana de segunda a domingo. */
export function weekRange(s: string): [string, string] {
  const d = parseIso(s);
  const offset = (d.getDay() + 6) % 7;
  const start = addDays(s, -offset);
  return [start, addDays(start, 6)];
}

export function nextWeekday(s: string, weekday: number): string {
  const d = parseIso(s);
  let n = (weekday - d.getDay() + 7) % 7;
  if (n === 0) n = 7;
  return addDays(s, n);
}

const fmtShort = new Intl.DateTimeFormat("pt-BR", { day: "numeric", month: "short" });
const fmtShortYear = new Intl.DateTimeFormat("pt-BR", { day: "numeric", month: "short", year: "numeric" });
const fmtWeekday = new Intl.DateTimeFormat("pt-BR", { weekday: "long" });
const fmtLong = new Intl.DateTimeFormat("pt-BR", { weekday: "long", day: "numeric", month: "long" });
const fmtMonth = new Intl.DateTimeFormat("pt-BR", { month: "long", year: "numeric" });

const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
const clean = (s: string) => s.replace(".", "").replace(" de ", " ");

export function shortDate(s: string, today = todayIso()): string {
  const d = parseIso(s);
  return clean(d.getFullYear() === parseIso(today).getFullYear() ? fmtShort.format(d) : fmtShortYear.format(d));
}

/** "Hoje", "Amanhã", "Ontem", "Sexta", "12 out". */
export function relativeDate(s: string, today = todayIso()): string {
  const diff = diffDays(s, today);
  if (diff === 0) return "Hoje";
  if (diff === 1) return "Amanhã";
  if (diff === -1) return "Ontem";
  if (diff > 1 && diff < 7) return cap(fmtWeekday.format(parseIso(s)).replace("-feira", ""));
  return shortDate(s, today);
}

export function longDate(s: string): string {
  return cap(fmtLong.format(parseIso(s)));
}

export function monthLabel(d: Date): string {
  return cap(fmtMonth.format(d));
}

export function dueTone(s: string | null, today = todayIso()): "" | "overdue" | "today" | "tomorrow" {
  if (!s) return "";
  const diff = diffDays(s, today);
  if (diff < 0) return "overdue";
  if (diff === 0) return "today";
  if (diff === 1) return "tomorrow";
  return "";
}

// ---------- data + hora (lembretes): "YYYY-MM-DDTHH:MM"

export function nowDateTime(): string {
  const d = new Date();
  return `${iso(d)}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function inOneHour(): string {
  const d = new Date(Date.now() + 60 * 60 * 1000);
  return `${iso(d)}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function splitDateTime(s: string): [string, string] {
  return [s.slice(0, 10), s.slice(11, 16)];
}

export function reminderLabel(s: string, today = todayIso()): string {
  const [d, t] = splitDateTime(s);
  return `${relativeDate(d, today)}, ${t}`;
}

/** Aceita "9", "930", "9:30", "09h30", "21" → "HH:MM" ou null. */
export function parseTime(input: string): string | null {
  const s = input.trim().toLowerCase().replace("h", ":");
  let m = s.match(/^(\d{1,2}):?(\d{2})?$/);
  if (!m && /^\d{3,4}$/.test(s)) m = [s, s.slice(0, s.length - 2), s.slice(-2)] as RegExpMatchArray;
  if (!m) return null;
  const h = Number(m[1]);
  const min = Number(m[2] ?? 0);
  if (h > 23 || min > 59) return null;
  return `${pad(h)}:${pad(min)}`;
}

export function timeAgo(dt: string): string {
  const d = new Date(dt.replace(" ", "T"));
  const mins = Math.round((Date.now() - d.getTime()) / 60000);
  if (mins < 1) return "agora";
  if (mins < 60) return `há ${mins} min`;
  const h = Math.round(mins / 60);
  if (h < 24) return `há ${h} h`;
  return shortDate(iso(d));
}
