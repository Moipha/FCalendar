<script setup lang="ts">
import { ChevronDown, ChevronRight } from "@lucide/vue";
import { useQuery, useQueryClient } from "@tanstack/vue-query";
import { computed, ref } from "vue";

import {
  deleteEvent,
  getEvent,
  listEvents,
  updateEvent,
  type EventInstance,
  type EventRow,
  type SaveEventInput,
} from "@/api/events";
import EventDialog from "@/components/EventDialog.vue";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { invalidateEvents, overviewEventsQueryKey } from "@/lib/calendarQueries";
import { toZonedDateTime } from "@/lib/datetime";
import { colloquialUntil, nextHolidayFrom, startOfDayInstant } from "@/lib/holiday";
import { useSessionStore } from "@/stores/session";
import { useSettingsStore } from "@/stores/settings";

const props = defineProps<{
  calendarId: string;
}>();

const settings = useSettingsStore();
const session = useSessionStore();
const queryClient = useQueryClient();

const todayIso = computed(() => Temporal.Now.plainDateISO().toString());
const tomorrowIso = computed(() => Temporal.PlainDate.from(todayIso.value).add({ days: 1 }).toString());

const eventRange = computed(() => {
  const from = todayIso.value;
  const to = Temporal.PlainDate.from(from).add({ days: settings.upcomingDays }).toString();
  return { from, to };
});

const calendarId = computed(() => props.calendarId);

const {
  data: instances,
  isPending: eventsPending,
  isError: eventsError,
  refetch: refetchEvents,
} = useQuery({
  queryKey: computed(() =>
    overviewEventsQueryKey(eventRange.value.from, eventRange.value.to, calendarId.value),
  ),
  queryFn: () =>
    listEvents(eventRange.value.from, eventRange.value.to, calendarId.value || undefined),
  enabled: () => Boolean(calendarId.value && eventRange.value.from),
});

function instanceStillOpen(instance: EventInstance, now: Temporal.Instant, today: string): boolean {
  if (instance.allDay) {
    const endInclusive = (instance.dtend || instance.dtstart).slice(0, 10);
    return endInclusive >= today;
  }
  try {
    return Temporal.Instant.compare(Temporal.Instant.from(instance.dtend || instance.dtstart), now) > 0;
  } catch {
    return true;
  }
}

function instanceStartSortKey(instance: EventInstance): string {
  if (instance.allDay) {
    return `${instance.dtstart.slice(0, 10)}T00:00:00`;
  }
  return instance.dtstart;
}

function instanceDayKey(instance: EventInstance): string {
  if (instance.allDay) {
    return instance.dtstart.slice(0, 10);
  }
  return toZonedDateTime(instance.dtstart).toPlainDate().toString();
}

const upcomingEvents = computed(() => {
  const now = Temporal.Now.instant();
  const today = todayIso.value;
  return [...(instances.value ?? [])]
    .filter((item) => instanceStillOpen(item, now, today))
    .sort((a, b) => instanceStartSortKey(a).localeCompare(instanceStartSortKey(b)));
});

const groupedUpcoming = computed(() => {
  const groups: { key: string; label: string; items: EventInstance[] }[] = [];
  const index = new Map<string, (typeof groups)[number]>();
  for (const item of upcomingEvents.value) {
    const key = instanceDayKey(item);
    let group = index.get(key);
    if (!group) {
      let label = `${Temporal.PlainDate.from(key).month}月${Temporal.PlainDate.from(key).day}日`;
      if (key === todayIso.value) {
        label = "今天";
      } else if (key === tomorrowIso.value) {
        label = "明天";
      }
      group = { key, label, items: [] };
      index.set(key, group);
      groups.push(group);
    }
    group.items.push(item);
  }
  return groups;
});

const holidayLine = computed(() => {
  const next = nextHolidayFrom(todayIso.value);
  if (next.kind === "today") {
    return { prefix: "", highlight: "今天放假中", suffix: "" };
  }
  if (next.kind === "none") {
    return { prefix: "近期没有找到放假日", highlight: "", suffix: "" };
  }
  const phrase = colloquialUntil(Temporal.Now.instant(), startOfDayInstant(next.date));
  return { prefix: "距离下次放假还有 ", highlight: phrase, suffix: "" };
});

function formatMd(dateIso: string) {
  const date = Temporal.PlainDate.from(dateIso.slice(0, 10));
  return `${date.month}月${date.day}日`;
}

