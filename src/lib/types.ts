export interface Project {
  id: number;
  name: string;
  color: string;
  isInbox: boolean;
  position: number;
}

export interface Section {
  id: number;
  projectId: number;
  name: string;
  position: number;
  collapsed: boolean;
}

export interface Task {
  id: number;
  projectId: number;
  sectionId: number | null;
  parentId: number | null;
  title: string;
  description: string;
  dueDate: string | null;
  remindAt: string | null;
  done: boolean;
  position: number;
  createdAt: string;
  projectName: string;
  projectColor: string;
  parentTitle: string | null;
  /** Dias da semana em que repete (0 = domingo … 6 = sábado). */
  repeatDays: number[] | null;
}

export interface Note {
  id: number;
  projectId: number;
  sectionId: number | null;
  title: string;
  body: string;
  noteDate: string | null;
  remindAt: string | null;
  createdAt: string;
  updatedAt: string;
  projectName: string;
  projectColor: string;
}

export interface Link {
  id: number;
  projectId: number;
  sectionId: number | null;
  title: string;
  description: string;
  url: string;
  position: number;
  createdAt: string;
}

export interface EventItem {
  id: number;
  projectId: number;
  title: string;
  description: string;
  startDate: string;
  endDate: string | null;
  startTime: string | null;
  endTime: string | null;
  remindBefore: number | null;
  remindAt: string | null;
  createdAt: string;
  projectName: string;
  projectColor: string;
}

export interface SyncStatus {
  enabled: boolean;
  repo: string;
  last: string | null;
  error: string | null;
  running: boolean;
}

export interface Counts {
  today: number;
  overdue: number;
  tomorrow: number;
  week: number;
  nextWeek: number;
  events: number;
  projects: [number, number][];
}

export interface Agenda {
  tasks: Task[];
  notes: Note[];
  overdue: Task[];
  events: EventItem[];
}

export interface Bootstrap {
  projects: Project[];
  sections: Section[];
  settings: [string, string][];
  notice: string | null;
  dataDir: string;
}

export type ProjectTab = "tasks" | "notes" | "links" | "events";

export type FilterKind = "today" | "tomorrow" | "week" | "nextweek";

export type View =
  | { kind: FilterKind }
  | { kind: "events" }
  | { kind: "project"; id: number; tab: ProjectTab; noteId?: number };
