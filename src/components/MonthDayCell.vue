<script setup lang="ts">
import { computed, inject, ref, watch } from "vue";
import {
  PopoverContent,
  PopoverPortal,
  PopoverRoot,
  PopoverTrigger,
} from "reka-ui";

import { DRAG_DROP_KEY } from "@/composables/useDragDrop";
import { DAY_CELL_TINT_FILL, DAY_CELL_TINT_HOVER, type DayCellTint } from "@/lib/dayCellColors";
import type { LunarDayInfo } from "@/lib/lunarDay";
import { useTaskDragStore } from "@/stores/taskDrag";

export type MonthDayEvent = {
  instanceId: string;
  eventId: string;
  summary: string;
};

const props = defineProps<{
  date: string;
  dayNumber: number;
  weekHeight: number;
  isToday: boolean;
  outsideMonth: boolean;
  events: MonthDayEvent[];
  lunar: LunarDayInfo;
  dayTint: DayCellTint;
  selected: boolean;
  showMinorFestivals: boolean;
}>();

const cornerBadges = computed(() => {
  if (props.showMinorFestivals) {
    return props.lunar.cornerBadges;
  }
  return props.lunar.cornerBadges.filter((badge) => badge.kind !== "minor");
});

const emit = defineEmits<{
  dayClick: [date: string, event: MouseEvent];
  eventClick: [eventId: string];
  dayContextMenu: [date: string, event: MouseEvent];
}>();

function isSelectionModifier(event: MouseEvent | PointerEvent) {
  return event.ctrlKey || event.shiftKey;
}

const dragDrop = inject(DRAG_DROP_KEY)!;
const { startEventDrag } = dragDrop;

const taskDragStore = useTaskDragStore();
const overflowOpen = ref(false);

const DAY_PADDING_Y = 8;
const HEADER_HEIGHT = 24;
const FOOTER_HEIGHT = 18;
const OVERFLOW_SLOT_HEIGHT = 18;
const EVENT_ITEM_HEIGHT = 18;

const maxVisibleEvents = computed(() => {
  if (props.weekHeight <= 0) {
    return 0;
  }
  const eventArea =
    props.weekHeight - DAY_PADDING_Y - HEADER_HEIGHT - FOOTER_HEIGHT - OVERFLOW_SLOT_HEIGHT;
  return Math.max(0, Math.floor(eventArea / EVENT_ITEM_HEIGHT));
});

const visibleEvents = computed(() => props.events.slice(0, maxVisibleEvents.value));

const overflowCount = computed(() =>
  Math.max(0, props.events.length - visibleEvents.value.length),
);

watch(
  () => taskDragStore.active,
  (active) => {
    if (active) {
      overflowOpen.value = false;
    }
  },
);

function onBlankClick(event: MouseEvent) {
  emit("dayClick", props.date, event);
}

function onMouseDown(event: MouseEvent) {
  if (event.button !== 0 || !isSelectionModifier(event)) {
    return;
  }
  event.preventDefault();
  window.getSelection()?.removeAllRanges();
}

function onEventButtonClick(event: MouseEvent, eventId: string) {
  event.stopPropagation();
  if (isSelectionModifier(event)) {
    emit("dayClick", props.date, event);
    return;
  }
  if (taskDragStore.shouldSuppressEventClick()) {
    return;
  }
  emit("eventClick", eventId);
}

function onEventPointerDown(pointerEvent: PointerEvent, eventId: string, title: string) {
  if (isSelectionModifier(pointerEvent)) {
    return;
  }
  startEventDrag(
    {
      eventId,
      sourceDate: props.date,
      title,
    },
    pointerEvent,
  );
}

function onPopoverEventClick(event: MouseEvent, eventId: string) {
  overflowOpen.value = false;
  if (isSelectionModifier(event)) {
    emit("dayClick", props.date, event);
    return;
  }
  if (taskDragStore.shouldSuppressEventClick()) {
    return;
  }
  emit("eventClick", eventId);
}

function onOverflowClick(event: MouseEvent) {
  event.stopPropagation();
  if (isSelectionModifier(event)) {
    emit("dayClick", props.date, event);
  }
}

