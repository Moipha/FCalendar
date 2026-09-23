import { CalendarDate } from "@internationalized/date";

import type { WeekStart } from "@/stores/settings";

export function toCalendarDate(isoDate: string) {
  const date = Temporal.PlainDate.from(isoDate.slice(0, 10));
  return new CalendarDate(date.year, date.month, date.day);
}

export function fromCalendarDate(value: { year: number; month: number; day: number }) {
  return Temporal.PlainDate.from({
    year: value.year,
    month: value.month,
    day: value.day,
  }).toString();
}

export function weekStartsOnFromSettings(weekStart: WeekStart): 0 | 1 {
  return weekStart === "sunday" ? 0 : 1;
}

export function formatPlainDateLabel(isoDate: string) {
  const date = Temporal.PlainDate.from(isoDate.slice(0, 10));
  return `${date.year}年${date.month}月${date.day}日`;
}

export function splitDatetimeLocal(value: string) {
  const date = value.slice(0, 10);
  const match = value.match(/T(\d{2}):(\d{2})/);
  return {
    date: date.length === 10 ? date : Temporal.Now.plainDateISO().toString(),
    hour: match?.[1] ?? "09",
    minute: match?.[2] ?? "00",
  };
}

export function joinDatetimeLocal(date: string, hour: string, minute: string) {
  return `${date}T${hour}:${minute}`;
}

export const HOUR_OPTIONS = Array.from({ length: 24 }, (_, hour) =>
  `${hour}`.padStart(2, "0"),
);

export const MINUTE_OPTIONS = Array.from({ length: 60 }, (_, minute) =>
  `${minute}`.padStart(2, "0"),
);
