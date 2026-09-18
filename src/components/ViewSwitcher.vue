<script setup lang="ts">
import { calendarViewOptions, useCalendarViewStore, type CalendarViewId } from "@/stores/calendarView";

const calendarView = useCalendarViewStore();

function onChange(event: Event) {
  const value = (event.target as HTMLSelectElement).value as CalendarViewId;
  calendarView.setView(value);
}
</script>

<template>
  <select
    class="h-8 rounded-md border border-border bg-background px-2 text-sm"
    :value="calendarView.current"
    @change="onChange"
  >
    <option v-for="option in calendarViewOptions" :key="option.id" :value="option.id">
      {{ option.label }}
    </option>
  </select>
</template>