function formatHm(value: string) {
  const zoned = toZonedDateTime(value);
  return `${`${zoned.hour}`.padStart(2, "0")}:${`${zoned.minute}`.padStart(2, "0")}`;
}

function formatWhen(instance: EventInstance) {
  if (instance.allDay) {
    const start = instance.dtstart.slice(0, 10);
    const end = (instance.dtend || instance.dtstart).slice(0, 10);
    if (end !== start) {
      return `全天 · ${formatMd(start)} – ${formatMd(end)}`;
    }
    return "全天";
  }
  return formatHm(instance.dtstart);
}

const upcomingExpanded = ref(true);
const dialogOpen = ref(false);
const editingEvent = ref<EventRow | null>(null);

async function openEvent(eventId: string) {
  editingEvent.value = await getEvent(eventId);
  dialogOpen.value = true;
}

async function handleSave(input: SaveEventInput) {
  if (!editingEvent.value) {
    return;
  }
  await updateEvent(editingEvent.value.id, input);
  session.noteLocalMutation();
  dialogOpen.value = false;
  editingEvent.value = null;
  await invalidateEvents(queryClient);
}

async function handleDelete() {
  if (!editingEvent.value) {
    return;
  }
  await deleteEvent(editingEvent.value.id);
  session.noteLocalMutation();
  dialogOpen.value = false;
  editingEvent.value = null;
  await invalidateEvents(queryClient);
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col text-foreground">
    <div class="flex shrink-0 items-center border-b border-border px-3 py-2">
      <span class="text-sm font-medium">概览</span>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto px-3 py-3">
      <p class="text-sm leading-relaxed">
        <template v-if="holidayLine.highlight">
          {{ holidayLine.prefix }}
          <span class="font-semibold text-primary">{{ holidayLine.highlight }}</span>
          {{ holidayLine.suffix }}
        </template>
        <template v-else>
          {{ holidayLine.prefix }}
        </template>
      </p>

      <button
        type="button"
        class="text-muted-foreground hover:bg-muted/30 mt-5 flex w-full items-center justify-between py-2 text-left text-xs font-medium"
        :title="upcomingExpanded ? '收起' : '展开'"
        @click="upcomingExpanded = !upcomingExpanded"
      >
        <span>待进行（{{ settings.upcomingDays }} 天内）</span>
        <ChevronDown v-if="upcomingExpanded" class="size-4" aria-hidden="true" />
        <ChevronRight v-else class="size-4" aria-hidden="true" />
      </button>
      <div v-show="upcomingExpanded">
        <div v-if="eventsPending" class="space-y-2 py-3">
          <Skeleton class="h-10 w-full" />
          <Skeleton class="h-10 w-5/6" />
        </div>
        <div v-else-if="eventsError" class="text-muted-foreground flex items-center gap-2 py-4 text-xs">
          <span>待进行加载失败</span>
          <Button type="button" variant="link" class="h-auto px-0 text-xs" @click="refetchEvents()">
            重试
          </Button>
        </div>
        <div v-else-if="groupedUpcoming.length === 0" class="text-muted-foreground py-4 text-xs">
          这段时间没有待进行的事件
        </div>
        <div v-else class="space-y-3">
          <section v-for="group in groupedUpcoming" :key="group.key">
            <p class="text-muted-foreground px-2 pb-1 text-xs font-medium">{{ group.label }}</p>
            <button
              v-for="item in group.items"
              :key="item.instanceId"
              type="button"
              class="hover:bg-muted/50 flex w-full items-start gap-2 rounded-md px-2 py-2 text-left"
              @click="openEvent(item.eventId)"
            >
              <span
                class="mt-0.5 h-8 w-0.5 shrink-0 rounded-full"
                :style="{ background: 'color-mix(in oklab, var(--primary) 45%, transparent)' }"
              />
              <span class="min-w-0 flex-1">
                <span class="block truncate text-sm">{{ item.summary }}</span>
                <span class="text-muted-foreground text-xs">{{ formatWhen(item) }}</span>
              </span>
            </button>
          </section>
        </div>
      </div>
    </div>

    <EventDialog
      :open="dialogOpen"
      mode="edit"
      :calendar-id="calendarId"
      :event="editingEvent"
      @close="dialogOpen = false"
      @save="handleSave"
      @delete="handleDelete"
    />
  </div>
</template>
