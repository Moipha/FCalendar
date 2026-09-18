<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";

import AppTitleBar from "@/components/AppTitleBar.vue";
import MonthCalendar from "@/components/MonthCalendar.vue";
import TaskPane from "@/components/TaskPane.vue";
import { listCalendars } from "@/api/calendars";
import { useCalendarViewStore } from "@/stores/calendarView";
import { useLayoutStore } from "@/stores/layout";
import { useQuery } from "@tanstack/vue-query";
import { computed } from "vue";

const layout = useLayoutStore();
const calendarView = useCalendarViewStore();
const dragging = ref(false);

const { data: calendars } = useQuery({
  queryKey: ["calendars"],
  queryFn: listCalendars,
});

const calendarId = computed(() => calendars.value?.[0]?.id ?? "");

function setResizingClass(active: boolean) {
  document.body.classList.toggle("is-resizing-panes", active);
}

function onPointerMove(event: PointerEvent) {
  if (!dragging.value) {
    return;
  }
  event.preventDefault();
  layout.resizeTo(event.clientX);
}

function stopDragging() {
  if (!dragging.value) {
    return;
  }
  dragging.value = false;
  setResizingClass(false);
  window.getSelection()?.removeAllRanges();
}

onMounted(() => {
  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", stopDragging);
  window.addEventListener("pointercancel", stopDragging);
});

onUnmounted(() => {
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", stopDragging);
  window.removeEventListener("pointercancel", stopDragging);
  setResizingClass(false);
});

function startDragging(event: PointerEvent) {
  event.preventDefault();
  event.stopPropagation();
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  dragging.value = true;
  setResizingClass(true);
  window.getSelection()?.removeAllRanges();
  layout.beginResize(event.clientX);
}
</script>

<template>
  <div class="flex h-screen w-screen overflow-hidden bg-background text-foreground">
    <aside
      :style="{ width: `${layout.railWidth}px` }"
      class="flex shrink-0 flex-col border-r border-border bg-muted/30"
      data-tauri-drag-region
    >
      <button
        class="m-3 rounded-md border border-border px-2 py-2 text-xs"
        @click="layout.toggleTaskPane()"
      >
        {{ layout.taskPaneHidden ? "展开任务区" : "收起任务区" }}
      </button>
    </aside>

    <section
      v-if="!layout.taskPaneHidden"
      :style="{ width: `${layout.taskPaneWidth}px` }"
      class="shrink-0 border-r border-border bg-muted/10"
    >
      <TaskPane :calendar-id="calendarId" />
    </section>

    <div
      v-if="!layout.taskPaneHidden"
      class="w-1 shrink-0 cursor-col-resize select-none bg-border/60 hover:bg-border"
      @pointerdown="startDragging"
    />

    <main class="flex min-h-0 min-w-0 flex-1 flex-col">
      <AppTitleBar />
      <div class="min-h-0 flex-1">
        <MonthCalendar v-if="calendarView.current === 'month'" />
      </div>
    </main>
  </div>
</template>
