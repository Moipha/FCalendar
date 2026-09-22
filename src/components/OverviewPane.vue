<script setup lang="ts">
import { useQuery, useQueryClient } from "@tanstack/vue-query";
import { ChevronDown, ChevronRight } from "@lucide/vue";
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
import { toZonedDateTime } from "@/lib/datetime";
import { colloquialUntil, nextHolidayFrom, startOfDayInstant } from "@/lib/holiday";
import { useSettingsStore } from "@/stores/settings";

const props = defineProps<{
  calendarId: string;
}>();

const settings = useSettingsStore();
const queryClient = useQueryClient();

const todayIso = computed(() => Temporal.Now.plainDateISO().toString());

const eventRange = computed(() => {
  const from = todayIso.value;
  const to = Temporal.PlainDate.from(from).add({ days: settings.upcomingDays }).toString();
  return { from, to };
});

const calendarId = computed(() => props.calendarId);

const { data: instances } = useQuery({
  queryKey: ["events", "overview", eventRange, calendarId],
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

const upcomingEvents = computed(() => {
  const now = Temporal.Now.instant();
  const today = todayIso.value;
  return [...(instances.value ?? [])]
    .filter((item) => instanceStillOpen(item, now, today))
    .sort((a, b) => instanceStartSortKey(a).localeCompare(instanceStartSortKey(b)));
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
    return `全天 · ${formatMd(start)}`;
  }
  const startDate = toZonedDateTime(instance.dtstart).toPlainDate().toString();
  return `${formatMd(startDate)} ${formatHm(instance.dtstart)}`;
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
  dialogOpen.value = false;
  editingEvent.value = null;
  await queryClient.invalidateQueries({ queryKey: ["events"] });
}

async function handleDelete() {
  if (!editingEvent.value) {
    return;
  }
  await deleteEvent(editingEvent.value.id);
  dialogOpen.value = false;
  editingEvent.value = null;
  await queryClient.invalidateQueries({ queryKey: ["events"] });
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
        class="mt-5 flex w-full items-center justify-between py-2 text-left text-xs font-medium text-muted-foreground hover:bg-muted/30"
        :title="upcomingExpanded ? '收起' : '展开'"
        @click="upcomingExpanded = !upcomingExpanded"
      >
        <span>待进行（{{ settings.upcomingDays }} 天内）</span>
        <ChevronDown v-if="upcomingExpanded" class="size-4" aria-hidden="true" />
        <ChevronRight v-else class="size-4" aria-hidden="true" />
      </button>
      <div v-show="upcomingExpanded">
        <div v-if="upcomingEvents.length === 0" class="py-4 text-xs text-muted-foreground">
          这段时间没有待进行的事件
        </div>
        <button
          v-for="item in upcomingEvents"
          :key="item.instanceId"
          type="button"
          class="flex w-full flex-col gap-0.5 rounded-md px-2 py-2 text-left hover:bg-muted/50"
          @click="openEvent(item.eventId)"
        >
          <span class="truncate text-sm">{{ item.summary }}</span>
          <span class="text-xs text-muted-foreground">{{ formatWhen(item) }}</span>
        </button>
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
