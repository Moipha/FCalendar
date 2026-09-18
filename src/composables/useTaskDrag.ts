import { useQueryClient } from "@tanstack/vue-query";
import { onMounted, onUnmounted, ref } from "vue";

import { createEvent } from "@/api/events";
import { deleteTask, updateTask, type TaskRow } from "@/api/tasks";
import { useTaskDragStore, type DropTargetKind } from "@/stores/taskDrag";

const DRAG_THRESHOLD = 5;
const DROP_TARGET_CLASS = "fc-drop-target";
const DAY_TARGET_CLASS = "fc-day-drop-target";

function findDropTarget(x: number, y: number): { kind: DropTargetKind; date: string | null } {
  const elements = document.elementsFromPoint(x, y);
  for (const element of elements) {
    if (!(element instanceof HTMLElement)) {
      continue;
    }
    const zone = element.closest("[data-drop-zone]") as HTMLElement | null;
    if (zone) {
      const value = zone.dataset.dropZone;
      if (value === "tasks" || value === "stamps") {
        return { kind: value, date: null };
      }
    }
    const day = element.closest(".sx__month-grid-day[data-date]") as HTMLElement | null;
    if (day?.dataset.date) {
      return { kind: "calendar-day", date: day.dataset.date };
    }
  }
  return { kind: null, date: null };
}

function clearDropHighlights() {
  document.querySelectorAll(`.${DROP_TARGET_CLASS}, .${DAY_TARGET_CLASS}`).forEach((node) => {
    node.classList.remove(DROP_TARGET_CLASS, DAY_TARGET_CLASS);
  });
}

function applyDropHighlight(kind: DropTargetKind, date: string | null) {
  clearDropHighlights();
  if (kind === "calendar-day" && date) {
    document
      .querySelector(`.sx__month-grid-day[data-date="${date}"]`)
      ?.classList.add(DAY_TARGET_CLASS);
    return;
  }
  if (kind === "tasks" || kind === "stamps") {
    document.querySelector(`[data-drop-zone="${kind}"]`)?.classList.add(DROP_TARGET_CLASS);
  }
}

function cursorForTask(task: TaskRow, kind: DropTargetKind) {
  if (kind === "calendar-day") {
    return task.isStamp ? "copy" : "grabbing";
  }
  if (kind === "tasks" || kind === "stamps") {
    return "grabbing";
  }
  return task.isStamp ? "copy" : "grab";
}

export function useTaskDrag(calendarId: () => string) {
  const dragStore = useTaskDragStore();
  const queryClient = useQueryClient();

  const pendingTask = ref<TaskRow | null>(null);
  const startX = ref(0);
  const startY = ref(0);
  const dragging = ref(false);
  const pointerId = ref<number | null>(null);

  async function dropToCalendar(task: TaskRow, date: string) {
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

  async function dropToList(task: TaskRow, isStamp: boolean) {
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

  async function commitDrop(task: TaskRow, kind: DropTargetKind, date: string | null) {
    if (kind === "calendar-day" && date) {
      await dropToCalendar(task, date);
      dragStore.suppressDateClick();
      return;
    }
    if (kind === "tasks") {
      await dropToList(task, false);
      return;
    }
    if (kind === "stamps") {
      await dropToList(task, true);
    }
  }

  function onPointerMove(event: PointerEvent) {
    if (pointerId.value !== event.pointerId || !pendingTask.value) {
      return;
    }
    const dx = event.clientX - startX.value;
    const dy = event.clientY - startY.value;
    if (!dragging.value && Math.hypot(dx, dy) < DRAG_THRESHOLD) {
      return;
    }
    if (!dragging.value) {
      dragging.value = true;
      dragStore.begin(pendingTask.value);
      document.body.classList.add("fc-task-dragging");
    }
    event.preventDefault();
    const { kind, date } = findDropTarget(event.clientX, event.clientY);
    dragStore.setDropTarget(kind, date);
    applyDropHighlight(kind, date);
    document.body.style.cursor = cursorForTask(pendingTask.value, kind);
  }

  async function onPointerUp(event: PointerEvent) {
    if (pointerId.value !== event.pointerId) {
      return;
    }
    const task = pendingTask.value;
    const wasDragging = dragging.value;
    const target = dragStore.dropTarget;
    const date = dragStore.dropDate;

    pointerId.value = null;
    pendingTask.value = null;
    dragging.value = false;
    dragStore.end();
    clearDropHighlights();
    document.body.classList.remove("fc-task-dragging");
    document.body.style.cursor = "";

    if (wasDragging && task && target) {
      try {
        await commitDrop(task, target, date);
      } catch (error) {
        console.error("task drop failed", error);
      }
    }
  }

  function onPointerCancel(event: PointerEvent) {
    void onPointerUp(event);
  }

  function startPointerDrag(task: TaskRow, event: PointerEvent) {
    if (event.button !== 0) {
      return;
    }
    pendingTask.value = task;
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
    startPointerDrag,
    isDragging: dragging,
  };
}
