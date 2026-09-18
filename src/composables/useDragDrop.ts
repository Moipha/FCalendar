import { useQueryClient } from "@tanstack/vue-query";
import { type InjectionKey, onMounted, onUnmounted, ref } from "vue";

import { createEvent, deleteEvent, getEvent, updateEvent } from "@/api/events";
import { createTask, deleteTask, updateTask, type TaskRow } from "@/api/tasks";
import {
  applyDropHighlight,
  clearDropHighlights,
  DRAG_THRESHOLD,
  findDropTarget,
} from "@/lib/dragDrop";
import { shiftEventToDate } from "@/lib/eventReschedule";
import { useTaskDragStore, type DropTargetKind } from "@/stores/taskDrag";

type PendingSession =
  | { kind: "task"; task: TaskRow }
  | { kind: "event"; eventId: string; sourceDate: string; title: string };

function cursorForSession(session: PendingSession, target: DropTargetKind) {
  if (session.kind === "task") {
    if (target === "calendar-day") {
      return session.task.isStamp ? "copy" : "grabbing";
    }
    if (target === "tasks" || target === "stamps") {
      return "grabbing";
    }
    return session.task.isStamp ? "copy" : "grab";
  }

  if (target === "calendar-day" || target === "tasks" || target === "stamps") {
    return "grabbing";
  }
  return "grab";
}

export function useDragDrop(calendarId: () => string) {
  const dragStore = useTaskDragStore();
  const queryClient = useQueryClient();

  const pending = ref<PendingSession | null>(null);
  const startX = ref(0);
  const startY = ref(0);
  const dragging = ref(false);
  const pointerId = ref<number | null>(null);

  async function dropTaskToCalendar(task: TaskRow, date: string) {
    const calId = calendarId();
    if (!calId) {
      return;
    }
    await createEvent({
      calendarId: calId,
      summary: task.summary,
      description: task.description ?? null,
      allDay: true,
      dtstart: date,
      dtend: date,
      rrule: null,
    });
    if (!task.isStamp) {
      await deleteTask(task.id);
      await queryClient.invalidateQueries({ queryKey: ["tasks"] });
    }
    await queryClient.invalidateQueries({ queryKey: ["events"] });
  }

  async function dropTaskToList(task: TaskRow, isStamp: boolean) {
    if (task.isStamp === isStamp) {
      return;
    }
    await updateTask(task.id, {
      summary: task.summary,
      description: task.description ?? null,
      isStamp,
    });
    await queryClient.invalidateQueries({ queryKey: ["tasks"] });
  }

  async function dropEventToCalendar(eventId: string, sourceDate: string, targetDate: string) {
    if (targetDate === sourceDate) {
      return;
    }
    const event = await getEvent(eventId);
    const input = shiftEventToDate(event, sourceDate, targetDate);
    await updateEvent(eventId, input);
    await queryClient.invalidateQueries({ queryKey: ["events"] });
  }

  async function dropEventToInbox(eventId: string, isStamp: boolean) {
    const event = await getEvent(eventId);
    await createTask({
      summary: event.summary,
      description: event.description ?? null,
      isStamp,
    });
    await deleteEvent(eventId);
    await queryClient.invalidateQueries({ queryKey: ["tasks"] });
    await queryClient.invalidateQueries({ queryKey: ["events"] });
  }

  async function commitTaskDrop(task: TaskRow, kind: DropTargetKind, date: string | null) {
    if (kind === "calendar-day" && date) {
      await dropTaskToCalendar(task, date);
      dragStore.suppressDateClick();
      return;
    }
    if (kind === "tasks") {
      await dropTaskToList(task, false);
      return;
    }
    if (kind === "stamps") {
      await dropTaskToList(task, true);
    }
  }

  async function commitEventDrop(
    eventId: string,
    sourceDate: string,
    kind: DropTargetKind,
    date: string | null,
  ) {
    if (kind === "calendar-day" && date) {
      await dropEventToCalendar(eventId, sourceDate, date);
      dragStore.suppressDateClick();
      return;
    }
    if (kind === "tasks") {
      await dropEventToInbox(eventId, false);
      return;
    }
    if (kind === "stamps") {
      await dropEventToInbox(eventId, true);
    }
  }

  function beginDragSession(session: PendingSession) {
    dragging.value = true;
    document.body.classList.add("fc-task-dragging");
    if (session.kind === "task") {
      dragStore.beginTask(session.task);
      return;
    }
    dragStore.beginEvent({
      eventId: session.eventId,
      sourceDate: session.sourceDate,
      title: session.title,
    });
  }

  function onPointerMove(event: PointerEvent) {
    if (pointerId.value !== event.pointerId || !pending.value) {
      return;
    }
    const dx = event.clientX - startX.value;
    const dy = event.clientY - startY.value;
    if (!dragging.value && Math.hypot(dx, dy) < DRAG_THRESHOLD) {
      return;
    }
    if (!dragging.value) {
      beginDragSession(pending.value);
    }
    event.preventDefault();
    dragStore.setPointer(event.clientX, event.clientY);
    const { kind, date } = findDropTarget(event.clientX, event.clientY);
    dragStore.setDropTarget(kind, date);
    applyDropHighlight(kind, date);
    document.body.style.cursor = cursorForSession(pending.value, kind);
  }

  async function onPointerUp(event: PointerEvent) {
    if (pointerId.value !== event.pointerId) {
      return;
    }
    const session = pending.value;
    const wasDragging = dragging.value;
    const target = dragStore.dropTarget;
    const date = dragStore.dropDate;

    pointerId.value = null;
    pending.value = null;
    dragging.value = false;
    if (wasDragging && session?.kind === "event") {
      dragStore.suppressEventClick();
    }
    dragStore.end();
    clearDropHighlights();
    document.body.classList.remove("fc-task-dragging");
    document.body.style.cursor = "";

    if (!wasDragging || !session || !target) {
      return;
    }

    try {
      if (session.kind === "task") {
        await commitTaskDrop(session.task, target, date);
        return;
      }
      await commitEventDrop(session.eventId, session.sourceDate, target, date);
    } catch (error) {
      console.error("drag drop failed", error);
    }
  }

  function onPointerCancel(event: PointerEvent) {
    void onPointerUp(event);
  }

  function startTaskDrag(task: TaskRow, event: PointerEvent) {
    if (event.button !== 0) {
      return;
    }
    pending.value = { kind: "task", task };
    startX.value = event.clientX;
    startY.value = event.clientY;
    pointerId.value = event.pointerId;
    dragging.value = false;
  }

  function startEventDrag(
    payload: { eventId: string; sourceDate: string; title: string },
    event: PointerEvent,
  ) {
    if (event.button !== 0) {
      return;
    }
    pending.value = {
      kind: "event",
      eventId: payload.eventId,
      sourceDate: payload.sourceDate,
      title: payload.title,
    };
    startX.value = event.clientX;
    startY.value = event.clientY;
    pointerId.value = event.pointerId;
    dragging.value = false;
  }

  onMounted(() => {
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    window.addEventListener("pointercancel", onPointerCancel);
  });

  onUnmounted(() => {
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
    window.removeEventListener("pointercancel", onPointerCancel);
    clearDropHighlights();
    document.body.classList.remove("fc-task-dragging");
    document.body.style.cursor = "";
  });

  return {
    startTaskDrag,
    startEventDrag,
  };
}

export type DragDropContext = ReturnType<typeof useDragDrop>;

export const DRAG_DROP_KEY: InjectionKey<DragDropContext> = Symbol("dragDrop");
