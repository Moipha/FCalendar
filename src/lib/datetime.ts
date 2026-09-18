export function localTimeZone() {
  return Intl.DateTimeFormat().resolvedOptions().timeZone;
}

export function toZonedDateTime(value: string) {
  const timeZone = localTimeZone();
  const bracketMatch = value.match(/^(.*)\[([^\]]+)\]$/);
  if (bracketMatch) {
    const base = bracketMatch[1];
    const tz = bracketMatch[2];
    // 同时带偏移与 [时区名] 时 Temporal 无法解析，只保留偏移部分
    if (base.includes("+") || base.endsWith("Z") || /-\d{2}:\d{2}$/.test(base)) {
      return Temporal.Instant.from(base).toZonedDateTimeISO(timeZone);
    }
    return Temporal.ZonedDateTime.from(`${base}[${tz}]`);
  }
  return Temporal.Instant.from(value).toZonedDateTimeISO(timeZone);
}

/** 写入后端用的含偏移 ISO 字符串（不含 [时区名]）。 */
export function toOffsetIsoString(zdt: Temporal.ZonedDateTime) {
  const plain = zdt.toPlainDateTime();
  const seconds = plain.toString({ smallestUnit: "second" });
  return `${seconds}${zdt.offset}`;
}

export const weekdayLabels = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];

export function formatMonthLabel(year: number, month: number) {
  return `${year}年${month}月`;
}

export function getMonthGridRange(year: number, month: number) {
  const first = new Date(year, month - 1, 1);
  const last = new Date(year, month, 0);
  const startOffset = (first.getDay() + 6) % 7;
  const gridStart = new Date(first);
  gridStart.setDate(first.getDate() - startOffset);
  const endOffset = 6 - ((last.getDay() + 6) % 7);
  const gridEnd = new Date(last);
  gridEnd.setDate(last.getDate() + endOffset);
  return {
    from: toDateString(gridStart),
    to: toDateString(gridEnd),
  };
}

export function toDateString(date: Date) {
  const y = date.getFullYear();
  const m = `${date.getMonth() + 1}`.padStart(2, "0");
  const d = `${date.getDate()}`.padStart(2, "0");
  return `${y}-${m}-${d}`;
}

export function localOffsetString(date = new Date()) {
  const offsetMinutes = -date.getTimezoneOffset();
  const sign = offsetMinutes >= 0 ? "+" : "-";
  const abs = Math.abs(offsetMinutes);
  const hours = `${Math.floor(abs / 60)}`.padStart(2, "0");
  const minutes = `${abs % 60}`.padStart(2, "0");
  return `${sign}${hours}:${minutes}`;
}

export function toDatetimeLocalValue(value: string) {
  if (value.length <= 10) {
    return `${value}T09:00`;
  }
  const match = value.match(/^(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2})/);
  return match ? `${match[1]}T${match[2]}` : value.slice(0, 16);
}

export function fromDatetimeLocalValue(value: string) {
  if (value.length <= 10) {
    return value;
  }
  const offset = localOffsetString();
  return value.length === 16 ? `${value}:00${offset}` : `${value}${offset}`;
}

export function defaultTimedRange(date: string) {
  const now = new Date();
  const dtstart = `${date}T${`${now.getHours()}`.padStart(2, "0")}:${`${now.getMinutes()}`.padStart(2, "0")}:00${localOffsetString(now)}`;
  const end = new Date(now.getTime() + 60 * 60 * 1000);
  const dtend = `${date}T${`${end.getHours()}`.padStart(2, "0")}:${`${end.getMinutes()}`.padStart(2, "0")}:00${localOffsetString(end)}`;
  return { dtstart, dtend };
}
