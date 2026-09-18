import type { EventRow, SaveEventInput } from "@/api/events";
import { toOffsetIsoString, toZonedDateTime } from "@/lib/datetime";

/** 按抓取日相对平移整组事件的起止日期，时刻与时长不变。 */
export function shiftEventToDate(
  event: EventRow,
  sourceDate: string,
  targetDate: string,
): SaveEventInput {
  const source = Temporal.PlainDate.from(sourceDate);
  const target = Temporal.PlainDate.from(targetDate);
  const days = source.until(target).days;

  if (event.allDay) {
    const dtstart = Temporal.PlainDate.from(event.dtstart.slice(0, 10)).add({ days }).toString();
    const endSource = (event.dtend ?? event.dtstart).slice(0, 10);
    const dtend = Temporal.PlainDate.from(endSource).add({ days }).toString();
    return {
      calendarId: event.calendarId,
      summary: event.summary,
      description: event.description ?? null,
      allDay: true,
      dtstart,
      dtend,
      rrule: event.rrule ?? null,
    };
  }

  const dtstart = toOffsetIsoString(toZonedDateTime(event.dtstart).add({ days }));
  const dtend = toOffsetIsoString(
    toZonedDateTime(event.dtend ?? event.dtstart).add({ days }),
  );

  return {
    calendarId: event.calendarId,
    summary: event.summary,
    description: event.description ?? null,
    allDay: false,
    dtstart,
    dtend,
    rrule: event.rrule ?? null,
  };
}
