<script setup lang="ts">
import { computed, reactive, watch } from "vue";

import type { EventRow, SaveEventInput } from "@/api/events";
import {
  datePart,
  defaultTimedRange,
  fromDatetimeLocalValue,
  toDatetimeLocalValue,
} from "@/lib/datetime";
import {
  presetToRrule,
  recurrenceOptions,
  rruleToPreset,
  type RecurrencePreset,
} from "@/lib/rrule";

const props = defineProps<{
  open: boolean;
  mode: "create" | "edit";
  calendarId: string;
  initialDate?: string;
  event?: EventRow | null;
}>();

const emit = defineEmits<{
  close: [];
  save: [SaveEventInput];
  delete: [];
}>();

const form = reactive({
  summary: "",
  description: "",
  allDay: false,
  dtstart: "",
  dtend: "",
  recurrence: "never" as RecurrencePreset,
});

const title = computed(() => (props.mode === "create" ? "新建事件" : "编辑事件"));

function fallbackDate() {
  return props.initialDate ?? Temporal.Now.plainDateISO().toString();
}

function resetForm() {
  if (props.mode === "edit" && props.event) {
    form.summary = props.event.summary;
    form.description = props.event.description ?? "";
    form.allDay = props.event.allDay;
    form.dtstart = props.event.allDay
      ? datePart(props.event.dtstart)
      : toDatetimeLocalValue(props.event.dtstart);
    form.dtend = props.event.allDay
      ? datePart(props.event.dtend ?? props.event.dtstart)
      : toDatetimeLocalValue(props.event.dtend ?? props.event.dtstart);
    form.recurrence = rruleToPreset(props.event.rrule);
    return;
  }
  const date = fallbackDate();
  const { dtstart, dtend } = defaultTimedRange(date);
  form.summary = "";
  form.description = "";
  form.allDay = false;
  form.dtstart = toDatetimeLocalValue(dtstart);
  form.dtend = toDatetimeLocalValue(dtend);
  form.recurrence = "never";
}

watch(
  () => ({
    open: props.open,
    mode: props.mode,
    eventId: props.event?.id ?? "",
    initialDate: props.initialDate ?? "",
  }),
  (state) => {
    if (!state.open) {
      return;
    }
    resetForm();
  },
  { immediate: true },
);

function onAllDayChange(checked: boolean) {
  const date = datePart(form.dtstart) || fallbackDate();
  form.allDay = checked;
  if (checked) {
    form.dtstart = date;
    form.dtend = date;
    return;
  }
  const range = defaultTimedRange(date);
  form.dtstart = toDatetimeLocalValue(range.dtstart);
  form.dtend = toDatetimeLocalValue(range.dtend);
}

function submit() {
  if (!form.summary.trim()) {
    window.alert("标题不能为空");
    return;
  }
  emit("save", {
    calendarId: props.calendarId,
    summary: form.summary.trim(),
    description: form.description.trim() || null,
    allDay: form.allDay,
    dtstart: form.allDay ? datePart(form.dtstart) : fromDatetimeLocalValue(form.dtstart),
    dtend: form.allDay ? datePart(form.dtend) : fromDatetimeLocalValue(form.dtend),
    rrule: presetToRrule(form.recurrence),
  });
}

function confirmDelete() {
  if (window.confirm("确定删除这个事件吗？")) {
    emit("delete");
  }
}
</script>

<template>
  <div v-if="open" class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4">
    <div class="w-full max-w-lg rounded-xl border border-border bg-background p-6 shadow-xl">
      <h2 class="mb-4 text-lg font-semibold">{{ title }}</h2>
      <div class="space-y-4">
        <label class="block space-y-1">
          <span class="text-sm text-muted-foreground">标题</span>
          <input
            v-model="form.summary"
            class="w-full rounded-md border border-input bg-background px-3 py-2"
            placeholder="请输入标题"
          />
        </label>
        <label class="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            :checked="form.allDay"
            @change="onAllDayChange(($event.target as HTMLInputElement).checked)"
          />
          全天
        </label>
        <div class="grid grid-cols-2 gap-3">
          <label class="block space-y-1">
            <span class="text-sm text-muted-foreground">开始</span>
            <input
              v-if="form.allDay"
              :key="`all-day-start`"
              v-model="form.dtstart"
              type="date"
              class="w-full rounded-md border border-input bg-background px-3 py-2"
            />
            <input
              v-else
              :key="`timed-start`"
              v-model="form.dtstart"
              type="datetime-local"
              class="w-full rounded-md border border-input bg-background px-3 py-2"
            />
          </label>
          <label class="block space-y-1">
            <span class="text-sm text-muted-foreground">结束</span>
            <input
              v-if="form.allDay"
              :key="`all-day-end`"
              v-model="form.dtend"
              type="date"
              class="w-full rounded-md border border-input bg-background px-3 py-2"
            />
            <input
              v-else
              :key="`timed-end`"
              v-model="form.dtend"
              type="datetime-local"
              class="w-full rounded-md border border-input bg-background px-3 py-2"
            />
          </label>
        </div>
        <label class="block space-y-1">
          <span class="text-sm text-muted-foreground">重复</span>
          <select v-model="form.recurrence" class="w-full rounded-md border border-input bg-background px-3 py-2">
            <option v-for="option in recurrenceOptions" :key="option.value" :value="option.value">
              {{ option.label }}
            </option>
          </select>
        </label>
        <label class="block space-y-1">
          <span class="text-sm text-muted-foreground">备注</span>
          <textarea
            v-model="form.description"
            rows="3"
            class="w-full rounded-md border border-input bg-background px-3 py-2"
          />
        </label>
      </div>
      <div class="mt-6 flex justify-between gap-2">
        <button
          v-if="mode === 'edit'"
          class="rounded-md border border-destructive px-4 py-2 text-destructive"
          @click="confirmDelete"
        >
          删除
        </button>
        <div class="ml-auto flex gap-2">
          <button class="rounded-md border border-border px-4 py-2" @click="emit('close')">取消</button>
          <button class="rounded-md bg-primary px-4 py-2 text-primary-foreground" @click="submit">保存</button>
        </div>
      </div>
    </div>
  </div>
</template>
