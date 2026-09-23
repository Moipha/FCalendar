import type { QueryClient } from "@tanstack/vue-query";

export function monthEventsQueryKey(from: string, to: string, calendarId: string) {
  return ["events", "month", from, to, calendarId] as const;
}

export function overviewEventsQueryKey(from: string, to: string, calendarId: string) {
  return ["events", "overview", from, to, calendarId] as const;
}

export function invalidateEvents(queryClient: QueryClient) {
  return queryClient.invalidateQueries({ queryKey: ["events"] });
}
