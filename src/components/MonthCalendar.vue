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
import MonthDayCell from "@/components/MonthDayCell.vue";
import { formatMonthLabel, toZonedDateTime, weekdayLabels } from "@/lib/datetime";
import { clearLunarDayCache, ensureLunarDays, getLunarDayInfo } from "@/lib/lunarDay";
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
const OVERSCAN = 4;

const selectedYear = ref(new Date().getFullYear());
const selectedMonth = ref(new Date().getMonth() + 1);

const dialogOpen = ref(false);
const dialogMode = ref<"create" | "edit">("create");
const dialogDate = ref<string>();
const editingEvent = ref<EventRow | null>(null);

const queryClient = useQueryClient();
const taskDragStore = useTaskDragStore();

const scrollViewportRef = ref<HTMLElement | null>(null);
const datePickerRef = ref<HTMLInputElement | null>(null);
const viewportHeight = ref(0);
const stripStartMonday = ref<Temporal.PlainDate>(
  buildWeekStrip(todayMonthRef(), BUFFER_MONTHS).firstMonday,
);
const stripWeekCount = ref(buildWeekStrip(todayMonthRef(), BUFFER_MONTHS).weeks.length);

const visibleWeekStart = ref(0);
const visibleWeekEnd = ref(ROWS_VISIBLE + OVERSCAN * 2);

let resizeObserver: ResizeObserver | null = null;
let sliceRaf = 0;
/** 切月 / 选日期等程序滚动期间，顶栏年月锁在目标月，避免 scroll-snap 途中反复改写。 */
let programmaticMonthLock: MonthRef | null = null;
let programmaticUnlockTimer = 0;

const monthLabel = computed(() => formatMonthLabel(selectedYear.value, selectedMonth.value));

const weekHeight = computed(() =>
  viewportHeight.value > 0 ? viewportHeight.value / ROWS_VISIBLE : 0,
);

const weeks = computed(() => enumerateWeeks(stripStartMonday.value, stripWeekCount.value));

const snapMonths = computed(() => monthsIntersectingWeekStrip(weeks.value));

const eventRange = computed(() => weekStripDateRange(weeks.value));

const stripTotalHeight = computed(() => weeks.value.length * weekHeight.value);

const visibleWeeks = computed(() => {
  const slice = weeks.value.slice(visibleWeekStart.value, visibleWeekEnd.value);
  return slice.map((weekMonday, offset) => ({
    weekMonday,
    index: visibleWeekStart.value + offset,
  }));
});

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

function lunarForDate(date: string) {
  return getLunarDayInfo(date);
}

function ensureLunarForVisibleWeeks() {
  const dateKeys: string[] = [];
  for (let index = visibleWeekStart.value; index < visibleWeekEnd.value; index += 1) {
    const weekMonday = weeks.value[index];
    if (!weekMonday) {
      continue;
    }
    for (const day of enumerateDaysInWeek(weekMonday)) {
      dateKeys.push(day.toString());
    }
  }
  ensureLunarDays(dateKeys);
}

function updateVisibleRange(scrollTop: number) {
  if (weekHeight.value <= 0 || weeks.value.length === 0) {
    visibleWeekStart.value = 0;
    visibleWeekEnd.value = Math.min(weeks.value.length, ROWS_VISIBLE + OVERSCAN * 2);
    return;
  }

  const firstVisible = Math.floor(scrollTop / weekHeight.value);
  const start = Math.max(0, firstVisible - OVERSCAN);
  const end = Math.min(weeks.value.length, start + ROWS_VISIBLE + OVERSCAN * 2);
  visibleWeekStart.value = start;
  visibleWeekEnd.value = end;
  ensureLunarForVisibleWeeks();
}

function scheduleVisibleRangeUpdate(scrollTop: number) {
  if (sliceRaf) {
    return;
  }
  sliceRaf = window.requestAnimationFrame(() => {
    sliceRaf = 0;
    updateVisibleRange(scrollTop);
  });
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
  if (programmaticMonthLock) {
    return;
  }
  const nearest = nearestSnapMonth(scrollTop, weekHeight.value, weeks.value, snapMonths.value);
  if (!nearest) {
    return;
  }
  selectedYear.value = nearest.year;
  selectedMonth.value = nearest.month;
}

function lockMonthLabel(month: MonthRef) {
  programmaticMonthLock = month;
  selectedYear.value = month.year;
  selectedMonth.value = month.month;
  window.clearTimeout(programmaticUnlockTimer);
  programmaticUnlockTimer = window.setTimeout(() => {
    releaseMonthLabelLock();
  }, 1500);
}

function releaseMonthLabelLock() {
  programmaticMonthLock = null;
  window.clearTimeout(programmaticUnlockTimer);
  programmaticUnlockTimer = 0;
  const viewport = scrollViewportRef.value;
  if (viewport && weekHeight.value > 0) {
    updateSelectedFromScrollTop(viewport.scrollTop);
  }
}

