<script setup lang="ts">
import type { DateValue } from "reka-ui";
import { computed, ref } from "vue";

import { Calendar } from "@/components/ui/calendar";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  formatPlainDateLabel,
  fromCalendarDate,
  HOUR_OPTIONS,
  joinDatetimeLocal,
  MINUTE_OPTIONS,
  splitDatetimeLocal,
  toCalendarDate,
  weekStartsOnFromSettings,
} from "@/lib/calendarDate";
import { useSettingsStore } from "@/stores/settings";

const props = defineProps<{
  modelValue: string;
  disabled?: boolean;
}>();

const emit = defineEmits<{
  "update:modelValue": [string];
}>();

const settings = useSettingsStore();
const open = ref(false);

const weekStartsOn = computed(() => weekStartsOnFromSettings(settings.weekStart));
const parts = computed(() => splitDatetimeLocal(props.modelValue));

const calendarValue = computed(() => toCalendarDate(parts.value.date));

const label = computed(() => {
  const { date, hour, minute } = parts.value;
  return `${formatPlainDateLabel(date)} ${hour}:${minute}`;
});

function emitParts(next: { date?: string; hour?: string; minute?: string }) {
  emit(
    "update:modelValue",
    joinDatetimeLocal(
      next.date ?? parts.value.date,
      next.hour ?? parts.value.hour,
      next.minute ?? parts.value.minute,
    ),
  );
}

function onSelectDate(value: DateValue | DateValue[] | undefined) {
  if (!value || Array.isArray(value)) {
    return;
  }
  emitParts({ date: fromCalendarDate(value) });
}

function onHour(value: unknown) {
  if (typeof value === "string") {
    emitParts({ hour: value });
  }
}

function onMinute(value: unknown) {
  if (typeof value === "string") {
    emitParts({ minute: value });
  }
}
</script>

<template>
  <Popover v-model:open="open">
    <PopoverTrigger as-child :disabled="disabled">
      <Button type="button" variant="outline" class="w-full justify-start font-normal" :disabled="disabled">
        {{ label }}
      </Button>
    </PopoverTrigger>
    <PopoverContent class="w-auto p-0" align="start">
      <Calendar
        :model-value="calendarValue"
        locale="zh-CN"
        :week-starts-on="weekStartsOn"
        @update:model-value="onSelectDate"
      />
      <div class="flex items-center gap-2 border-t border-border p-2">
        <Select :model-value="parts.hour" @update:model-value="onHour">
          <SelectTrigger class="w-[4.5rem]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="hour in HOUR_OPTIONS" :key="hour" :value="hour">
              {{ hour }}
            </SelectItem>
          </SelectContent>
        </Select>
        <span class="text-muted-foreground">:</span>
        <Select :model-value="parts.minute" @update:model-value="onMinute">
          <SelectTrigger class="w-[4.5rem]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="minute in MINUTE_OPTIONS" :key="minute" :value="minute">
              {{ minute }}
            </SelectItem>
          </SelectContent>
        </Select>
      </div>
    </PopoverContent>
  </Popover>
</template>
