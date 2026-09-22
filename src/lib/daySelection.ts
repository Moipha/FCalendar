import { enumerateDaysInWeek } from "@/lib/monthGrid";

/** 周带子内全部日期，按周从左到右、再下一周。 */
export function orderedDatesInWeekStrip(weekStarts: Temporal.PlainDate[]): string[] {
  const dates: string[] = [];
  for (const week of weekStarts) {
    for (const day of enumerateDaysInWeek(week)) {
      dates.push(day.toString());
    }
  }
  return dates;
}

/** 锚点到目标日（含），按带子顺序；若某日不在带子中则只含存在的端点。 */
export function dateRangeInclusiveInStrip(
  weekStarts: Temporal.PlainDate[],
  anchorDate: string,
  targetDate: string,
): string[] {
  const ordered = orderedDatesInWeekStrip(weekStarts);
  const anchorIndex = ordered.indexOf(anchorDate);
  const targetIndex = ordered.indexOf(targetDate);
  if (anchorIndex < 0 && targetIndex < 0) {
    return [];
  }
  if (anchorIndex < 0) {
    return [targetDate];
  }
  if (targetIndex < 0) {
    return [anchorDate];
  }
  const start = Math.min(anchorIndex, targetIndex);
  const end = Math.max(anchorIndex, targetIndex);
  return ordered.slice(start, end + 1);
}
