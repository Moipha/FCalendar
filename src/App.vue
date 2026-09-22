<script setup lang="ts">
import { CalendarClock, Inbox, Settings } from "@lucide/vue";
import { onMounted, onUnmounted, provide, ref } from "vue";

import AppTitleBar from "@/components/AppTitleBar.vue";
import DragTitlePreview from "@/components/DragTitlePreview.vue";
import MonthCalendar from "@/components/MonthCalendar.vue";
import OverviewPane from "@/components/OverviewPane.vue";
import SettingsDialog from "@/components/SettingsDialog.vue";
import TaskPane from "@/components/TaskPane.vue";
import { listCalendars } from "@/api/calendars";
import { DRAG_DROP_KEY, useDragDrop } from "@/composables/useDragDrop";
import { useCalendarViewStore } from "@/stores/calendarView";
import { useLayoutStore } from "@/stores/layout";
import { useSettingsStore } from "@/stores/settings";
import { useQuery } from "@tanstack/vue-query";
import { computed } from "vue";

const layout = useLayoutStore();
const settings = useSettingsStore();
const calendarView = useCalendarViewStore();
const dragging = ref(false);

const { data: calendars } = useQuery({
  queryKey: ["calendars"],
  queryFn: listCalendars,
});

const calendarId = computed(() => calendars.value?.[0]?.id ?? "");

const dragDrop = useDragDrop(() => calendarId.value);
provide(DRAG_DROP_KEY, dragDrop);

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
  settings.hydrate();
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

function railButtonClass(page: "tasks" | "overview") {
  const active = !layout.taskPaneHidden && layout.sidePanePage === page;
  return [
    "mx-3 mt-3 flex h-10 items-center justify-center rounded-md",
    active ? "bg-primary/15 text-primary" : "text-muted-foreground hover:bg-muted/60",
  ];
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
        type="button"
        title="任务区"
        :class="railButtonClass('tasks')"
        @pointerdown.stop
        @click="layout.selectSidePage('tasks')"
      >
        <Inbox class="size-5" />
      </button>
      <button
        type="button"
        title="概览"
        :class="railButtonClass('overview')"
        @pointerdown.stop
        @click="layout.selectSidePage('overview')"
      >
        <CalendarClock class="size-5" />
      </button>
      <button
        type="button"
        title="设置"
        class="mx-3 mt-auto mb-3 flex h-10 items-center justify-center rounded-md text-muted-foreground hover:bg-muted/60"
        @pointerdown.stop
        @click="settings.openDialog()"
      >
        <Settings class="size-5" />
      </button>
    </aside>

    <section
      v-if="!layout.taskPaneHidden"
      :style="{ width: `${layout.taskPaneWidth}px` }"
      class="flex h-full min-h-0 shrink-0 flex-col border-r border-border bg-muted/10"
    >
      <TaskPane
        v-show="layout.sidePanePage === 'tasks'"
        class="min-h-0 flex-1"
        :calendar-id="calendarId"
      />
      <OverviewPane
        v-show="layout.sidePanePage === 'overview'"
        class="min-h-0 flex-1"
        :calendar-id="calendarId"
      />
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

    <DragTitlePreview />
    <SettingsDialog />
  </div>
</template>
