<script setup lang="ts">
import { Copy, Minus, Square, X } from "@lucide/vue";

import { Button } from "@/components/ui/button";
import { useWindowControls } from "@/composables/useWindowControls";

const { isMaximized, minimizeWindow, toggleMaximize, closeWindow } = useWindowControls();
</script>

<template>
  <div class="flex items-center">
    <Button variant="ghost" size="icon" class="rounded-md" title="最小化" @click="minimizeWindow">
      <Minus />
    </Button>
    <Button
      variant="ghost"
      size="icon"
      class="rounded-md"
      :title="isMaximized ? '还原' : '最大化'"
      @click="toggleMaximize"
    >
      <Copy v-if="isMaximized" />
      <Square v-else />
    </Button>
    <Button
      variant="ghost"
      size="icon"
      class="rounded-md hover:bg-destructive/15 hover:text-destructive"
      title="关闭"
      @click="closeWindow"
    >
      <X />
    </Button>
  </div>
</template>
