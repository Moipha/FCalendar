import { invoke } from "@tauri-apps/api/core";

import type { AccountRow } from "@/api/account";
import type { CalendarRow } from "@/api/calendars";

export type ConnectionStatus = "logged_out" | "online" | "offline" | "auth_error";

export type SessionSnapshot = {
  currentCalendarId: string;
  connectionStatus: ConnectionStatus;
  pendingRemoteChanges: boolean;
  account: AccountRow | null;
  calendars: CalendarRow[];
};

export function getSession() {
  return invoke<SessionSnapshot>("get_session");
}

export function bootstrapSession() {
  return invoke<SessionSnapshot>("bootstrap_session");
}

export function syncCurrentCalendar() {
  return invoke<SessionSnapshot>("sync_current_calendar");
}

export function setCurrentCalendar(calendarId: string) {
  return invoke<SessionSnapshot>("set_current_calendar", { calendarId });
}

export function updateAccountPassword(password: string) {
  return invoke<SessionSnapshot>("update_account_password", { password });
}

export function createRemoteCalendar(input: {
  displayName: string;
  color?: string | null;
  migrateFromLocal: boolean;
  clearLocal: boolean;
}) {
  return invoke<SessionSnapshot>("create_remote_calendar", { input });
}

export function logoutAccount(discard: boolean) {
  return invoke<SessionSnapshot>("logout_account", { discard });
}
