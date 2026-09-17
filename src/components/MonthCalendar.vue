<script setup lang="ts">
import { ScheduleXCalendar } from "@schedule-x/vue";
import { createCalendar, createViewMonthGrid } from "@schedule-x/calendar";
import "@schedule-x/theme-default/dist/index.css";
import { useQuery, useQueryClient } from "@tanstack/vue-query";
import { computed, onBeforeUnmount, ref, shallowRef, watch } from "vue";

import { listCalendars } from "@/api/calendars";
import {
  createEvent,
  deleteEvent,
  getEvent,
  listEvents,
  updateEvent,
  type EventInstance,
  type EventRow,
  type SaveEventInput,
} from "@/api/events";
import EventDialog from "@/components/EventDialog.vue";
import { useWindowControls } from "@/composables/useWindowControls";
import { formatMonthLabel, getMonthGridRange, localTimeZone, toZonedDateTime, weekdayLabels } from "@/lib/datetime";

const monthView = createViewMonthGrid();
const selectedYear = ref(new Date().getFullYear());
const selectedMonth = ref(new Date().getMonth() + 1);
type CalendarController = {
  datePickerState: { selectedDate: { value: Temporal.PlainDate } };
  calendarState: { setView: (view: string, date: Temporal.PlainDate) => void };
};

const appSingleton = shallowRef<CalendarController | null>(null);
const { isMaximized, minimizeWindow, toggleMaximize, closeWindow } = useWindowControls();

const dialogOpen = ref(false);
const dialogMode = ref<"create" | "edit">("create");
const dialogDate = ref<string>();
const editingEvent = ref<EventRow | null>(null);

const queryClient = useQueryClient();
const monthLabel = computed(() => formatMonthLabel(selectedYear.value, selectedMonth.value));
const range = computed(() => getMonthGridRange(selectedYear.value, selectedMonth.value));

const { data: calendars } = useQuery({
  queryKey: ["calendars"],
  queryFn: listCalendars,
});

const calendarId = computed(() => calendars.value?.[0]?.id ?? "");

const { data: instances } = useQuery({
  queryKey: ["events", range, calendarId],
  queryFn: () => listEvents(range.value.from, range.value.to, calendarId.value || undefined),
  enabled: () => Boolean(calendarId.value),
});

function selectedPlainDate() {
  return Temporal.PlainDate.from(
    `${selectedYear.value}-${`${selectedMonth.value}`.padStart(2, "0")}-01`,
  );
}

function scheduleXEventId(instanceId: string) {
  return instanceId.replace(/[^a-zA-Z0-9_-]/g, "_");
}

function toScheduleXEvent(instance: EventInstance) {
  const id = scheduleXEventId(instance.instanceId);
  if (instance.allDay) {
    return {
      id,
      title: instance.summary,
      start: Temporal.PlainDate.from(instance.dtstart),
      end: Temporal.PlainDate.from(instance.dtend || instance.dtstart),
      _eventId: instance.eventId,
    };
  }
  return {
    id,
    title: instance.summary,
    start: toZonedDateTime(instance.dtstart),
    end: toZonedDateTime(instance.dtend),
    _eventId: instance.eventId,
  };
}

function navigateCalendarTo(date: Temporal.PlainDate) {
  const app = appSingleton.value;
  if (!app) {
    return;
  }
  app.datePickerState.selectedDate.value = date;
  app.calendarState.setView(monthView.name, date);
}

const calendarApp = shallowRef(
  createCalendar({
    selectedDate: selectedPlainDate(),
    views: [monthView],
    defaultView: monthView.name,
    firstDayOfWeek: 1,
    locale: "zh-CN",
    timezone: localTimeZone(),
    monthGridOptions: {
      nEventsPerDay: 8,
    },
    events: [],
    callbacks: {
      onRender($app) {
        appSingleton.value = $app;
        navigateCalendarTo(selectedPlainDate());
      },
      onClickDate(date) {
        openCreateDialog(date.toString());
      },
      onEventClick(calendarEvent) {
        const eventId = calendarEvent._eventId as string | undefined;
        if (eventId) {
          void openEditDialog(eventId);
        }
      },
    },
  }),
);

