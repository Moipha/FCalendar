<script setup lang="ts">
import { ChevronLeft, ChevronRight, Circle } from "@lucide/vue";
import { useQuery, useQueryClient } from "@tanstack/vue-query";
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
} from "vue";

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
import { formatMonthLabel, toZonedDateTime, weekdayLabels } from "@/lib/datetime";
import {
  addMonths,
  buildWeekStrip,
  enumerateDaysInWeek,
  enumerateWeeks,
  monthsIntersectingWeekStrip,
  nearestSnapMonth,
  snapIndexForMonth,
  todayMonthRef,
  weekStripDateRange,
  type MonthRef,
} from "@/lib/monthGrid";
import { useTaskDragStore } from "@/stores/taskDrag";

const ROWS_VISIBLE = 6;
const BUFFER_MONTHS = 6;
const EXTEND_WEEKS = 13;
const EXTEND_THRESHOLD_ROWS = 3;
const MAX_EVENTS_PER_DAY = 8;

const selectedYear = ref(new Date().getFullYear());
const selectedMonth = ref(new Date().getMonth() + 1);

const dialogOpen = ref(false);
const dialogMode = ref<"create" | "edit">("create");
const dialogDate = ref<string>();
const editingEvent = ref<EventRow | null>(null);

const queryClient = useQueryClient();
const taskDragStore = useTaskDragStore();

const scrollViewportRef = ref<HTMLElement | null>(null);
const viewportHeight = ref(0);
const stripStartMonday = ref<Temporal.PlainDate>(
  buildWeekStrip(todayMonthRef(), BUFFER_MONTHS).firstMonday,
);
const stripWeekCount = ref(buildWeekStrip(todayMonthRef(), BUFFER_MONTHS).weeks.length);

let resizeObserver: ResizeObserver | null = null;

const monthLabel = computed(() => formatMonthLabel(selectedYear.value, selectedMonth.value));

const weekHeight = computed(() =>
  viewportHeight.value > 0 ? viewportHeight.value / ROWS_VISIBLE : 0,
);

const weeks = computed(() => enumerateWeeks(stripStartMonday.value, stripWeekCount.value));

const snapMonths = computed(() => monthsIntersectingWeekStrip(weeks.value));

const eventRange = computed(() => weekStripDateRange(weeks.value));

const { data: calendars } = useQuery({
  queryKey: ["calendars"],
  queryFn: listCalendars,
});

const calendarId = computed(() => calendars.value?.[0]?.id ?? "");

const { data: instances } = useQuery({
  queryKey: ["events", eventRange, calendarId],
  queryFn: () => listEvents(eventRange.value.from, eventRange.value.to, calendarId.value || undefined),
  enabled: () => Boolean(calendarId.value && eventRange.value.from && eventRange.value.to),
});

type DayEvent = {
  instanceId: string;
  eventId: string;
  summary: string;
  allDay: boolean;
};

function datesForInstance(instance: EventInstance): string[] {
  if (instance.allDay) {
    const start = Temporal.PlainDate.from(instance.dtstart);
    // list_events 返回的 dtend 为含当日（见 docs/details.md）
    const endInclusive = Temporal.PlainDate.from(instance.dtend || instance.dtstart);
    const dates: string[] = [];
    let cursor = start;
    while (Temporal.PlainDate.compare(cursor, endInclusive) <= 0) {
      dates.push(cursor.toString());
      cursor = cursor.add({ days: 1 });
    }
    return dates;
  }
  return [toZonedDateTime(instance.dtstart).toPlainDate().toString()];
}

const eventsByDate = computed(() => {
  const map = new Map<string, DayEvent[]>();
  for (const instance of instances.value ?? []) {
    const item: DayEvent = {
      instanceId: instance.instanceId,
      eventId: instance.eventId,
      summary: instance.summary,
      allDay: instance.allDay,
    };
    for (const date of datesForInstance(instance)) {
      const bucket = map.get(date);
      if (bucket) {
        bucket.push(item);
      } else {
        map.set(date, [item]);
      }
    }
  }
  return map;
});

const todayIso = computed(() => Temporal.Now.plainDateISO().toString());

function isOutsideMonth(date: Temporal.PlainDate) {
  return date.year !== selectedYear.value || date.month !== selectedMonth.value;
}

function eventsForDate(date: string) {
  return eventsByDate.value.get(date) ?? [];
}

function visibleEventsForDate(date: string) {
  return eventsForDate(date).slice(0, MAX_EVENTS_PER_DAY);
}

