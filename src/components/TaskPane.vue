<script setup lang="ts">
import { ChevronDown, ChevronRight, Plus } from "@lucide/vue";
import { useQuery, useQueryClient } from "@tanstack/vue-query";
import { computed, inject, nextTick, ref, watch } from "vue";

import {
  createTask,
  deleteTask,
  listTasks,
  updateTask,
  type TaskRow,
  type UpdateTaskInput,
} from "@/api/tasks";
import TaskDialog from "@/components/TaskDialog.vue";
import TaskListItem from "@/components/TaskListItem.vue";
import { DRAG_DROP_KEY } from "@/composables/useDragDrop";
import { useLayoutStore } from "@/stores/layout";
import { useTaskDragStore } from "@/stores/taskDrag";

defineProps<{
  calendarId: string;
}>();

const layout = useLayoutStore();
const taskDragStore = useTaskDragStore();
const queryClient = useQueryClient();
const dragDrop = inject(DRAG_DROP_KEY)!;
const { startTaskDrag } = dragDrop;

const tasksExpanded = ref(true);
const stampsExpanded = ref(false);
const draftActive = ref(false);
const draftValue = ref("");
const draftInputRef = ref<HTMLInputElement | null>(null);
const inlineEditingId = ref<string | null>(null);
const dialogOpen = ref(false);
const dialogTask = ref<TaskRow | null>(null);

const { data: tasks } = useQuery({
  queryKey: ["tasks"],
  queryFn: listTasks,
});

const taskItems = computed(() => tasks.value?.filter((task) => !task.isStamp) ?? []);
const stampItems = computed(() => tasks.value?.filter((task) => task.isStamp) ?? []);

watch(
  () => taskDragStore.dropTarget,
  (target) => {
    if (target === "tasks") {
      tasksExpanded.value = true;
    }
    if (target === "stamps") {
      stampsExpanded.value = true;
    }
  },
);

watch(draftActive, async (active) => {
  if (!active) {
    return;
  }
  await nextTick();
  draftInputRef.value?.focus();
});

async function invalidateTasks() {
  await queryClient.invalidateQueries({ queryKey: ["tasks"] });
}

async function startDraft() {
  if (layout.taskPaneHidden) {
    layout.openSidePage("tasks");
  }
  tasksExpanded.value = true;
  if (draftActive.value) {
    draftInputRef.value?.focus();
    return;
  }
  draftActive.value = true;
  draftValue.value = "";
}

async function commitDraft() {
  const summary = draftValue.value.trim();
  draftActive.value = false;
  draftValue.value = "";
  if (!summary) {
    return;
  }
  await createTask({ summary, description: null, isStamp: false });
  await invalidateTasks();
}

function cancelDraft() {
  draftActive.value = false;
  draftValue.value = "";
}

function openEdit(task: TaskRow) {
  dialogTask.value = task;
  dialogOpen.value = true;
}

async function saveDialog(input: UpdateTaskInput) {
  if (!dialogTask.value) {
    return;
  }
  await updateTask(dialogTask.value.id, input);
  dialogOpen.value = false;
  dialogTask.value = null;
  await invalidateTasks();
}

async function deleteFromDialog() {
  if (!dialogTask.value) {
    return;
  }
  await deleteTask(dialogTask.value.id);
  dialogOpen.value = false;
  dialogTask.value = null;
  await invalidateTasks();
}

function startInlineEdit(task: TaskRow) {
  inlineEditingId.value = task.id;
}

async function saveInline(task: TaskRow, summary: string) {
  inlineEditingId.value = null;
  if (summary === task.summary) {
    return;
  }
  await updateTask(task.id, {
    summary,
    description: task.description ?? null,
    isStamp: task.isStamp,
  });
  await invalidateTasks();
}

function cancelInline() {
  inlineEditingId.value = null;
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col text-foreground">
    <div class="flex shrink-0 items-center justify-between border-b border-border px-3 py-2">
      <span class="text-sm font-medium">任务区</span>
      <button
        type="button"
        class="rounded-md border border-border p-1 hover:bg-muted"
        title="添加任务"
        @click="startDraft"
      >
        <Plus class="size-4" />
      </button>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto">
      <section data-drop-zone="tasks">
        <button
          type="button"
          class="flex w-full items-center justify-between px-3 py-2 text-left text-xs font-medium text-muted-foreground hover:bg-muted/30"
          :title="tasksExpanded ? '收起' : '展开'"
          @click="tasksExpanded = !tasksExpanded"
        >
          <span>任务 ({{ taskItems.length }})</span>
          <ChevronDown v-if="tasksExpanded" class="size-4" aria-hidden="true" />
          <ChevronRight v-else class="size-4" aria-hidden="true" />
        </button>
        <div v-show="tasksExpanded" class="min-h-[2rem] transition-colors">
          <div v-if="draftActive" class="border-b border-border/60 px-3 py-2" @keydown.esc.prevent="cancelDraft">
            <input
              ref="draftInputRef"
              v-model="draftValue"
              class="w-full rounded-md border border-input bg-background px-2 py-1 text-sm"
              placeholder="请输入标题"
              @keydown.enter.prevent="commitDraft"
              @blur="commitDraft"
            />
          </div>
          <TaskListItem
            v-for="task in taskItems"
            :key="task.id"
            :task="task"
            :inline-editing="inlineEditingId === task.id"
            @open-edit="openEdit"
            @inline-save="saveInline"
            @inline-cancel="cancelInline"
            @drag-start="startTaskDrag"
            @inline-edit="startInlineEdit"
          />
          <p v-if="!draftActive && taskItems.length === 0" class="px-3 py-4 text-xs text-muted-foreground">
            暂无任务，点击右上角加号添加
          </p>
        </div>
      </section>

      <section class="border-t border-border" data-drop-zone="stamps">
        <button
          type="button"
          class="flex w-full items-center justify-between px-3 py-2 text-left text-xs font-medium text-muted-foreground hover:bg-muted/30"
          :title="stampsExpanded ? '收起' : '展开'"
          @click="stampsExpanded = !stampsExpanded"
        >
          <span>图章 ({{ stampItems.length }})</span>
          <ChevronDown v-if="stampsExpanded" class="size-4" aria-hidden="true" />
          <ChevronRight v-else class="size-4" aria-hidden="true" />
        </button>
        <div v-show="stampsExpanded" class="min-h-[2rem] transition-colors">
          <TaskListItem
            v-for="task in stampItems"
            :key="task.id"
            :task="task"
            :inline-editing="inlineEditingId === task.id"
            @open-edit="openEdit"
            @inline-save="saveInline"
            @inline-cancel="cancelInline"
            @drag-start="startTaskDrag"
            @inline-edit="startInlineEdit"
          />
          <p v-if="stampItems.length === 0" class="px-3 py-4 text-xs text-muted-foreground">
            暂无图章，可将任务转为图章或从图章列表拖入
          </p>
        </div>
      </section>
    </div>

    <TaskDialog
      :open="dialogOpen"
      :task="dialogTask"
      @close="dialogOpen = false"
      @save="saveDialog"
      @delete="deleteFromDialog"
    />
  </div>
</template>
