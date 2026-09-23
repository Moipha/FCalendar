import { invoke } from "@tauri-apps/api/core";

import type { CalendarRow } from "@/api/calendars";

export type AccountRow = {
  id: string;
  displayName: string;
  serverUrl: string;
  username: string;
  principalUrl?: string | null;
  calendarHomeUrl?: string | null;
};

export type AccountStatus = {
  account: AccountRow | null;
  remoteCalendars: CalendarRow[];
};

export type ConnectAccountInput = {
  serverUrl: string;
  username: string;
  password: string;
};

export function getAccount() {
  return invoke<AccountStatus>("get_account");
}

export function connectAccount(input: ConnectAccountInput) {
  return invoke<AccountStatus>("connect_account", { input });
}

export function disconnectAccount() {
  return invoke<void>("disconnect_account_cmd");
}

export function rediscoverCalendars() {
  return invoke<AccountStatus>("rediscover_calendars");
}
