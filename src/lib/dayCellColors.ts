import type { DayColorPreset } from "@/api/dayColors";

/** 日格底色预设（含默认）。 */
export type DayCellTint = "default" | DayColorPreset;

export const DAY_CELL_TINT_ORDER: DayCellTint[] = ["default", "green", "red"];

export const DAY_CELL_TINT_LABEL: Record<DayCellTint, string> = {
  default: "默认",
  green: "浅绿",
  red: "浅红",
};

/** 日格填充色。菜单色块与格子底色共用 CSS 变量，浅色/深色各自一套。 */
export const DAY_CELL_TINT_FILL: Record<DayCellTint, string> = {
  default: "var(--fc-day-tint-default)",
  green: "var(--fc-day-tint-green)",
  red: "var(--fc-day-tint-red)",
};

/** 日格悬浮色。 */
export const DAY_CELL_TINT_HOVER: Record<DayCellTint, string> = {
  default: "var(--fc-day-tint-default-hover)",
  green: "var(--fc-day-tint-green-hover)",
  red: "var(--fc-day-tint-red-hover)",
};

export const DAY_CELL_TINT_SWATCH = DAY_CELL_TINT_FILL;

export function normalizeDayColorPreset(value: unknown): DayColorPreset | null {
  if (value === "green" || value === "Green") {
    return "green";
  }
  if (value === "red" || value === "Red") {
    return "red";
  }
  return null;
}
