<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";

import type { EventRow, SaveEventInput } from "@/api/events";
import AppDatePicker from "@/components/AppDatePicker.vue";
import AppDateTimePicker from "@/components/AppDateTimePicker.vue";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { preventOverlayDismiss } from "@/lib/dialogDismiss";
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

const summaryError = ref("");
const deleteOpen = ref(false);

const title = computed(() => (props.mode === "create" ? "新建事件" : "编辑事件"));

function fallbackDate() {
  return props.initialDate ?? Temporal.Now.plainDateISO().toString();
}

function resetForm() {
  summaryError.value = "";
  deleteOpen.value = false;
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

function onOpenChange(next: boolean) {
  if (!next) {
    emit("close");
  }
}

function submit() {
  if (!form.summary.trim()) {
    summaryError.value = "标题不能为空";
    return;
  }
  summaryError.value = "";
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
  deleteOpen.value = false;
  emit("delete");
}
</script>

<template>
  <Dialog :open="open" @update:open="onOpenChange">
    <DialogContent
      class="max-w-lg sm:max-w-lg"
      :show-close-button="false"
      @pointer-down-outside="preventOverlayDismiss"
      @interact-outside="preventOverlayDismiss"
    >
      <DialogHeader>
        <DialogTitle>{{ title }}</DialogTitle>
      </DialogHeader>
      <div class="space-y-4">
        <div class="space-y-1.5">
          <Label for="event-summary">标题</Label>
          <Input
            id="event-summary"
            v-model="form.summary"
            placeholder="请输入标题"
            :aria-invalid="summaryError ? true : undefined"
            @update:model-value="summaryError = ''"
          />
          <p v-if="summaryError" class="text-destructive text-xs">{{ summaryError }}</p>
        </div>
        <label class="flex items-center gap-2 text-sm">
          <Checkbox
            :model-value="form.allDay"
            @update:model-value="onAllDayChange($event === true)"
          />
          全天
        </label>
        <div class="grid grid-cols-2 gap-3">
          <div class="space-y-1.5">
            <Label>开始</Label>
            <AppDatePicker v-if="form.allDay" v-model="form.dtstart" />
            <AppDateTimePicker v-else v-model="form.dtstart" />
          </div>
          <div class="space-y-1.5">
            <Label>结束</Label>
            <AppDatePicker v-if="form.allDay" v-model="form.dtend" />
            <AppDateTimePicker v-else v-model="form.dtend" />
          </div>
        </div>
        <div class="space-y-1.5">
          <Label>重复</Label>
          <Select v-model="form.recurrence">
            <SelectTrigger class="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem v-for="option in recurrenceOptions" :key="option.value" :value="option.value">
                {{ option.label }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div class="space-y-1.5">
          <Label for="event-note">备注</Label>
          <Textarea id="event-note" v-model="form.description" />
        </div>
      </div>
      <DialogFooter class="sm:justify-between">
        <Button v-if="mode === 'edit'" type="button" variant="destructive" @click="deleteOpen = true">
          删除
        </Button>
        <div class="ml-auto flex gap-2">
          <Button type="button" variant="outline" @click="emit('close')">取消</Button>
          <Button type="button" @click="submit">保存</Button>
        </div>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <AlertDialog v-model:open="deleteOpen">
    <AlertDialogContent @pointer-down-outside="preventOverlayDismiss" @interact-outside="preventOverlayDismiss">
      <AlertDialogHeader>
        <AlertDialogTitle>确定删除这个事件吗？</AlertDialogTitle>
        <AlertDialogDescription>删除后无法恢复，重复事件会整组删除。</AlertDialogDescription>
      </AlertDialogHeader>
      <AlertDialogFooter>
        <AlertDialogCancel>返回</AlertDialogCancel>
        <AlertDialogAction @click="confirmDelete">确定删除</AlertDialogAction>
      </AlertDialogFooter>
    </AlertDialogContent>
  </AlertDialog>
</template>
