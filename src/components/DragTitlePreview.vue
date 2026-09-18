<script setup lang="ts">
import { storeToRefs } from "pinia";

import { useTaskDragStore } from "@/stores/taskDrag";

const dragStore = useTaskDragStore();
const { active, title, pointerX, pointerY } = storeToRefs(dragStore);
</script>

<template>
  <div
    v-if="active && title"
    class="fc-drag-title-preview pointer-events-none fixed z-[100] max-w-[240px] truncate rounded-md border border-border bg-popover px-2 py-1 text-sm shadow-md"
    :style="{ left: `${pointerX + 12}px`, top: `${pointerY + 12}px` }"
  >
    {{ title }}
  </div>
</template>
