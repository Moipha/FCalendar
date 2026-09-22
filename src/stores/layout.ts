import { defineStore } from "pinia";

const MIN_TASK_PANE = 320;
const DEFAULT_TASK_PANE = 360;
const RAIL_WIDTH = 80;

export type SidePanePage = "tasks" | "overview";

export const useLayoutStore = defineStore("layout", {
  state: () => ({
    railWidth: RAIL_WIDTH,
    taskPaneWidth: DEFAULT_TASK_PANE,
    taskPaneHidden: false,
    taskPaneWidthBeforeHide: DEFAULT_TASK_PANE,
    dragStartWidth: DEFAULT_TASK_PANE,
    dragStartX: 0,
    hideTriggeredByDrag: false,
    sidePanePage: "tasks" as SidePanePage,
  }),
  getters: {
    minTaskPane: () => MIN_TASK_PANE,
    defaultTaskPane: () => DEFAULT_TASK_PANE,
  },
  actions: {
    beginResize(clientX: number) {
      this.dragStartX = clientX;
      this.dragStartWidth = this.taskPaneHidden ? MIN_TASK_PANE : this.taskPaneWidth;
    },
    resizeTo(clientX: number) {
      if (this.taskPaneHidden) {
        return;
      }
      const delta = clientX - this.dragStartX;
      const next = Math.max(MIN_TASK_PANE, this.dragStartWidth + delta);
      const movedLeft = this.dragStartX - clientX;
      if (movedLeft > (this.dragStartWidth * 2) / 3) {
        this.taskPaneHidden = true;
        this.hideTriggeredByDrag = true;
        return;
      }
      this.taskPaneWidth = next;
    },
    toggleTaskPane() {
      if (this.taskPaneHidden) {
        this.taskPaneHidden = false;
        this.taskPaneWidth = this.hideTriggeredByDrag
          ? MIN_TASK_PANE
          : this.taskPaneWidthBeforeHide;
        this.hideTriggeredByDrag = false;
        return;
      }
      this.taskPaneWidthBeforeHide = this.taskPaneWidth;
      this.taskPaneHidden = true;
      this.hideTriggeredByDrag = false;
    },
    openSidePage(page: SidePanePage) {
      this.sidePanePage = page;
      if (this.taskPaneHidden) {
        this.toggleTaskPane();
      }
    },
    selectSidePage(page: SidePanePage) {
      if (!this.taskPaneHidden && this.sidePanePage === page) {
        this.toggleTaskPane();
        return;
      }
      this.openSidePage(page);
    },
  },
});
