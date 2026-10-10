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

export interface Counts {
  today: number;
  overdue: number;
  tomorrow: number;
  week: number;
  projects: [number, number][];
}

export interface Agenda {
  tasks: Task[];
  notes: Note[];
  overdue: Task[];
}

export interface Bootstrap {
  projects: Project[];
  sections: Section[];
  settings: [string, string][];
  notice: string | null;
  dataDir: string;
}

export type FilterKind = "today" | "tomorrow" | "week";

export type View =
  | { kind: FilterKind }
  | { kind: "project"; id: number; tab: "tasks" | "notes"; noteId?: number };
