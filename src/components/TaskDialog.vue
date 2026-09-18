<script setup lang="ts">
import { computed, reactive, watch } from "vue";

import type { TaskRow, UpdateTaskInput } from "@/api/tasks";

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

const title = computed(() => (props.task?.isStamp ? "编辑图章" : "编辑任务"));

watch(
  () => [props.open, props.task] as const,
  () => {
    if (!props.open || !props.task) {
      return;
    }
    form.summary = props.task.summary;
    form.description = props.task.description ?? "";
    form.isStamp = props.task.isStamp;
  },
  { immediate: true },
);

function submit() {
  if (!form.summary.trim()) {
    window.alert("标题不能为空");
    return;
  }
  emit("save", {
    summary: form.summary.trim(),
    description: form.description.trim() || null,
    isStamp: form.isStamp,
  });
}

function confirmDelete() {
  if (window.confirm(`确定删除这个${props.task?.isStamp ? "图章" : "任务"}吗？`)) {
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
          <input v-model="form.isStamp" type="checkbox" />
          恒定图章
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
          class="rounded-md border border-destructive px-4 py-2 text-destructive"
          @click="confirmDelete"
        >
          删除
        </button>
        <div class="ml-auto flex gap-2">
          <button class="rounded-md border border-border px-4 py-2" @click="emit('close')">取消</button>
          <button class="rounded-md bg-primary px-4 py-2 text-primary-foreground" @click="submit">确定</button>
        </div>
      </div>
    </div>
  </div>
</template>
