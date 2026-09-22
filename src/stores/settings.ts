import { defineStore } from "pinia";

import {
  applyAppearanceToDocument,
  DEFAULT_JIEQI_COLORS,
  DEFAULT_PRIMARY,
  type JieqiSeason,
} from "@/lib/settingsTheme";

export const UPCOMING_DAY_OPTIONS = [7, 14, 30] as const;
export type UpcomingDays = (typeof UPCOMING_DAY_OPTIONS)[number];

export type WeekStart = "monday" | "sunday";

export type AppSettings = {
  upcomingDays: UpcomingDays;
  primaryColor: string;
  jieqiColors: Record<JieqiSeason, string>;
  weekStart: WeekStart;
  showMinorFestivals: boolean;
  scrollSnapWeeks: boolean;
};

export const DEFAULT_SETTINGS: AppSettings = {
  upcomingDays: 7,
  primaryColor: DEFAULT_PRIMARY,
  jieqiColors: { ...DEFAULT_JIEQI_COLORS },
  weekStart: "monday",
  showMinorFestivals: true,
  scrollSnapWeeks: true,
};

const STORAGE_KEY = "fc.settings.v1";

function parseSettings(raw: string | null): AppSettings {
  if (!raw) {
    return { ...DEFAULT_SETTINGS, jieqiColors: { ...DEFAULT_JIEQI_COLORS } };
  }
  try {
    const data = JSON.parse(raw) as Partial<AppSettings>;
    const upcoming = data.upcomingDays;
    const upcomingDays =
      upcoming === 7 || upcoming === 14 || upcoming === 30 ? upcoming : DEFAULT_SETTINGS.upcomingDays;
    return {
      upcomingDays,
      primaryColor:
        typeof data.primaryColor === "string" && data.primaryColor.startsWith("#")
          ? data.primaryColor
          : DEFAULT_SETTINGS.primaryColor,
      jieqiColors: {
        spring: data.jieqiColors?.spring ?? DEFAULT_JIEQI_COLORS.spring,
        summer: data.jieqiColors?.summer ?? DEFAULT_JIEQI_COLORS.summer,
        autumn: data.jieqiColors?.autumn ?? DEFAULT_JIEQI_COLORS.autumn,
        winter: data.jieqiColors?.winter ?? DEFAULT_JIEQI_COLORS.winter,
      },
      weekStart: data.weekStart === "sunday" ? "sunday" : "monday",
      showMinorFestivals: data.showMinorFestivals !== false,
      scrollSnapWeeks: data.scrollSnapWeeks !== false,
    };
  } catch {
    return { ...DEFAULT_SETTINGS, jieqiColors: { ...DEFAULT_JIEQI_COLORS } };
  }
}

function readSettings(): AppSettings {
  try {
    const legacy = localStorage.getItem("fc.settings.upcomingDays");
    const v1 = localStorage.getItem(STORAGE_KEY);
    if (!v1 && legacy) {
      const n = Number(legacy);
      const base = parseSettings(null);
      if (n === 7 || n === 14 || n === 30) {
        base.upcomingDays = n;
      }
      return base;
    }
    return parseSettings(v1);
  } catch {
    return { ...DEFAULT_SETTINGS, jieqiColors: { ...DEFAULT_JIEQI_COLORS } };
  }
}

function writeSettings(settings: AppSettings) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
    localStorage.removeItem("fc.settings.upcomingDays");
  } catch {
    // 写入失败不影响本次会话
  }
}

export const useSettingsStore = defineStore("settings", {
  state: (): AppSettings & { dialogOpen: boolean } => ({
    ...readSettings(),
    dialogOpen: false,
  }),
  actions: {
    hydrate() {
      const loaded = readSettings();
      Object.assign(this, loaded, { dialogOpen: this.dialogOpen });
      applyAppearanceToDocument(this.primaryColor, this.jieqiColors);
    },
    applyAppearance() {
      applyAppearanceToDocument(this.primaryColor, this.jieqiColors);
    },
    persist() {
      const payload: AppSettings = {
        upcomingDays: this.upcomingDays,
        primaryColor: this.primaryColor,
        jieqiColors: { ...this.jieqiColors },
        weekStart: this.weekStart,
        showMinorFestivals: this.showMinorFestivals,
        scrollSnapWeeks: this.scrollSnapWeeks,
      };
      writeSettings(payload);
      this.applyAppearance();
    },
    patch(partial: Partial<AppSettings>) {
      Object.assign(this, partial);
      if (partial.primaryColor || partial.jieqiColors) {
        this.applyAppearance();
      }
    },
    resetJieqiColors() {
      this.jieqiColors = { ...DEFAULT_JIEQI_COLORS };
      this.applyAppearance();
    },
    openDialog() {
      this.dialogOpen = true;
    },
    closeDialog() {
      this.dialogOpen = false;
    },
  },
});
