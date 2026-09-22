/** 默认主题色（近黑，与 style.css 一致） */
export const DEFAULT_PRIMARY = "#343434";

export const THEME_PRESETS = [
  { id: "default", label: "默认", hex: "#343434" },
  { id: "blue", label: "蓝", hex: "#2563eb" },
  { id: "green", label: "绿", hex: "#16a34a" },
  { id: "amber", label: "琥珀", hex: "#d97706" },
  { id: "rose", label: "玫红", hex: "#e11d48" },
] as const;

export const DEFAULT_JIEQI_COLORS = {
  spring: "#d4537e",
  summer: "#1f6b4a",
  autumn: "#c47a12",
  winter: "#2f7ae5",
} as const;

export type JieqiSeason = keyof typeof DEFAULT_JIEQI_COLORS;

function hexToRgb(hex: string) {
  const normalized = hex.replace("#", "");
  const full =
    normalized.length === 3
      ? normalized
          .split("")
          .map((c) => c + c)
          .join("")
      : normalized;
  const n = Number.parseInt(full, 16);
  return { r: (n >> 16) & 255, g: (n >> 8) & 255, b: n & 255 };
}

function relativeLuminance(hex: string) {
  const { r, g, b } = hexToRgb(hex);
  const channel = (c: number) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

export function primaryForegroundFor(hex: string) {
  return relativeLuminance(hex) > 0.45 ? "#171717" : "#fafafa";
}

export function hexToOklchPrimary(hex: string) {
  // 浏览器原生 color-mix 更稳；主题色直接写 hex 供 Tailwind arbitrary 或 CSS
  return hex;
}

export function applyAppearanceToDocument(primary: string, jieqi: Record<JieqiSeason, string>) {
  const root = document.documentElement;
  const fg = primaryForegroundFor(primary);
  root.style.setProperty("--primary", hexToOklchPrimary(primary));
  root.style.setProperty("--primary-foreground", fg);
  root.style.setProperty("--fc-jieqi-spring", jieqi.spring);
  root.style.setProperty("--fc-jieqi-summer", jieqi.summer);
  root.style.setProperty("--fc-jieqi-autumn", jieqi.autumn);
  root.style.setProperty("--fc-jieqi-winter", jieqi.winter);
}