function overflowCount(date: string) {
  const total = eventsForDate(date).length;
  return total > MAX_EVENTS_PER_DAY ? total - MAX_EVENTS_PER_DAY : 0;
}

function snapScrollTopForMonth(month: MonthRef) {
  const index = snapIndexForMonth(weeks.value, month.year, month.month);
  if (index < 0) {
    return null;
  }
  return index * weekHeight.value;
}

/** 对齐到最近整周行，避免视口出现半行。 */
function nearestWeekScrollTop(scrollTop: number) {
  if (weekHeight.value <= 0) {
    return scrollTop;
  }
  const maxIndex = Math.max(0, weeks.value.length - ROWS_VISIBLE);
  const index = Math.round(scrollTop / weekHeight.value);
  return Math.min(maxIndex, Math.max(0, index)) * weekHeight.value;
}

function updateSelectedFromScrollTop(scrollTop: number) {
  const nearest = nearestSnapMonth(scrollTop, weekHeight.value, weeks.value, snapMonths.value);
  if (!nearest) {
    return;
  }
  selectedYear.value = nearest.year;
  selectedMonth.value = nearest.month;
}

async function ensureMonthInStrip(month: MonthRef) {
  if (snapScrollTopForMonth(month) !== null) {
    return;
  }
  const strip = buildWeekStrip(month, BUFFER_MONTHS);
  stripStartMonday.value = strip.firstMonday;
  stripWeekCount.value = strip.weeks.length;
  await nextTick();
}

async function scrollToMonth(month: MonthRef, behavior: ScrollBehavior = "smooth") {
  await ensureMonthInStrip(month);
  await nextTick();
  const viewport = scrollViewportRef.value;
  const targetTop = snapScrollTopForMonth(month);
  if (!viewport || targetTop === null) {
    return;
  }

  viewport.scrollTo({ top: targetTop, behavior });
  updateSelectedFromScrollTop(targetTop);
}

function maybeExtendStrip() {
  const viewport = scrollViewportRef.value;
  if (!viewport || weekHeight.value <= 0) {
    return;
  }

  const scrollTop = viewport.scrollTop;
  const maxScroll = viewport.scrollHeight - viewport.clientHeight;
  const threshold = EXTEND_THRESHOLD_ROWS * weekHeight.value;

  if (scrollTop < threshold) {
    stripStartMonday.value = stripStartMonday.value.subtract({ days: EXTEND_WEEKS * 7 });
    stripWeekCount.value += EXTEND_WEEKS;
    nextTick(() => {
      if (scrollViewportRef.value) {
        scrollViewportRef.value.scrollTop = scrollTop + EXTEND_WEEKS * weekHeight.value;
      }
    });
    return;
  }

  if (maxScroll - scrollTop < threshold) {
    stripWeekCount.value += EXTEND_WEEKS;
  }
}

function onScroll() {
  const viewport = scrollViewportRef.value;
  if (!viewport || weekHeight.value <= 0) {
    return;
  }

  updateSelectedFromScrollTop(viewport.scrollTop);
  maybeExtendStrip();
}

function updateViewportHeight() {
  const viewport = scrollViewportRef.value;
  if (!viewport) {
    return;
  }

  const previousWeekHeight = weekHeight.value;
  const previousScrollTop = viewport.scrollTop;
  viewportHeight.value = viewport.clientHeight;

  if (previousWeekHeight <= 0 || weekHeight.value <= 0) {
    return;
  }

  const weekIndex = Math.round(previousScrollTop / previousWeekHeight);
  viewport.scrollTop = nearestWeekScrollTop(weekIndex * weekHeight.value);
  updateSelectedFromScrollTop(viewport.scrollTop);
}

async function initializeScrollPosition() {
  await nextTick();
  updateViewportHeight();
  await scrollToMonth(todayMonthRef(), "auto");
}

function shiftMonth(delta: number) {
  const target = addMonths({ year: selectedYear.value, month: selectedMonth.value }, delta);
  void scrollToMonth(target);
}

