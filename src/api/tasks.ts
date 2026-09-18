import { invoke } from "@tauri-apps/api/core";

export type TaskRow = {
  id: string;
  summary: string;
  description?: string | null;
  isStamp: boolean;
  sortOrder: number;
  createdAt: number;
  updatedAt: number;
};

export type CreateTaskInput = {
  summary: string;
  description?: string | null;
  isStamp: boolean;
};

export type UpdateTaskInput = {
  summary: string;
  description?: string | null;
  isStamp: boolean;
};

export function listTasks() {
  return invoke<TaskRow[]>("list_tasks");
}

export function createTask(input: CreateTaskInput) {
  return invoke<TaskRow>("create_task", { input });
}

export function updateTask(id: string, input: UpdateTaskInput) {
  return invoke<TaskRow>("update_task", { id, input });
}

export function deleteTask(id: string) {
  return invoke<void>("delete_task", { id });
}
