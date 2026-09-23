<script setup lang="ts">
import { CalendarClock, Inbox, RefreshCw, Settings } from "@lucide/vue";
import { useQueryClient } from "@tanstack/vue-query";
import { computed, onMounted, onUnmounted, provide, ref, watch } from "vue";
import { toast } from "vue-sonner";

import AppTitleBar from "@/components/AppTitleBar.vue";
import DragTitlePreview from "@/components/DragTitlePreview.vue";
import MonthCalendar from "@/components/MonthCalendar.vue";
import OverviewPane from "@/components/OverviewPane.vue";
import SettingsDialog from "@/components/SettingsDialog.vue";
import TaskPane from "@/components/TaskPane.vue";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { DRAG_DROP_KEY, useDragDrop } from "@/composables/useDragDrop";
import { invokeErrorMessage } from "@/lib/invokeError";
import { onChromeDblClick, onChromeDragPointerDown } from "@/lib/windowDrag";
import { useCalendarViewStore } from "@/stores/calendarView";
import { useLayoutStore } from "@/stores/layout";
import { useSessionStore } from "@/stores/session";
import { useSettingsStore } from "@/stores/settings";

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

watch(
  () => session.authToast,
  (message) => {
    if (!message) {
      return;
    }
    toast.error(message, {
      action: {
        label: "打开设置",
        onClick: () => {
          settings.openDialog();
        },
      },
    });
    session.clearAuthToast();
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

const statusDotClass = computed(() => {
  if (session.connectionStatus === "online") {
    return "bg-emerald-500";
  }
  if (session.connectionStatus === "auth_error") {
    return "bg-destructive";
  }
  return "bg-muted-foreground";
});

async function onManualSync() {
  try {
    const result = await session.syncFromUi();
    if (result.kind === "busy") {
      return;
    }
    if (result.kind === "logged_out") {
      toast.error("请先登录");
      return;
    }
    if (result.kind === "local") {
      toast.message("当前是本地日历，不会同步到服务器");
      return;
    }
    if (result.kind === "auth_error") {
      toast.error("密码无效，请到设置重新输入", {
        action: {
          label: "打开设置",
          onClick: () => {
            settings.openDialog();
          },
        },
      });
      return;
    }
    await queryClient.invalidateQueries({ queryKey: ["events"] });
    await queryClient.invalidateQueries({ queryKey: ["tasks"] });
    await queryClient.invalidateQueries({ queryKey: ["dayColors"] });
    await queryClient.invalidateQueries({ queryKey: ["calendars"] });
    if (result.pushed > 0) {
      toast.success(`已推送 ${result.pushed} 项到服务器`);
    } else if (result.pulled > 0) {
      toast.success(`已从服务器更新 ${result.pulled} 项`);
    } else {
      toast.message("当前日历没有未上推的改动");
    }
  } catch (e) {
    toast.error(invokeErrorMessage(e));
  }
}
</script>

<template>
  <TooltipProvider>
    <div class="flex h-screen w-screen overflow-hidden bg-background text-foreground">
      <aside
        :style="{ width: `${layout.railWidth}px` }"
        class="relative flex shrink-0 flex-col border-r border-border bg-background"
        @pointerdown="onChromeDragPointerDown"
        @dblclick="onChromeDblClick"
      >
        <button
          type="button"
          :class="railButtonClass('tasks')"
          @pointerdown.stop
          @click="layout.selectSidePage('tasks')"
        >
          <Inbox class="size-5" />
        </button>
        <button
          type="button"
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
            @pointerdown.stop
            @click="settings.openDialog()"
          >
            <span class="size-2.5 rounded-full" :class="statusDotClass" />
          </button>
          <button
            v-if="session.loggedIn"
            type="button"
            class="text-muted-foreground hover:bg-muted/60 relative flex h-10 w-full items-center justify-center rounded-md disabled:opacity-50"
            :disabled="session.syncing"
            @pointerdown.stop
            @click="onManualSync"
          >
            <RefreshCw class="size-5" :class="{ 'animate-spin': session.syncing }" />
            <span
              v-if="session.pendingRemoteChanges"
              class="bg-primary absolute top-1.5 right-1.5 size-1.5 rounded-full"
            />
          </button>
          <button
            type="button"
            class="text-muted-foreground hover:bg-muted/60 flex h-10 w-full items-center justify-center rounded-md"
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
        class="bg-muted/10 flex h-full min-h-0 shrink-0 flex-col border-r border-border"
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
        class="w-1 shrink-0 cursor-col-resize select-none bg-border/60 transition-colors hover:bg-primary"
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
      <Toaster position="bottom-center" />
    </div>
  </TooltipProvider>
</template>
