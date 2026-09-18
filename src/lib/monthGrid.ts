export type MonthRef = { year: number; month: number };

export function plainDateFromParts(year: number, month: number, day = 1) {
  return Temporal.PlainDate.from({ year, month, day });
}

export function toDateString(date: Temporal.PlainDate) {
  return date.toString();
}

/** 含 `date` 的那一周的周一（ISO 周，周一起算）。 */
export function mondayOf(date: Temporal.PlainDate): Temporal.PlainDate {
  return date.subtract({ days: date.dayOfWeek - 1 });
}

/** 该月至少占一天的自然周数。 */
export function weeksSpannedByMonth(year: number, month: number): number {
  const first = plainDateFromParts(year, month, 1);
  const last = first.with({ day: first.daysInMonth });
  const gridStart = mondayOf(first);
  const gridEnd = mondayOf(last);
  return gridStart.until(gridEnd, { largestUnit: "weeks" }).weeks + 1;
}

/**
 * 月初卡点：含该月 1 号的那一周的周一。
 * 4 周月（28 天且 1 号为周一）锚点上移一周，以便切月后在 6 行视口内垂直居中。
 * 整周行由 CSS scroll-snap 对齐；不强制滚到此卡点。
 */
export function snapWeekMonday(year: number, month: number): Temporal.PlainDate {
  const first = plainDateFromParts(year, month, 1);
  let snap = mondayOf(first);
  if (weeksSpannedByMonth(year, month) === 4) {
    snap = snap.subtract({ days: 7 });
  }
  return snap;
}

export function enumerateWeeks(fromMonday: Temporal.PlainDate, count: number): Temporal.PlainDate[] {
  return Array.from({ length: count }, (_, index) => fromMonday.add({ days: index * 7 }));
}

export function enumerateDaysInWeek(weekMonday: Temporal.PlainDate): Temporal.PlainDate[] {
  return Array.from({ length: 7 }, (_, index) => weekMonday.add({ days: index }));
}

export function addMonths(ref: MonthRef, delta: number): MonthRef {
  const next = plainDateFromParts(ref.year, ref.month, 1).add({ months: delta });
  return { year: next.year, month: next.month };
}

export function monthRangeAround(center: MonthRef, radiusMonths: number): MonthRef[] {
  return Array.from({ length: radiusMonths * 2 + 1 }, (_, index) => addMonths(center, index - radiusMonths));
}

export function monthOwningSnap(weekMonday: Temporal.PlainDate, candidates: MonthRef[]): MonthRef | null {
  const key = weekMonday.toString();
  for (const candidate of candidates) {
    if (snapWeekMonday(candidate.year, candidate.month).toString() === key) {
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

export function buildWeekStrip(center: MonthRef, bufferMonths: number) {
  const months = monthRangeAround(center, bufferMonths);
  const snapMondays = months.map((month) => snapWeekMonday(month.year, month.month));
  const firstMonday = snapMondays.reduce((earliest, current) =>
    Temporal.PlainDate.compare(current, earliest) < 0 ? current : earliest,
  );
  const lastMonday = snapMondays.reduce((latest, current) =>
    Temporal.PlainDate.compare(current, latest) > 0 ? current : latest,
  );
  const weekCount = firstMonday.until(lastMonday, { largestUnit: "weeks" }).weeks + 1;
  return {
    weeks: enumerateWeeks(firstMonday, weekCount),
    months,
    firstMonday,
  };
}

export function snapIndexForMonth(weeks: Temporal.PlainDate[], year: number, month: number): number {
  const snapKey = snapWeekMonday(year, month).toString();
  return weeks.findIndex((week) => week.toString() === snapKey);
}

export function nearestSnapMonth(
  scrollTop: number,
  weekHeight: number,
  weeks: Temporal.PlainDate[],
  snapMonths: MonthRef[],
): MonthRef | null {
  if (weekHeight <= 0 || weeks.length === 0 || snapMonths.length === 0) {
    return null;
  }

  let bestDistance = Number.POSITIVE_INFINITY;
  let bestMonth: MonthRef | null = null;

  for (const month of snapMonths) {
    const index = snapIndexForMonth(weeks, month.year, month.month);
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
