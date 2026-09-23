import type { CalendarRow } from "@/api/calendars";

/** 预览与同步前：事件只写入无 accountId 的「本地」日历。 */
export function pickLocalCalendarId(calendars: CalendarRow[] | undefined): string {
  if (!calendars?.length) {
    return "";
  }
  const local = calendars.find((c) => !c.accountId);
  return local?.id ?? "";
}
