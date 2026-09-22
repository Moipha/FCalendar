export type MonthRef = { year: number; month: number };

export type WeekStart = "monday" | "sunday";

export function plainDateFromParts(year: number, month: number, day = 1) {
  return Temporal.PlainDate.from({ year, month, day });
}

export function toDateString(date: Temporal.PlainDate) {
  return date.toString();
}

/** 含 `date` 的那一周的首日（周一起或周日起）。 */
export function weekStartOf(date: Temporal.PlainDate, weekStart: WeekStart): Temporal.PlainDate {
  if (weekStart === "monday") {
    return date.subtract({ days: date.dayOfWeek - 1 });
  }
  const offset = date.dayOfWeek % 7;
  return date.subtract({ days: offset });
}

/** 该月至少占一天的自然周数。 */
export function weeksSpannedByMonth(
  year: number,
  month: number,
  weekStart: WeekStart,
): number {
  const first = plainDateFromParts(year, month, 1);
  const last = first.with({ day: first.daysInMonth });
  const gridStart = weekStartOf(first, weekStart);
  const gridEnd = weekStartOf(last, weekStart);
  return gridStart.until(gridEnd, { largestUnit: "weeks" }).weeks + 1;
}

function isFourWeekMonth(year: number, month: number, weekStart: WeekStart): boolean {
  const first = plainDateFromParts(year, month, 1);
  if (first.daysInMonth !== 28) {
    return false;
  }
  if (weekStart === "monday") {
    return first.dayOfWeek === 1;
  }
  return first.dayOfWeek === 7;
}

/**
 * 月初卡点：含该月 1 号的那一周的首日。
 * 4 周月（28 天且 1 号恰为周首）锚点上移一周，以便切月后在 6 行视口内垂直居中。
 */
export function snapWeekStart(
  year: number,
  month: number,
  weekStart: WeekStart,
): Temporal.PlainDate {
  const first = plainDateFromParts(year, month, 1);
  let snap = weekStartOf(first, weekStart);
  if (isFourWeekMonth(year, month, weekStart)) {
    snap = snap.subtract({ days: 7 });
  }
  return snap;
}

export function enumerateWeeks(fromWeekStart: Temporal.PlainDate, count: number): Temporal.PlainDate[] {
  return Array.from({ length: count }, (_, index) => fromWeekStart.add({ days: index * 7 }));
}

export function enumerateDaysInWeek(weekStartDate: Temporal.PlainDate): Temporal.PlainDate[] {
  return Array.from({ length: 7 }, (_, index) => weekStartDate.add({ days: index }));
}

export function addMonths(ref: MonthRef, delta: number): MonthRef {
  const next = plainDateFromParts(ref.year, ref.month, 1).add({ months: delta });
  return { year: next.year, month: next.month };
}

export function monthRangeAround(center: MonthRef, radiusMonths: number): MonthRef[] {
  return Array.from({ length: radiusMonths * 2 + 1 }, (_, index) => addMonths(center, index - radiusMonths));
}

export function monthOwningSnap(
  weekStartDate: Temporal.PlainDate,
  candidates: MonthRef[],
  weekStart: WeekStart,
): MonthRef | null {
  const key = weekStartDate.toString();
  for (const candidate of candidates) {
    if (snapWeekStart(candidate.year, candidate.month, weekStart).toString() === key) {
      return candidate;
    }
  }
  return null;
}

export function monthsIntersectingWeekStrip(weeks: Temporal.PlainDate[]): MonthRef[] {
  if (weeks.length === 0) {
    return [];
  }
  const stripStart = weeks[0];
  const stripEnd = weeks[weeks.length - 1].add({ days: 6 });
  const months: MonthRef[] = [];
  let cursor = plainDateFromParts(stripStart.year, stripStart.month, 1);
  const last = plainDateFromParts(stripEnd.year, stripEnd.month, 1);
  while (Temporal.PlainDate.compare(cursor, last) <= 0) {
    months.push({ year: cursor.year, month: cursor.month });
    cursor = cursor.add({ months: 1 });
  }
  return months;
}

export function buildWeekStrip(center: MonthRef, bufferMonths: number, weekStart: WeekStart) {
  const months = monthRangeAround(center, bufferMonths);
  const snapWeeks = months.map((month) => snapWeekStart(month.year, month.month, weekStart));
  const firstWeekStart = snapWeeks.reduce((earliest, current) =>
    Temporal.PlainDate.compare(current, earliest) < 0 ? current : earliest,
  );
  const lastWeekStart = snapWeeks.reduce(
    (latest, current) => (Temporal.PlainDate.compare(current, latest) > 0 ? current : latest),
    snapWeeks[0],
  );
  const weekCount = firstWeekStart.until(lastWeekStart, { largestUnit: "weeks" }).weeks + 1;
  return {
    weeks: enumerateWeeks(firstWeekStart, weekCount),
    months,
    firstMonday: firstWeekStart,
  };
}

export function snapIndexForMonth(
  weeks: Temporal.PlainDate[],
  year: number,
  month: number,
  weekStart: WeekStart,
): number {
  const snapKey = snapWeekStart(year, month, weekStart).toString();
  return weeks.findIndex((week) => week.toString() === snapKey);
}

export function nearestSnapMonth(
  scrollTop: number,
  weekHeight: number,
  weeks: Temporal.PlainDate[],
  snapMonths: MonthRef[],
  weekStart: WeekStart,
): MonthRef | null {
  if (weekHeight <= 0 || weeks.length === 0 || snapMonths.length === 0) {
    return null;
  }

  let bestDistance = Number.POSITIVE_INFINITY;
  let bestMonth: MonthRef | null = null;

  for (const month of snapMonths) {
    const index = snapIndexForMonth(weeks, month.year, month.month, weekStart);
    if (index < 0) {
      continue;
    }
    const distance = Math.abs(scrollTop - index * weekHeight);
    if (distance < bestDistance) {
      bestDistance = distance;
      bestMonth = month;
    }
  }

  return bestMonth;
}

export function weekStripDateRange(weeks: Temporal.PlainDate[]) {
  if (weeks.length === 0) {
    return { from: "", to: "" };
  }
  return {
    from: weeks[0].toString(),
    to: weeks[weeks.length - 1].add({ days: 6 }).toString(),
  };
}

export function todayMonthRef(): MonthRef {
  const today = Temporal.Now.plainDateISO();
  return { year: today.year, month: today.month };
}
