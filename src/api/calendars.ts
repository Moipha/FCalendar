import { invoke } from "@tauri-apps/api/core";

export type CalendarRow = {
  id: string;
  accountId?: string | null;
  href: string;
  displayName: string;
  color?: string | null;
  visible: boolean;
};

export function listCalendars() {
  return invoke<CalendarRow[]>("list_calendars");
}
