<script setup lang="ts">
import { CalendarClock, Inbox, Settings } from "@lucide/vue";
import { useQueryClient } from "@tanstack/vue-query";
import { onMounted, onUnmounted, provide, ref, watch } from "vue";

import AppTitleBar from "@/components/AppTitleBar.vue";
import DragTitlePreview from "@/components/DragTitlePreview.vue";
import MonthCalendar from "@/components/MonthCalendar.vue";
import OverviewPane from "@/components/OverviewPane.vue";
import SettingsDialog from "@/components/SettingsDialog.vue";
import TaskPane from "@/components/TaskPane.vue";
import { DRAG_DROP_KEY, useDragDrop } from "@/composables/useDragDrop";
import { useCalendarViewStore } from "@/stores/calendarView";
import { useLayoutStore } from "@/stores/layout";
import { useSessionStore } from "@/stores/session";
import { useSettingsStore } from "@/stores/settings";
import { computed } from "vue";

const layout = useLayoutStore();
const settings = useSettingsStore();
const session = useSessionStore();
const calendarView = useCalendarViewStore();
const queryClient = useQueryClient();
const dragging = ref(false);

const calendarId = computed(() => session.currentCalendarId);

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

watch(
  () => session.currentCalendarId,
  () => {
    void queryClient.invalidateQueries({ queryKey: ["events"] });
    void queryClient.invalidateQueries({ queryKey: ["tasks"] });
    void queryClient.invalidateQueries({ queryKey: ["dayColors"] });
    void queryClient.invalidateQueries({ queryKey: ["calendars"] });
  },
);

onMounted(() => {
  settings.hydrate();
  void session.bootstrap();
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
      <div class="mx-3 mt-auto mb-3 flex flex-col items-center gap-2">
        <button
          v-if="session.connectionStatus !== 'logged_out'"
          type="button"
          class="flex size-5 items-center justify-center"
          :title="
            session.connectionStatus === 'online'
              ? '在线'
              : session.connectionStatus === 'auth_error'
                ? '密码无效，点击打开设置'
                : '离线'
          "
          @pointerdown.stop
          @click="settings.openDialog()"
        >
          <span
            class="size-2.5 rounded-full"
            :class="
              session.connectionStatus === 'online'
                ? 'bg-emerald-500'
                : 'bg-red-500'
            "
          />
        </button>
        <button
          type="button"
          title="设置"
          class="flex h-10 w-full items-center justify-center rounded-md text-muted-foreground hover:bg-muted/60"
          @pointerdown.stop
          @click="settings.openDialog()"
        >
          <Settings class="size-5" />
        </button>
      </div>
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
    <div
      v-if="session.authToast"
      class="fixed bottom-6 left-1/2 z-[80] flex -translate-x-1/2 items-center gap-3 rounded-lg bg-foreground px-4 py-2 text-sm text-background shadow-lg"
    >
      <span>{{ session.authToast }}</span>
      <button type="button" class="underline" @click="settings.openDialog(); session.clearAuthToast()">
        打开设置
      </button>
      <button type="button" @click="session.clearAuthToast()">关闭</button>
    </div>
  </div>
</template>
