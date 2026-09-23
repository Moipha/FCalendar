<script setup lang="ts">
import { onChromeDblClick, onChromeDragPointerDown } from "@/lib/windowDrag";
</script>

<template>
  <!-- 只应作为遮罩的子节点：弹窗关闭后随遮罩卸载，避免挡住切月 / 边栏图标 -->
  <div
    class="pointer-events-auto fixed top-0 left-0 h-12"
    style="right: var(--fc-window-controls-width)"
    data-fc-overlay-drag
    @pointerdown.stop="onChromeDragPointerDown"
    @dblclick.stop.prevent="onChromeDblClick"
  />
  <div
    class="pointer-events-auto fixed bottom-0 left-0 w-20"
    style="top: var(--fc-titlebar-height)"
    data-fc-overlay-drag
    @pointerdown.stop="onChromeDragPointerDown"
    @dblclick.stop.prevent="onChromeDblClick"
  />
</template>
