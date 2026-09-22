import { getLunarDayInfo } from "@/lib/lunarDay";
import { localTimeZone } from "@/lib/datetime";

/** 放假日：国务院「假」，或周末且非「班」。 */
export function isRestDay(dateIso: string): boolean {
  const mark = getLunarDayInfo(dateIso).workMark;
  if (mark === "work") {
    return false;
  }
  if (mark === "off") {
    return true;
  }
  const weekday = Temporal.PlainDate.from(dateIso).dayOfWeek;
  return weekday === 6 || weekday === 7;
}

export type NextHoliday =
  | { kind: "today" }
  | { kind: "upcoming"; date: string }
  | { kind: "none" };

const SCAN_DAYS = 400;

export function nextHolidayFrom(todayIso: string): NextHoliday {
  if (isRestDay(todayIso)) {
    return { kind: "today" };
  }
  let cursor = Temporal.PlainDate.from(todayIso);
  for (let i = 0; i < SCAN_DAYS; i += 1) {
    cursor = cursor.add({ days: 1 });
    const iso = cursor.toString();
    if (isRestDay(iso)) {
      return { kind: "upcoming", date: iso };
    }
  }
  return { kind: "none" };
}

export function startOfDayInstant(dateIso: string): Temporal.Instant {
  return Temporal.PlainDate.from(dateIso)
    .toZonedDateTime({ timeZone: localTimeZone() })
    .toInstant();
}

const CN_DIGITS = ["零", "一", "二", "三", "四", "五", "六", "七", "八", "九", "十"];

/** 数量词：2 读「两」（两天 / 两天半），其余用小写数字。 */
function daysWord(n: number): string {
  if (n === 2) {
    return "两";
  }
  if (n <= 10) {
    return CN_DIGITS[n];
  }
  return `${n}`;
}

/** 从现在到目标日 0:00 的口语化剩余。 */
export function colloquialUntil(now: Temporal.Instant, target: Temporal.Instant): string {
  const hours = now.until(target, { largestUnit: "hours" }).hours;
  if (hours < 12) {
    return "不到半天";
  }
  const halfDays = Math.floor(hours / 12);
  if (halfDays % 2 === 1) {
    return `${daysWord(Math.ceil(halfDays / 2))}天`;
  }
  return `${daysWord(halfDays / 2)}天半`;
}
