import type { DayColorPreset } from "@/api/dayColors";

/** 日格底色预设（含默认）。 */
export type DayCellTint = "default" | DayColorPreset;

export const DAY_CELL_TINT_ORDER: DayCellTint[] = ["default", "green", "red"];

export const DAY_CELL_TINT_LABEL: Record<DayCellTint, string> = {
  default: "默认",
  green: "浅绿",
  red: "浅红",
};

/** 日格填充色。菜单色块与格子底色共用。 */
export const DAY_CELL_TINT_FILL: Record<DayCellTint, string> = {
  default: "var(--background)",
  green: "oklch(0.94 0.045 150)",
  red: "oklch(0.94 0.03 25)",
};

/** 日格悬浮色。默认白无法再变浅，沿用现有浅灰。 */
export const DAY_CELL_TINT_HOVER: Record<DayCellTint, string> = {
  default: "color-mix(in oklab, var(--muted) 35%, var(--background))",
  green: "oklch(0.97 0.025 150)",
  red: "oklch(0.975 0.015 25)",
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
