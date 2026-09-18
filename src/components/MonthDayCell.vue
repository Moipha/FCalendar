<script setup lang="ts">
import { computed, ref } from "vue";
import {
  PopoverContent,
  PopoverPortal,
  PopoverRoot,
  PopoverTrigger,
} from "reka-ui";

import type { LunarDayInfo } from "@/lib/lunarDay";

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
}>();

const emit = defineEmits<{
  dayClick: [date: string];
  eventClick: [eventId: string];
}>();

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

function onBlankClick() {
  emit("dayClick", props.date);
}

function onEventButtonClick(event: MouseEvent, eventId: string) {
  event.stopPropagation();
  emit("eventClick", eventId);
}

function onPopoverEventClick(eventId: string) {
  overflowOpen.value = false;
  emit("eventClick", eventId);
}
</script>

<template>
  <div
    class="fc-month-day relative flex min-h-0 flex-col p-1 text-left"
    :class="{
      'fc-month-day--today': isToday,
      'fc-month-day--outside': outsideMonth,
    }"
    :data-date="date"
    @click="onBlankClick"
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
          class="fc-month-event block w-full truncate rounded px-1 py-0.5 text-left text-[11px] leading-tight"
          :class="{ 'fc-month-event--outside': outsideMonth }"
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
            @click.stop
          >
            +{{ overflowCount }}
          </button>
        </PopoverTrigger>
        <PopoverPortal>
          <PopoverContent
            class="z-50 w-44 rounded-md border border-border bg-popover p-1 shadow-md outline-none"
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
                class="fc-month-event block w-full truncate rounded px-1 py-0.5 text-left text-[11px] leading-tight"
                @click="onPopoverEventClick(event.eventId)"
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
          v-for="badge in lunar.cornerBadges"
          :key="badge.text"
          class="truncate"
          :class="{
            'font-semibold': ['major', 'jieqi'].includes(badge.kind) && !outsideMonth,
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
  background: var(--background);
  cursor: pointer;
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

.fc-month-day-mark--outside,
.fc-month-day-meta--outside {
  opacity: 0.72;
}

.fc-month-event {
  background: color-mix(in oklab, var(--primary) 16%, var(--background));
  color: var(--foreground);
}

.fc-month-event--outside {
  background: color-mix(in oklab, var(--muted-foreground) 14%, var(--background));
  color: color-mix(in oklab, var(--muted-foreground) 88%, transparent);
}

.fc-jieqi--spring {
  color: #d4537e;
}

.fc-jieqi--summer {
  color: #1f6b4a;
}

.fc-jieqi--autumn {
  color: #c47a12;
}

.fc-jieqi--winter {
  color: #2f7ae5;
}
</style>
