<script setup lang="ts">
import { ChevronLeft, ChevronRight, Circle } from "@lucide/vue";
import { useQuery, useQueryClient } from "@tanstack/vue-query";
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  shallowRef,
  watch,
} from "vue";

import { useSessionStore } from "@/stores/session";
import {
  listDayColors,
  setDayColor,
  setDayColors,
  type DayColorPreset,
  type DayColorRow,
} from "@/api/dayColors";
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
import MonthDayContextMenu from "@/components/MonthDayContextMenu.vue";
import { formatMonthLabel, toZonedDateTime, weekdayLabelsFor } from "@/lib/datetime";
import { normalizeDayColorPreset, type DayCellTint } from "@/lib/dayCellColors";
import { dateRangeInclusiveInStrip } from "@/lib/daySelection";
import { dayColorsQueryKey } from "@/lib/dayColorsQuery";
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
import { useSettingsStore } from "@/stores/settings";
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

type DayContextMenuState = {
  date: string;
  x: number;
  y: number;
  showColorPicker: boolean;
  mode: "single" | "batch";
  batchDates: string[];
};

const dayContextMenu = ref<DayContextMenuState | null>(null);
const dayContextMenuRef = ref<HTMLElement | null>(null);

const selectedDayDates = shallowRef(new Set<string>());
const selectionAnchorDate = ref<string | null>(null);

const queryClient = useQueryClient();
const settings = useSettingsStore();
const taskDragStore = useTaskDragStore();

const weekStart = computed(() => settings.weekStart);
const weekdayLabels = computed(() => weekdayLabelsFor(weekStart.value));
const scrollSnapWeeks = computed(() => settings.scrollSnapWeeks);

function initialStrip() {
  return buildWeekStrip(todayMonthRef(), BUFFER_MONTHS, settings.weekStart);
}

const scrollViewportRef = ref<HTMLElement | null>(null);
const datePickerRef = ref<HTMLInputElement | null>(null);
const viewportHeight = ref(0);
const stripStartMonday = ref<Temporal.PlainDate>(initialStrip().firstMonday);
const stripWeekCount = ref(initialStrip().weeks.length);

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

const session = useSessionStore();
const calendarId = computed(() => session.currentCalendarId);

const { data: instances } = useQuery({
  queryKey: ["events", eventRange, calendarId],
  queryFn: () => listEvents(eventRange.value.from, eventRange.value.to, calendarId.value || undefined),
  enabled: () => Boolean(calendarId.value && eventRange.value.from && eventRange.value.to),
});

const dayColorsKey = computed(() =>
  dayColorsQueryKey(eventRange.value.from, eventRange.value.to, calendarId.value),
);

const { data: dayColorRows } = useQuery({
  queryKey: dayColorsKey,
  queryFn: () =>
    listDayColors(eventRange.value.from, eventRange.value.to, calendarId.value || undefined),
  enabled: () => Boolean(calendarId.value && eventRange.value.from && eventRange.value.to),
});

/** 日格底色：与查询同步，改色时先本地更新以保证格子立刻变色。 */
const dayColorByDate = shallowRef(new Map<string, DayColorPreset>());

watch(
  dayColorRows,
  (rows) => {
    const next = new Map<string, DayColorPreset>();
    for (const row of rows ?? []) {
      const preset = normalizeDayColorPreset(row.color);
      if (preset) {
        next.set(row.date, preset);
      }
    }
    dayColorByDate.value = next;
  },
  { immediate: true },
);

function dayTintForDate(date: string): DayCellTint {
  return dayColorByDate.value.get(date) ?? "default";
}

function patchDayColorsLocal(dates: string[], preset: DayColorPreset | null) {
  const next = new Map(dayColorByDate.value);
  for (const date of dates) {
    if (preset) {
      next.set(date, preset);
    } else {
      next.delete(date);
    }
  }
  dayColorByDate.value = next;
}

function isDaySelected(date: string) {
  return selectedDayDates.value.has(date);
}

function clearDaySelection() {
  selectedDayDates.value = new Set();
  selectionAnchorDate.value = null;
}

function setSelectedDays(dates: Iterable<string>) {
  selectedDayDates.value = new Set(dates);
}