function onProgrammaticScrollEnd() {
  if (!programmaticMonthLock) {
    return;
  }
  releaseMonthLabelLock();
}

async function ensureMonthInStrip(month: MonthRef) {
  if (snapScrollTopForMonth(month) !== null) {
    return;
  }
  clearLunarDayCache();
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

  lockMonthLabel(month);
  viewport.scrollTo({ top: targetTop, behavior });
  updateVisibleRange(targetTop);
  if (behavior === "auto") {
    releaseMonthLabelLock();
  }
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
      const el = scrollViewportRef.value;
      if (!el) {
        return;
      }
      el.scrollTop = scrollTop + EXTEND_WEEKS * weekHeight.value;
      updateVisibleRange(el.scrollTop);
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
  scheduleVisibleRangeUpdate(viewport.scrollTop);
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
    if (weekHeight.value > 0) {
      updateVisibleRange(viewport.scrollTop);
    }
    return;
  }

  const weekIndex = Math.round(previousScrollTop / previousWeekHeight);
  viewport.scrollTop = nearestWeekScrollTop(weekIndex * weekHeight.value);
  updateSelectedFromScrollTop(viewport.scrollTop);
  updateVisibleRange(viewport.scrollTop);
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

function selectedDateValue() {
  return `${selectedYear.value}-${`${selectedMonth.value}`.padStart(2, "0")}-01`;
}

function openDatePicker() {
  const input = datePickerRef.value;
  if (!input) {
    return;
  }
  input.value = selectedDateValue();
  try {
    input.showPicker();
  } catch {
    input.click();
  }
}

function onDatePicked(event: Event) {
  const value = (event.target as HTMLInputElement).value;
  if (!value) {
    return;
  }
  const date = Temporal.PlainDate.from(value);
  void scrollToMonth({ year: date.year, month: date.month });
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

function onEventClick(eventId: string) {
  if (taskDragStore.shouldSuppressEventClick()) {
    return;
  }
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
  viewport.addEventListener("scrollend", onProgrammaticScrollEnd);

  void initializeScrollPosition();
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  scrollViewportRef.value?.removeEventListener("scrollend", onProgrammaticScrollEnd);
  if (sliceRaf) {
    window.cancelAnimationFrame(sliceRaf);
  }
  window.clearTimeout(programmaticUnlockTimer);
});
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <Teleport defer to="#app-title-leading">
      <button
        type="button"
        class="rounded-md px-1.5 py-0.5 text-base font-medium hover:bg-muted"
        title="选择日期"
        @pointerdown.stop
        @click="openDatePicker"
      >
        {{ monthLabel }}
      </button>
      <input
        ref="datePickerRef"
        type="date"
        class="pointer-events-none fixed h-px w-px opacity-0"
        tabindex="-1"
        :value="selectedDateValue()"
        @change="onDatePicked"
      />
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
        <div
          class="fc-month-weeks"
          :style="{ height: weekHeight > 0 ? `${stripTotalHeight}px` : undefined }"
        >
          <!-- 全带子 snap 占位：保证 mandatory 吸附点覆盖整条带子，内容周仍虚拟挂载 -->
          <div
            v-for="(weekMonday, index) in weeks"
            :key="`snap-${weekMonday.toString()}`"
            class="fc-month-week-snap"
            aria-hidden="true"
            :style="{
              height: weekHeight > 0 ? `${weekHeight}px` : undefined,
              top: weekHeight > 0 ? `${index * weekHeight}px` : undefined,
            }"
          />
          <div
            v-for="{ weekMonday, index } in visibleWeeks"
            :key="weekMonday.toString()"
            class="fc-month-week grid grid-cols-7 border-b border-border"
            :style="{
              height: weekHeight > 0 ? `${weekHeight}px` : undefined,
              top: weekHeight > 0 ? `${index * weekHeight}px` : undefined,
            }"
          >
            <MonthDayCell
              v-for="day in enumerateDaysInWeek(weekMonday)"
              :key="day.toString()"
              class="border-r border-border last:border-r-0"
              :date="day.toString()"
              :day-number="day.day"
              :week-height="weekHeight"
              :is-today="day.toString() === todayIso"
              :outside-month="isOutsideMonth(day)"
              :events="eventsForDate(day.toString())"
              :lunar="lunarForDate(day.toString())"
              @day-click="onDayClick"
              @event-click="onEventClick"
            />
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
  scrollbar-width: none;
}

.fc-month-scroll::-webkit-scrollbar {
  display: none;
}

.fc-month-weeks {
  position: relative;
  min-height: 100%;
}

.fc-month-week-snap {
  position: absolute;
  left: 0;
  right: 0;
  scroll-snap-align: start;
  scroll-snap-stop: normal;
  pointer-events: none;
}

.fc-month-week {
  position: absolute;
  left: 0;
  right: 0;
}
</style>