function onContextMenu(event: MouseEvent) {
  event.preventDefault();
  overflowOpen.value = false;
  emit("dayContextMenu", props.date, event);
}

/** 行内自定义属性，优先级高于 scoped 类，避免底色被默认白盖掉。 */
const daySurfaceStyle = computed(() => {
  const shadows: string[] = [];
  if (props.isToday) {
    shadows.push("inset 0 0 0 1px color-mix(in oklab, var(--primary) 70%, transparent)");
  }
  return {
    "--fc-day-base": DAY_CELL_TINT_FILL[props.dayTint],
    "--fc-day-base-hover": DAY_CELL_TINT_HOVER[props.dayTint],
    ...(shadows.length > 0 ? { boxShadow: shadows.join(", ") } : {}),
  };
});
</script>

<template>
  <div
    class="fc-month-day relative flex h-full min-h-0 w-full flex-col p-1 text-left"
    :class="{
      'fc-month-day--today': isToday,
      'fc-month-day--outside': outsideMonth,
      'fc-month-day--tint-green': dayTint === 'green',
      'fc-month-day--tint-red': dayTint === 'red',
      'fc-month-day--selected': selected,
    }"
    :data-date="date"
    :style="daySurfaceStyle"
    @mousedown="onMouseDown"
    @selectstart.prevent
    @click="onBlankClick"
    @contextmenu="onContextMenu"
  >
    <div class="mb-1 flex shrink-0 items-start justify-between gap-1">
      <span
        class="inline-flex size-6 shrink-0 items-center justify-center rounded-full text-xs font-medium"
        :class="isToday ? 'bg-primary text-primary-foreground' : ''"
      >
        {{ dayNumber }}
      </span>
      <span
        v-if="lunar.workMark === 'off'"
        class="inline-flex size-4.5 shrink-0 items-center justify-center rounded-full border border-destructive/40 bg-destructive/10 text-[12px] font-medium text-destructive"
        :class="{ 'fc-month-day-mark--outside': outsideMonth }"
      >
        假
      </span>
      <span
        v-else-if="lunar.workMark === 'work'"
        class="inline-flex size-4.5 shrink-0 items-center justify-center rounded-full border border-border bg-muted text-[12px] font-medium text-muted-foreground"
        :class="{ 'fc-month-day-mark--outside': outsideMonth }"
      >
        班
      </span>
    </div>

    <div class="min-h-0 flex-1 overflow-hidden">
      <div class="space-y-0.5">
        <button
          v-for="event in visibleEvents"
          :key="event.instanceId"
          type="button"
          class="fc-month-event relative block w-full truncate rounded py-0.5 pr-1 pl-2 text-left text-[11px] leading-tight"
          :class="{ 'fc-month-event--outside': outsideMonth }"
          @pointerdown.stop="onEventPointerDown($event, event.eventId, event.summary)"
          @click="onEventButtonClick($event, event.eventId)"
        >
          {{ event.summary }}
        </button>
      </div>
    </div>

    <div class="flex h-[18px] shrink-0 items-center">
      <PopoverRoot v-if="overflowCount > 0" v-model:open="overflowOpen">
        <PopoverTrigger as-child>
          <button
            type="button"
            class="inline-flex size-4 items-center justify-center rounded-full border border-border bg-muted/70 text-[9px] font-medium text-muted-foreground"
            @click="onOverflowClick"
          >
            +{{ overflowCount }}
          </button>
        </PopoverTrigger>
        <PopoverPortal>
          <PopoverContent
            class="fc-month-event-popover z-50 w-44 rounded-md border border-border bg-popover p-1 shadow-md outline-none"
            side="right"
            align="start"
            :side-offset="6"
            :collision-padding="8"
            @click.stop
          >
            <div class="max-h-48 space-y-0.5 overflow-y-auto">
              <button
                v-for="event in events"
                :key="event.instanceId"
                type="button"
                class="fc-month-event relative block w-full truncate rounded py-0.5 pr-1 pl-2 text-left text-[11px] leading-tight"
                @pointerdown.stop="onEventPointerDown($event, event.eventId, event.summary)"
                @click="onPopoverEventClick($event, event.eventId)"
              >
                {{ event.summary }}
              </button>
            </div>
          </PopoverContent>
        </PopoverPortal>
      </PopoverRoot>
    </div>

    <div class="flex shrink-0 items-end justify-between gap-1 text-[12px] leading-none">
      <span
        class="truncate text-muted-foreground"
        :class="{ 'fc-month-day-meta--outside': outsideMonth }"
      >
        {{ lunar.lunarText }}
      </span>
      <div class="flex min-w-0 shrink items-center justify-end gap-1">
        <span
          v-for="badge in cornerBadges"
          :key="badge.text"
          class="truncate"
          :class="{
            'font-semibold text-primary': badge.kind === 'major' && !outsideMonth,
            'font-semibold': badge.kind === 'jieqi' && !outsideMonth,
            'fc-jieqi--spring': badge.kind === 'jieqi' && badge.season === 'spring' && !outsideMonth,
            'fc-jieqi--summer': badge.kind === 'jieqi' && badge.season === 'summer' && !outsideMonth,
            'fc-jieqi--autumn': badge.kind === 'jieqi' && badge.season === 'autumn' && !outsideMonth,
            'fc-jieqi--winter': badge.kind === 'jieqi' && badge.season === 'winter' && !outsideMonth,
            'text-muted-foreground fc-month-day-meta--outside': outsideMonth,
          }"
        >
          {{ badge.text }}
        </span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.fc-month-day {
  --fc-day-bg: var(--fc-day-base, var(--background));
  background-color: var(--fc-day-bg);
  cursor: pointer;
  user-select: none;
  transition: background-color 0.12s ease;
}