function goToday() {
  void scrollToMonth(todayMonthRef());
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

function onDayClick(date: string) {
  if (taskDragStore.shouldSuppressDateClick()) {
    return;
  }
  void openCreateDialog(date);
}

function onEventClick(event: MouseEvent, eventId: string) {
  event.stopPropagation();
  void openEditDialog(eventId);
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

onMounted(() => {
  const viewport = scrollViewportRef.value;
  if (!viewport) {
    return;
  }

  resizeObserver = new ResizeObserver(() => {
    updateViewportHeight();
  });
  resizeObserver.observe(viewport);

  void initializeScrollPosition();
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
});
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <Teleport defer to="#app-title-leading">
      <div class="text-base font-medium" data-tauri-drag-region>{{ monthLabel }}</div>
      <div class="h-full min-w-4 flex-1" data-tauri-drag-region />
      <div class="flex items-center gap-1" @pointerdown.stop>
        <button
          class="flex h-8 w-8 items-center justify-center rounded-md border border-border"
          title="上个月"
          @click="shiftMonth(-1)"
        >
          <ChevronLeft class="size-4" />
        </button>
        <button
          class="flex h-8 w-8 items-center justify-center rounded-md border border-border"
          title="回到当前月"
          @click="goToday"
        >
          <Circle class="size-3.5" />
        </button>
        <button
          class="flex h-8 w-8 items-center justify-center rounded-md border border-border"
          title="下个月"
          @click="shiftMonth(1)"
        >
          <ChevronRight class="size-4" />
        </button>
      </div>
    </Teleport>

    <div class="flex min-h-0 flex-1 flex-col p-3">
      <div class="grid shrink-0 grid-cols-7 border-b border-border text-center text-xs text-muted-foreground">
        <div v-for="label in weekdayLabels" :key="label" class="py-2">{{ label }}</div>
      </div>

      <div
        ref="scrollViewportRef"
        class="fc-month-scroll min-h-0 flex-1 overflow-y-auto"
        @scroll="onScroll"
      >
        <div class="fc-month-weeks">
          <div
            v-for="weekMonday in weeks"
            :key="weekMonday.toString()"
            class="fc-month-week grid grid-cols-7 border-b border-border"
            :style="{ height: weekHeight > 0 ? `${weekHeight}px` : undefined }"
          >
            <button
              v-for="day in enumerateDaysInWeek(weekMonday)"
              :key="day.toString()"
              type="button"
              class="fc-month-day flex min-h-0 flex-col border-r border-border p-1 text-left last:border-r-0"
              :class="{
                'fc-month-day--today': day.toString() === todayIso,
                'fc-month-day--outside': isOutsideMonth(day),
              }"
              :data-date="day.toString()"
              @click="onDayClick(day.toString())"
            >
              <span
                class="mb-1 inline-flex size-6 shrink-0 items-center justify-center rounded-full text-xs font-medium"
                :class="day.toString() === todayIso ? 'bg-primary text-primary-foreground' : ''"
              >
                {{ day.day }}
              </span>
              <div class="min-h-0 flex-1 space-y-0.5 overflow-hidden">
                <button
                  v-for="event in visibleEventsForDate(day.toString())"
                  :key="event.instanceId"
                  type="button"
                  class="fc-month-event block w-full truncate rounded px-1 py-0.5 text-left text-[11px] leading-tight"
                  :class="{ 'fc-month-event--outside': isOutsideMonth(day) }"
                  @click="onEventClick($event, event.eventId)"
                >
                  {{ event.summary }}
                </button>
                <div
                  v-if="overflowCount(day.toString()) > 0"
                  class="truncate px-1 text-[10px] text-muted-foreground"
                >
                  +{{ overflowCount(day.toString()) }}
                </div>
              </div>
            </button>
          </div>
        </div>
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
.fc-month-scroll {
  overflow-anchor: none;
  overscroll-behavior: contain;
  scroll-snap-type: y mandatory;
}

.fc-month-weeks {
  min-height: 100%;
}

.fc-month-week {
  scroll-snap-align: start;
  scroll-snap-stop: normal;
}

.fc-month-day {
  background: var(--background);
  transition: background-color 0.12s ease;
}

.fc-month-day:hover {
  background: color-mix(in oklab, var(--muted) 35%, var(--background));
}

.fc-month-day--today {
  box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--primary) 70%, transparent);
}

.fc-month-day--outside {
  background: color-mix(in oklab, var(--muted) 55%, var(--background));
  color: color-mix(in oklab, var(--muted-foreground) 85%, transparent);
}

.fc-month-day--outside:hover {
  background: color-mix(in oklab, var(--muted) 70%, var(--background));
}

.fc-month-event {
  background: color-mix(in oklab, var(--primary) 16%, var(--background));
  color: var(--foreground);
}

.fc-month-event--outside {
  background: color-mix(in oklab, var(--muted-foreground) 14%, var(--background));
  color: color-mix(in oklab, var(--muted-foreground) 88%, transparent);
}
</style>
