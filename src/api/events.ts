import { invoke } from "@tauri-apps/api/core";

export type EventInstance = {
  instanceId: string;
  eventId: string;
  calendarId: string;
  summary: string;
  description?: string | null;
  allDay: boolean;
  dtstart: string;
  dtend: string;
  rrule?: string | null;
};

export type EventRow = {
  id: string;
  calendarId: string;
  uid: string;
  summary: string;
  description?: string | null;
  allDay: boolean;
  dtstart: string;
  dtend?: string | null;
  rrule?: string | null;
  dirty: boolean;
};

export type SaveEventInput = {
  calendarId: string;
  summary: string;
  description?: string | null;
  allDay: boolean;
  dtstart: string;
  dtend: string;
  rrule?: string | null;
};

export function listEvents(from: string, to: string, calendarId?: string) {
  return invoke<EventInstance[]>("list_events", { from, to, calendarId });
}

export function getEvent(id: string) {
  return invoke<EventRow>("get_event", { id });
}

export function createEvent(input: SaveEventInput) {
  return invoke<EventRow>("create_event", { input });
}

export function updateEvent(id: string, input: SaveEventInput) {
  return invoke<EventRow>("update_event", { id, input });
}

export function deleteEvent(id: string) {
  return invoke<void>("delete_event", { id });
}