function rangeBetweenAnchorAnd(date: string): string[] {
  const anchor = selectionAnchorDate.value;
  if (!anchor) {
    return [date];
  }
  return dateRangeInclusiveInStrip(weeks.value, anchor, date);
}

function applyDayColorsToQueryCache(dates: string[], preset: DayColorPreset | null) {
  queryClient.setQueryData<DayColorRow[]>(dayColorsKey.value, (old) => {
    const remove = new Set(dates);
    const next = (old ?? []).filter((row) => !remove.has(row.date));
    if (preset) {
      for (const date of dates) {
        next.push({ date, color: preset });
      }
      next.sort((a, b) => a.date.localeCompare(b.date));
    }
    return next;
  });
}

async function persistDayColors(dates: string[], preset: DayColorPreset | null) {
  if (dates.length === 0) {
    return;
  }
  if (dates.length === 1) {
    await setDayColor(dates[0], preset, calendarId.value || undefined);
    return;
  }
  await setDayColors(dates, preset, calendarId.value || undefined);
}

function selectionHasAnyTint(dates: string[]) {
  return dates.some((date) => dayTintForDate(date) !== "default");
}

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
  const index = snapIndexForMonth(weeks.value, month.year, month.month, weekStart.value);
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
  const nearest = nearestSnapMonth(
    scrollTop,
    weekHeight.value,
    weeks.value,
    snapMonths.value,
    weekStart.value,
  );
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
  const strip = buildWeekStrip(month, BUFFER_MONTHS, weekStart.value);
  stripStartMonday.value = strip.firstMonday;
  stripWeekCount.value = strip.weeks.length;
  await nextTick();
}

watch(weekStart, async () => {
  clearLunarDayCache();
  const month = { year: selectedYear.value, month: selectedMonth.value };
  await ensureMonthInStrip(month);
  await scrollToMonth(month, "auto");
});

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

  closeDayContextMenu();
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

function closeDayContextMenu(options?: { suppressClick?: boolean }) {
  if (!dayContextMenu.value) {
    return;
  }
  dayContextMenu.value = null;
  if (options?.suppressClick) {
    taskDragStore.suppressDateClick();
    taskDragStore.suppressEventClick();
  }
}

function onDayContextMenu(date: string, event: MouseEvent) {
  if (taskDragStore.active) {
    return;
  }

  const selected = selectedDayDates.value;
  const isOnSelection = selected.has(date) && selected.size > 1;

  if (!isOnSelection) {
    clearDaySelection();
  }

  dayContextMenu.value = {
    date,
    x: event.clientX,
    y: event.clientY,
    showColorPicker: false,
    mode: isOnSelection ? "batch" : "single",
    batchDates: isOnSelection ? [...selected] : [date],
  };
}

function onDayContextMenuCreate() {
  const date = dayContextMenu.value?.date;
  closeDayContextMenu();
  if (date) {
    void openCreateDialog(date);
  }
}

function onDayContextMenuExpandColors() {
  if (!dayContextMenu.value) {
    return;
  }
  dayContextMenu.value = {
    ...dayContextMenu.value,
    showColorPicker: true,
  };
}

function onDayContextMenuCollapseColors() {
  if (!dayContextMenu.value?.showColorPicker) {
    return;
  }
  dayContextMenu.value = {
    ...dayContextMenu.value,
    showColorPicker: false,
  };
}

async function onDayContextMenuPickColor(tint: DayCellTint) {
  const menu = dayContextMenu.value;
  if (!menu) {
    return;
  }
  const dates = menu.mode === "batch" ? menu.batchDates : [menu.date];
  const preset = tint === "default" ? null : tint;

  patchDayColorsLocal(dates, preset);
  applyDayColorsToQueryCache(dates, preset);
  closeDayContextMenu();

  try {
    await persistDayColors(dates, preset);
  } catch {
    await queryClient.invalidateQueries({ queryKey: ["dayColors"] });
  }
}

