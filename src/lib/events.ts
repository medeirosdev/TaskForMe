import { diffDays, relativeDate, shortDate } from "./dates";
import type { EventItem } from "./types";

/** Opções de aviso (minutos antes do início). */
export const REMIND_OPTIONS: { value: number | null; label: string; allDayLabel?: string }[] = [
  { value: null, label: "Sem aviso" },
  { value: 0, label: "Na hora", allDayLabel: "No dia, 9h" },
  { value: 15, label: "15 min antes" },
  { value: 60, label: "1 hora antes" },
  { value: 1440, label: "1 dia antes", allDayLabel: "1 dia antes, 9h" },
  { value: 10080, label: "1 semana antes", allDayLabel: "1 semana antes, 9h" },
];

export function remindLabel(minutes: number | null, allDay: boolean): string {
  const o = REMIND_OPTIONS.find((r) => r.value === minutes);
  if (!o) return minutes === null ? "Sem aviso" : `${minutes} min antes`;
  return allDay ? (o.allDayLabel ?? o.label) : o.label;
}

export function eventEnd(e: EventItem) {
  return e.endDate ?? e.startDate;
}

export function eventDays(e: EventItem) {
  return diffDays(eventEnd(e), e.startDate) + 1;
}

export function timeRange(e: EventItem) {
  if (!e.startTime) return "Dia inteiro";
  return e.endTime ? `${e.startTime} – ${e.endTime}` : e.startTime;
}

/** "Hoje · 14:00 – 16:00", "12 out → 15 out · 4 dias". */
export function eventWhen(e: EventItem, today: string) {
  if (e.endDate) {
    const t = e.startTime ? ` · ${e.startTime}` : "";
    return `${shortDate(e.startDate, today)} → ${shortDate(e.endDate, today)} · ${eventDays(e)} dias${t}`;
  }
  return `${relativeDate(e.startDate, today)} · ${timeRange(e)}`;
}

/** Na agenda: o que mostrar em um dia específico de um evento. */
export function eventOnDay(e: EventItem, day: string) {
  if (!e.endDate) return timeRange(e);
  const n = diffDays(day, e.startDate) + 1;
  const total = eventDays(e);
  if (n === 1 && e.startTime) return `Começa às ${e.startTime} · dia 1 de ${total}`;
  if (n === total && e.endTime) return `Termina às ${e.endTime} · dia ${n} de ${total}`;
  return `Dia ${n} de ${total}`;
}

export function isPast(e: EventItem, today: string) {
  return eventEnd(e) < today;
}

export function isHappening(e: EventItem, today: string) {
  return e.startDate <= today && eventEnd(e) >= today;
}
