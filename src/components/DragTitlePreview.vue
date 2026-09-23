<script setup lang="ts">
import { storeToRefs } from "pinia";

import { useTaskDragStore } from "@/stores/taskDrag";

const dragStore = useTaskDragStore();
const { active, title, pointerX, pointerY } = storeToRefs(dragStore);
</script>

<template>
  <div
    v-if="active && title"
    class="fc-month-event pointer-events-none fixed z-[100] max-w-[240px] truncate rounded py-0.5 pr-2 pl-2.5 text-[11px] leading-tight shadow-md"
    :style="{ left: `${pointerX + 12}px`, top: `${pointerY + 12}px` }"
  >
    {{ title }}
  </div>
</template>
