<script setup lang="ts">
import WindowControls from "@/components/WindowControls.vue";
import { onChromeDblClick, onChromeDragPointerDown } from "@/lib/windowDrag";
</script>

<template>
  <div
    class="relative flex h-12 shrink-0 items-center border-b border-border bg-background px-4"
  >
    <div
      id="app-title-leading"
      class="flex h-full min-w-0 flex-1 items-center"
      @pointerdown="onChromeDragPointerDown"
      @dblclick="onChromeDblClick"
    />
    <div
      class="pointer-events-auto relative z-[200] ml-3 flex items-center"
      data-no-drag
      data-fc-window-controls
      @pointerdown.stop
    >
      <WindowControls />
    </div>
  </div>
</template>
