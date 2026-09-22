<script setup lang="ts">
import { Eraser, Plus } from "@lucide/vue";
import { nextTick, onMounted, ref, watch } from "vue";

import {
  DAY_CELL_TINT_LABEL,
  DAY_CELL_TINT_ORDER,
  DAY_CELL_TINT_SWATCH,
  type DayCellTint,
} from "@/lib/dayCellColors";

const props = defineProps<{
  x: number;
  y: number;
  currentTint: DayCellTint;
  showColorPicker: boolean;
  mode: "single" | "batch";
  showClearColor: boolean;
}>();

const emit = defineEmits<{
  create: [];
  clearColors: [];
  expandColors: [];
  collapseColors: [];
  pickColor: [DayCellTint];
}>();

const menuRef = ref<HTMLElement | null>(null);
const pos = ref({ left: props.x, top: props.y });

async function adjustPosition() {
  await nextTick();
  const el = menuRef.value;
  if (!el) {
    return;
  }
  const pad = 8;
  const rect = el.getBoundingClientRect();
  let left = props.x;
  let top = props.y;
  if (left + rect.width > window.innerWidth - pad) {
    left = window.innerWidth - rect.width - pad;
  }
  if (top + rect.height > window.innerHeight - pad) {
    top = window.innerHeight - rect.height - pad;
  }
  pos.value = {
    left: Math.max(pad, left),
    top: Math.max(pad, top),
  };
}

watch(
  () => [props.x, props.y, props.showColorPicker, props.mode, props.showClearColor] as const,
  () => {
    void adjustPosition();
  },
);

onMounted(() => {
  void adjustPosition();
});
</script>

<template>
  <div
    ref="menuRef"
    class="fc-day-context-menu fixed z-[100] min-w-[11rem] rounded-md border border-border bg-popover p-1 text-popover-foreground shadow-md"
    :style="{ left: `${pos.left}px`, top: `${pos.top}px` }"
    role="menu"
    @contextmenu.prevent
  >
    <button
      v-if="showClearColor"
      type="button"
      class="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-left text-sm hover:bg-muted"
      role="menuitem"
      @click="emit('clearColors')"
    >
      <Eraser class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
      <span>清除日格颜色</span>
    </button>

    <button
      v-if="mode === 'single'"
      type="button"
      class="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-left text-sm hover:bg-muted"
      role="menuitem"
      @click="emit('create')"
    >
      <Plus class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
      <span>创建新的事件</span>
    </button>

    <div v-if="!showColorPicker">
      <button
        type="button"
        class="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-left text-sm hover:bg-muted"
        role="menuitem"
        @click="emit('expandColors')"
      >
        <span
          class="size-3.5 shrink-0 rounded-[3px] border border-border"
          :style="{ background: DAY_CELL_TINT_SWATCH[currentTint] }"
          aria-hidden="true"
        />
        <span>设置日格颜色</span>
      </button>
    </div>

    <div
      v-else
      class="flex min-h-9 cursor-default items-center gap-2 px-2 py-1.5"
      role="presentation"
      @click="emit('collapseColors')"
    >
      <button
        v-for="tint in DAY_CELL_TINT_ORDER"
        :key="tint"
        type="button"
        class="size-7 shrink-0 rounded-[4px] border-2 transition-shadow hover:opacity-90"
        :class="
          tint === currentTint
            ? 'border-primary ring-2 ring-primary/30'
            : tint === 'default'
              ? 'border-border'
              : 'border-transparent'
        "
        :style="{ background: DAY_CELL_TINT_SWATCH[tint] }"
        :title="DAY_CELL_TINT_LABEL[tint]"
        @click.stop="emit('pickColor', tint)"
      />
    </div>
  </div>
</template>
