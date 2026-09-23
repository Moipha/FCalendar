<script setup lang="ts">
import type { DateValue } from "reka-ui";
import { computed, ref } from "vue";

import { Calendar } from "@/components/ui/calendar";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import {
  formatPlainDateLabel,
  fromCalendarDate,
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

const calendarValue = computed(() => {
  if (!props.modelValue || props.modelValue.length < 10) {
    return undefined;
  }
  return toCalendarDate(props.modelValue);
});

const label = computed(() =>
  props.modelValue ? formatPlainDateLabel(props.modelValue) : "选择日期",
);

function onSelect(value: DateValue | DateValue[] | undefined) {
  if (!value || Array.isArray(value)) {
    return;
  }
  emit("update:modelValue", fromCalendarDate(value));
  open.value = false;
}
</script>

<template>
  <Popover v-model:open="open">
    <PopoverTrigger as-child>
      <slot>
        <Button type="button" variant="outline" class="w-full justify-start font-normal" :disabled="disabled">
          {{ label }}
        </Button>
      </slot>
    </PopoverTrigger>
    <PopoverContent class="w-auto p-0" align="start">
      <Calendar
        :model-value="calendarValue"
        locale="zh-CN"
        :week-starts-on="weekStartsOn"
        @update:model-value="onSelect"
      />
    </PopoverContent>
  </Popover>
</template>
