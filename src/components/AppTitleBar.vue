<script setup lang="ts">
import ViewSwitcher from "@/components/ViewSwitcher.vue";
import WindowControls from "@/components/WindowControls.vue";
</script>

<template>
  <div class="flex h-12 shrink-0 items-center border-b border-border px-4">
    <div
      id="app-title-leading"
      class="flex h-full min-w-0 flex-1 items-center"
      data-tauri-drag-region
    />
    <div class="ml-3 flex items-center gap-2" @pointerdown.stop>
      <ViewSwitcher />
      <div class="relative z-[200]">
        <WindowControls />
      </div>
    </div>
  </div>
</template>
