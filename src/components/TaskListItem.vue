<script setup lang="ts">
import { GripVertical, MoreHorizontal } from "@lucide/vue";
import { nextTick, ref, watch } from "vue";

import type { TaskRow } from "@/api/tasks";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

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
const editWrapRef = ref<HTMLElement | null>(null);

watch(
  () => props.inlineEditing,
  async (editing) => {
    if (!editing) {
      return;
    }
    editValue.value = props.task.summary;
    await nextTick();
    const input = editWrapRef.value?.querySelector("input");
    input?.focus();
    input?.select();
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
    ref="editWrapRef"
    class="border-b border-border/60 px-3 py-2"
    @keydown.esc.prevent="emit('inlineCancel')"
  >
    <Input
      v-model="editValue"
      placeholder="请输入标题"
      @keydown.enter.prevent="commitInline"
      @blur="commitInline"
    />
  </div>
  <div
    v-else
    class="group hover:bg-muted/40 flex items-center gap-1 border-b border-border/60 px-2 py-2"
    @contextmenu="onContextMenu"
    @dblclick="onDoubleClick"
    @pointerdown="onPointerDown"
  >
    <GripVertical class="text-muted-foreground size-3.5 shrink-0 opacity-40" aria-hidden="true" />
    <span class="min-w-0 flex-1 truncate text-sm select-none">{{ task.summary }}</span>
    <Button
      type="button"
      variant="ghost"
      size="icon-xs"
      class="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
      @pointerdown.stop
      @click.stop="emit('openEdit', task)"
    >
      <MoreHorizontal class="text-muted-foreground" />
    </Button>
  </div>
</template>
