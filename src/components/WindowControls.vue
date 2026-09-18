<script setup lang="ts">
import { useWindowControls } from "@/composables/useWindowControls";

const { isMaximized, minimizeWindow, toggleMaximize, closeWindow } = useWindowControls();
</script>

<template>
  <div class="flex items-center gap-1">
    <button class="h-8 w-10 rounded-md border border-border text-sm" title="最小化" @click="minimizeWindow">
      —
    </button>
    <button
      class="h-8 w-10 rounded-md border border-border text-sm"
      :title="isMaximized ? '还原' : '最大化'"
      @click="toggleMaximize"
    >
      {{ isMaximized ? "❐" : "□" }}
    </button>
    <button class="h-8 w-10 rounded-md border border-border text-sm" title="关闭" @click="closeWindow">
      ×
    </button>
  </div>
</template>
