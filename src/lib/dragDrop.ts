import type { DropTargetKind } from "@/stores/taskDrag";

export const DRAG_THRESHOLD = 5;
export const DROP_TARGET_CLASS = "fc-drop-target";
export const DAY_TARGET_CLASS = "fc-day-drop-target";

export function findDropTarget(x: number, y: number): { kind: DropTargetKind; date: string | null } {
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
    const day = element.closest(".fc-month-day[data-date]") as HTMLElement | null;
    if (day?.dataset.date) {
      return { kind: "calendar-day", date: day.dataset.date };
    }
  }
  return { kind: null, date: null };
}

export function clearDropHighlights() {
  document.querySelectorAll(`.${DROP_TARGET_CLASS}, .${DAY_TARGET_CLASS}`).forEach((node) => {
    node.classList.remove(DROP_TARGET_CLASS, DAY_TARGET_CLASS);
  });
}

export function applyDropHighlight(kind: DropTargetKind, date: string | null) {
  clearDropHighlights();
  if (kind === "calendar-day" && date) {
    document
      .querySelector(`.fc-month-day[data-date="${date}"]`)
      ?.classList.add(DAY_TARGET_CLASS);
    return;
  }
  if (kind === "tasks" || kind === "stamps") {
    document.querySelector(`[data-drop-zone="${kind}"]`)?.classList.add(DROP_TARGET_CLASS);
  }
}
