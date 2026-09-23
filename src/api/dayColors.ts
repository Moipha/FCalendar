import { invoke } from "@tauri-apps/api/core";

export type DayColorPreset = "green" | "red";

export type DayColorRow = {
  date: string;
  /** 后端为 `green` / `red` 字符串 */
  color: DayColorPreset | string;
};

export function listDayColors(from: string, to: string, calendarId?: string) {
  return invoke<DayColorRow[]>("list_day_colors", { from, to, calendarId });
}

/** 传 `null` 或省略 color 表示恢复默认底色（删除库内记录）。 */
export function setDayColor(date: string, color: DayColorPreset | null, calendarId?: string) {
  return invoke<void>("set_day_color", { date, color, calendarId });
}

export function setDayColors(
  dates: string[],
  color: DayColorPreset | null,
  calendarId?: string,
) {
  return invoke<void>("set_day_colors", { dates, color, calendarId });
}