async function onDayContextMenuClearColors() {
  const menu = dayContextMenu.value;
  if (!menu) {
    return;
  }
  const dates = menu.mode === "batch" ? menu.batchDates : [menu.date];

  patchDayColorsLocal(dates, null);
  applyDayColorsToQueryCache(dates, null);
  closeDayContextMenu();

  try {
    await persistDayColors(dates, null);
  } catch {
    await queryClient.invalidateQueries({ queryKey: ["dayColors"] });
  }
}

const dayContextMenuShowClear = computed(() => {
  const menu = dayContextMenu.value;
  if (!menu) {
    return false;
  }
  const dates = menu.mode === "batch" ? menu.batchDates : [menu.date];
  return selectionHasAnyTint(dates);
});

function onDocumentPointerDown(event: PointerEvent) {
  if (!dayContextMenu.value) {
    return;
  }
  const target = event.target as Node | null;
  if (target && dayContextMenuRef.value?.contains(target)) {
    return;
  }
  closeDayContextMenu({ suppressClick: event.button === 0 });
}

function onDocumentKeyDown(event: KeyboardEvent) {
  if (event.key !== "Escape") {
    return;
  }
  if (dayContextMenu.value) {
    event.preventDefault();
    closeDayContextMenu();
    return;
  }
  if (selectedDayDates.value.size > 0) {
    event.preventDefault();
    clearDaySelection();
  }
}

function onDayClick(date: string, event: MouseEvent) {
  if (taskDragStore.shouldSuppressDateClick()) {
    return;
  }

  const ctrl = event.ctrlKey;
  const shift = event.shiftKey;

  if (ctrl && shift) {
    event.preventDefault();
    const range = rangeBetweenAnchorAnd(date);
    if (!selectionAnchorDate.value) {
      setSelectedDays([date]);
      selectionAnchorDate.value = date;
      return;
    }
    const merged = new Set(selectedDayDates.value);
    for (const d of range) {
      merged.add(d);
    }
    setSelectedDays(merged);
    return;
  }

  if (shift) {
    event.preventDefault();
    if (!selectionAnchorDate.value) {
      setSelectedDays([date]);
      selectionAnchorDate.value = date;
      return;
    }
    setSelectedDays(rangeBetweenAnchorAnd(date));
    return;
  }

  if (ctrl) {
    event.preventDefault();
    const next = new Set(selectedDayDates.value);
    if (next.has(date)) {
      next.delete(date);
    } else {
      next.add(date);
    }
    setSelectedDays(next);
    selectionAnchorDate.value = date;
    return;
  }

  if (selectedDayDates.value.size > 0) {
    clearDaySelection();
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
  document.addEventListener("pointerdown", onDocumentPointerDown, true);
  document.addEventListener("keydown", onDocumentKeyDown);

  void initializeScrollPosition();
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  scrollViewportRef.value?.removeEventListener("scrollend", onProgrammaticScrollEnd);
  document.removeEventListener("pointerdown", onDocumentPointerDown, true);
  document.removeEventListener("keydown", onDocumentKeyDown);
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
        :class="{ 'fc-month-scroll--snap': scrollSnapWeeks }"
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
              :day-tint="dayTintForDate(day.toString())"
              :selected="isDaySelected(day.toString())"
              :show-minor-festivals="settings.showMinorFestivals"
              @day-click="onDayClick"
              @event-click="onEventClick"
              @day-context-menu="onDayContextMenu"
            />
          </div>
        </div>
      </div>
    </div>

    <Teleport to="body">
      <div
        v-if="dayContextMenu"
        ref="dayContextMenuRef"
      >
        <MonthDayContextMenu
          :x="dayContextMenu.x"
          :y="dayContextMenu.y"
          :current-tint="dayTintForDate(dayContextMenu.date)"
          :show-color-picker="dayContextMenu.showColorPicker"
          :mode="dayContextMenu.mode"
          :show-clear-color="dayContextMenuShowClear"
          @create="onDayContextMenuCreate"
          @clear-colors="onDayContextMenuClearColors"
          @expand-colors="onDayContextMenuExpandColors"
          @collapse-colors="onDayContextMenuCollapseColors"
          @pick-color="onDayContextMenuPickColor"
        />
      </div>
    </Teleport>

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
  scrollbar-width: none;
}

.fc-month-scroll--snap {
  scroll-snap-type: y mandatory;
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