.fc-month-day:hover:not(.fc-month-day--outside) {
  --fc-day-bg: var(--fc-day-base-hover, var(--fc-day-base, var(--background)));
}

.fc-month-day--selected::after {
  content: "";
  position: absolute;
  inset: 1px;
  pointer-events: none;
  background:
    linear-gradient(90deg, var(--primary) 50%, transparent 50%) 0 0 / 8px 2px repeat-x,
    linear-gradient(90deg, var(--primary) 50%, transparent 50%) 0 100% / 8px 2px repeat-x,
    linear-gradient(0deg, var(--primary) 50%, transparent 50%) 0 0 / 2px 8px repeat-y,
    linear-gradient(0deg, var(--primary) 50%, transparent 50%) 100% 0 / 2px 8px repeat-y;
  animation: fc-day-select-march 0.45s linear infinite;
}

@keyframes fc-day-select-march {
  to {
    background-position:
      8px 0,
      -8px 100%,
      0 -8px,
      100% 8px;
  }
}

.fc-month-day--outside {
  --fc-day-bg: color-mix(in oklab, var(--muted) 55%, var(--fc-day-base, var(--background)));
  color: color-mix(in oklab, var(--muted-foreground) 85%, transparent);
}

.fc-month-day-mark--outside,
.fc-month-day-meta--outside {
  opacity: 0.72;
}

.fc-month-event {
  background: color-mix(in oklab, var(--primary) 16%, var(--background));
  color: var(--foreground);
  cursor: grab;
}

.fc-month-event::before {
  content: "";
  position: absolute;
  top: 3px;
  bottom: 3px;
  left: 3px;
  width: 2px;
  border-radius: 1px;
  background: color-mix(in oklab, var(--fc-day-bg, var(--background)) 80%, #333);
}

.fc-month-event:active {
  cursor: grabbing;
}

.fc-month-event--outside {
  background: color-mix(in oklab, var(--muted-foreground) 14%, var(--background));
  color: color-mix(in oklab, var(--muted-foreground) 88%, transparent);
}

:deep(.fc-month-event-popover) {
  --fc-day-bg: var(--popover);
}

.fc-jieqi--spring {
  color: var(--fc-jieqi-spring);
}

.fc-jieqi--summer {
  color: var(--fc-jieqi-summer);
}

.fc-jieqi--autumn {
  color: var(--fc-jieqi-autumn);
}

.fc-jieqi--winter {
  color: var(--fc-jieqi-winter);
}
</style>
