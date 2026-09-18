import { defineStore } from "pinia";

export const calendarViewOptions = [{ id: "month", label: "月视图" }] as const;

export type CalendarViewId = (typeof calendarViewOptions)[number]["id"];

export const useCalendarViewStore = defineStore("calendarView", {
  state: () => ({
    current: "month" as CalendarViewId,
  }),
  actions: {
    setView(view: CalendarViewId) {
      this.current = view;
    },
  },
});
