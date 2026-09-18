import { defineStore } from "pinia";

import type { TaskRow } from "@/api/tasks";

export type DropTargetKind = "calendar-day" | "tasks" | "stamps" | null;
export type DragKind = "task" | "event" | null;

export const useTaskDragStore = defineStore("taskDrag", {
  state: () => ({
    active: false,
    kind: null as DragKind,
    title: "",
    pointerX: 0,
    pointerY: 0,
    task: null as TaskRow | null,
    eventId: null as string | null,
    sourceDate: null as string | null,
    dropTarget: null as DropTargetKind,
    dropDate: null as string | null,
    suppressDateClickUntil: 0,
    suppressEventClickUntil: 0,
  }),
  actions: {
    beginTask(task: TaskRow) {
      this.active = true;
      this.kind = "task";
      this.title = task.summary;
      this.task = task;
      this.eventId = null;
      this.sourceDate = null;
      this.dropTarget = null;
      this.dropDate = null;
    },
    beginEvent(payload: { eventId: string; sourceDate: string; title: string }) {
      this.active = true;
      this.kind = "event";
      this.title = payload.title;
      this.task = null;
      this.eventId = payload.eventId;
      this.sourceDate = payload.sourceDate;
      this.dropTarget = null;
      this.dropDate = null;
    },
    setPointer(x: number, y: number) {
      this.pointerX = x;
      this.pointerY = y;
    },
    setDropTarget(target: DropTargetKind, date: string | null = null) {
      this.dropTarget = target;
      this.dropDate = date;
    },
    end() {
      this.active = false;
      this.kind = null;
      this.title = "";
      this.task = null;
      this.eventId = null;
      this.sourceDate = null;
      this.dropTarget = null;
      this.dropDate = null;
    },
    suppressDateClick(ms = 300) {
      this.suppressDateClickUntil = Date.now() + ms;
    },
    shouldSuppressDateClick() {
      return Date.now() < this.suppressDateClickUntil;
    },
    suppressEventClick(ms = 300) {
      this.suppressEventClickUntil = Date.now() + ms;
    },
    shouldSuppressEventClick() {
      return Date.now() < this.suppressEventClickUntil;
    },
  },
});
