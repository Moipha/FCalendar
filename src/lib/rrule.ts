export type RecurrencePreset = "never" | "daily" | "weekly" | "monthly" | "yearly";

export const recurrenceOptions: { value: RecurrencePreset; label: string }[] = [
  { value: "never", label: "不重复" },
  { value: "daily", label: "每天" },
  { value: "weekly", label: "每周" },
  { value: "monthly", label: "每月" },
  { value: "yearly", label: "每年" },
];

export function presetToRrule(preset: RecurrencePreset): string | null {
  switch (preset) {
    case "never":
      return null;
    case "daily":
      return "FREQ=DAILY";
    case "weekly":
      return "FREQ=WEEKLY";
    case "monthly":
      return "FREQ=MONTHLY";
    case "yearly":
      return "FREQ=YEARLY";
    default:
      return null;
  }
}

export function rruleToPreset(rrule: string | null | undefined): RecurrencePreset {
  if (!rrule?.trim()) {
    return "never";
  }
  const normalized = rrule.trim().toUpperCase();
  if (normalized === "FREQ=DAILY") return "daily";
  if (normalized === "FREQ=WEEKLY") return "weekly";
  if (normalized === "FREQ=MONTHLY") return "monthly";
  if (normalized === "FREQ=YEARLY") return "yearly";
  return "never";
}
