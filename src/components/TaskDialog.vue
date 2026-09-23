<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";

import type { TaskRow, UpdateTaskInput } from "@/api/tasks";
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
import { Textarea } from "@/components/ui/textarea";
import { preventOverlayDismiss } from "@/lib/dialogDismiss";

const props = defineProps<{
  open: boolean;
  task: TaskRow | null;
}>();

const emit = defineEmits<{
  close: [];
  save: [UpdateTaskInput];
  delete: [];
}>();

const form = reactive({
  summary: "",
  description: "",
  isStamp: false,
});

const summaryError = ref("");
const deleteOpen = ref(false);

const title = computed(() => (props.task?.isStamp ? "编辑图章" : "编辑任务"));
const kindLabel = computed(() => (props.task?.isStamp ? "图章" : "任务"));

watch(
  () => [props.open, props.task] as const,
  () => {
    if (!props.open || !props.task) {
      return;
    }
    form.summary = props.task.summary;
    form.description = props.task.description ?? "";
    form.isStamp = props.task.isStamp;
    summaryError.value = "";
    deleteOpen.value = false;
  },
  { immediate: true },
);

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
    summary: form.summary.trim(),
    description: form.description.trim() || null,
    isStamp: form.isStamp,
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
          <Label for="task-summary">标题</Label>
          <Input
            id="task-summary"
            v-model="form.summary"
            placeholder="请输入标题"
            :aria-invalid="summaryError ? true : undefined"
            @update:model-value="summaryError = ''"
          />
          <p v-if="summaryError" class="text-destructive text-xs">{{ summaryError }}</p>
        </div>
        <label class="flex items-center gap-2 text-sm">
          <Checkbox v-model="form.isStamp" />
          恒定图章
        </label>
        <div class="space-y-1.5">
          <Label for="task-note">备注</Label>
          <Textarea id="task-note" v-model="form.description" />
        </div>
      </div>
      <DialogFooter class="sm:justify-between">
        <Button type="button" variant="destructive" @click="deleteOpen = true">删除</Button>
        <div class="ml-auto flex gap-2">
          <Button type="button" variant="outline" @click="emit('close')">取消</Button>
          <Button type="button" @click="submit">确定</Button>
        </div>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <AlertDialog v-model:open="deleteOpen">
    <AlertDialogContent @pointer-down-outside="preventOverlayDismiss" @interact-outside="preventOverlayDismiss">
      <AlertDialogHeader>
        <AlertDialogTitle>确定删除这个{{ kindLabel }}吗？</AlertDialogTitle>
        <AlertDialogDescription>删除后无法恢复。</AlertDialogDescription>
      </AlertDialogHeader>
      <AlertDialogFooter>
        <AlertDialogCancel>返回</AlertDialogCancel>
        <AlertDialogAction @click="confirmDelete">确定删除</AlertDialogAction>
      </AlertDialogFooter>
    </AlertDialogContent>
  </AlertDialog>
</template>
