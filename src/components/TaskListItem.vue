<script setup lang="ts">
import { MoreHorizontal } from "@lucide/vue";
import { nextTick, ref, watch } from "vue";

import type { TaskRow } from "@/api/tasks";

const props = defineProps<{
  task: TaskRow;
  inlineEditing?: boolean;
}>();

const emit = defineEmits<{
  openEdit: [TaskRow];
  inlineEdit: [TaskRow];
  inlineSave: [TaskRow, string];
  inlineCancel: [];
  dragStart: [TaskRow, PointerEvent];
}>();

const editValue = ref(props.task.summary);
const inputRef = ref<HTMLInputElement | null>(null);

watch(
  () => props.inlineEditing,
  async (editing) => {
    if (!editing) {
      return;
    }
    editValue.value = props.task.summary;
    await nextTick();
    inputRef.value?.focus();
    inputRef.value?.select();
  },
);

function commitInline() {
  const next = editValue.value.trim();
  if (!next) {
    editValue.value = props.task.summary;
    emit("inlineCancel");
    return;
  }
  emit("inlineSave", props.task, next);
}

function onPointerDown(event: PointerEvent) {
  if (props.inlineEditing) {
    return;
  }
  emit("dragStart", props.task, event);
}

function onContextMenu(event: MouseEvent) {
  event.preventDefault();
  emit("openEdit", props.task);
}

function onDoubleClick(event: MouseEvent) {
  event.preventDefault();
  event.stopPropagation();
  emit("inlineEdit", props.task);
}
</script>

<template>
  <div
    v-if="inlineEditing"
    class="border-b border-border/60 px-3 py-2"
    @keydown.esc.prevent="emit('inlineCancel')"
  >
    <input
      ref="inputRef"
      v-model="editValue"
      class="w-full rounded-md border border-input bg-background px-2 py-1 text-sm"
      placeholder="请输入标题"
      @keydown.enter.prevent="commitInline"
      @blur="commitInline"
    />
  </div>
  <div
    v-else
    class="group flex items-center gap-2 border-b border-border/60 px-3 py-2 hover:bg-muted/40"
    @contextmenu="onContextMenu"
    @dblclick="onDoubleClick"
    @pointerdown="onPointerDown"
  >
    <span class="min-w-0 flex-1 truncate text-sm select-none">{{ task.summary }}</span>
    <button
      type="button"
      class="shrink-0 rounded p-1 opacity-0 transition-opacity group-hover:opacity-100 hover:bg-muted"
      @pointerdown.stop
      @click.stop="emit('openEdit', task)"
    >
      <MoreHorizontal class="size-4 text-muted-foreground" />
    </button>
  </div>
</template>
