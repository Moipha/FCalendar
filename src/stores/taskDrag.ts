import { defineStore } from "pinia";

import type { TaskRow } from "@/api/tasks";

export type DropTargetKind = "calendar-day" | "tasks" | "stamps" | null;

export const useTaskDragStore = defineStore("taskDrag", {
  state: () => ({
    active: false,
    task: null as TaskRow | null,
    dropTarget: null as DropTargetKind,
    dropDate: null as string | null,
    suppressDateClickUntil: 0,
  }),
  actions: {
    begin(task: TaskRow) {
      this.active = true;
      this.task = task;
      this.dropTarget = null;
      this.dropDate = null;
    },
    setDropTarget(target: DropTargetKind, date: string | null = null) {
      this.dropTarget = target;
      this.dropDate = date;
    },
    end() {
      this.active = false;
      this.task = null;
      this.dropTarget = null;
      this.dropDate = null;
    },
    suppressDateClick(ms = 300) {
      this.suppressDateClickUntil = Date.now() + ms;
    },
    shouldSuppressDateClick() {
      return Date.now() < this.suppressDateClickUntil;
    },
  },
});