watch(
  instances,
  (value) => {
    try {
      calendarApp.value.events.set(value?.map(toScheduleXEvent) ?? []);
    } catch (error) {
      console.error("failed to render events", error);
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  calendarApp.value.destroy();
});

function shiftMonth(delta: number) {
  const next = selectedPlainDate().add({ months: delta });
  selectedYear.value = next.year;
  selectedMonth.value = next.month;
  navigateCalendarTo(next);
}

function goToday() {
  const now = Temporal.Now.plainDateISO();
  selectedYear.value = now.year;
  selectedMonth.value = now.month;
  navigateCalendarTo(Temporal.PlainDate.from(`${now.year}-${`${now.month}`.padStart(2, "0")}-01`));
}

async function openCreateDialog(date: string) {
  if (!calendarId.value) {
    return;
  }
  dialogMode.value = "create";
  dialogDate.value = date;
  editingEvent.value = null;
  dialogOpen.value = true;
}

async function openEditDialog(eventId: string) {
  editingEvent.value = await getEvent(eventId);
  dialogMode.value = "edit";
  dialogDate.value = editingEvent.value.dtstart.slice(0, 10);
  dialogOpen.value = true;
}

async function invalidateEvents() {
  await queryClient.invalidateQueries({ queryKey: ["events"] });
}

async function handleSave(input: SaveEventInput) {
  if (dialogMode.value === "create") {
    await createEvent(input);
  } else if (editingEvent.value) {
    await updateEvent(editingEvent.value.id, input);
  }
  dialogOpen.value = false;
  await invalidateEvents();
}

async function handleDelete() {
  if (!editingEvent.value) {
    return;
  }
  await deleteEvent(editingEvent.value.id);
  dialogOpen.value = false;
  await invalidateEvents();
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <div class="flex h-12 shrink-0 items-center border-b border-border px-4">
      <div class="text-base font-medium" data-tauri-drag-region>{{ monthLabel }}</div>
      <div class="h-full min-w-4 flex-1" data-tauri-drag-region />
      <div class="ml-auto flex items-center gap-2" @pointerdown.stop>
        <button class="rounded-md border border-border px-3 py-1 text-sm" @click="shiftMonth(-1)">
          上个月
        </button>
        <button class="rounded-md border border-border px-3 py-1 text-sm" @click="goToday">
          回到当前月
        </button>
        <button class="rounded-md border border-border px-3 py-1 text-sm" @click="shiftMonth(1)">
          下个月
        </button>
        <div class="ml-3 flex items-center gap-1">
          <button class="h-8 w-10 rounded-md border border-border text-sm" @click="minimizeWindow">—</button>
          <button
            class="h-8 w-10 rounded-md border border-border text-sm"
            :title="isMaximized ? '还原' : '最大化'"
            @click="toggleMaximize"
          >
            {{ isMaximized ? "❐" : "□" }}
          </button>
          <button class="h-8 w-10 rounded-md border border-border text-sm" @click="closeWindow">×</button>
        </div>
      </div>
    </div>
    <div class="flex min-h-0 flex-1 flex-col p-3">
      <div class="grid shrink-0 grid-cols-7 border-b border-border text-center text-xs text-muted-foreground">
        <div v-for="label in weekdayLabels" :key="label" class="py-2">{{ label }}</div>
      </div>
      <div class="sx-month-fill min-h-0 flex-1">
        <ScheduleXCalendar :calendar-app="calendarApp" />
      </div>
    </div>
    <EventDialog
      :open="dialogOpen"
      :mode="dialogMode"
      :calendar-id="calendarId"
      :initial-date="dialogDate"
      :event="editingEvent"
      @close="dialogOpen = false"
      @save="handleSave"
      @delete="handleDelete"
    />
  </div>
</template>

<style scoped>
.sx-month-fill {
  min-height: 0;
}

.sx-month-fill :deep(.sx-vue-calendar-wrapper),
.sx-month-fill :deep(.sx__calendar-wrapper),
.sx-month-fill :deep(.sx__calendar) {
  height: 100%;
  min-height: 0;
}

:deep(.sx__calendar-header) {
  display: none;
}

:deep(.sx__view-container) {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

:deep(.sx__month-grid-wrapper) {
  height: 100%;
}

:deep(.sx__month-grid-day__header-day-name) {
  display: none;
}
</style>
