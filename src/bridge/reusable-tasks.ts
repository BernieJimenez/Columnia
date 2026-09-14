import { invoke } from "@tauri-apps/api/core";

import type {
  ReusableTask,
  ReusableTaskSchema,
  ReusableTaskSchemaCompatibility,
  ReusableTaskSummary,
} from "./reusable-task-contracts";

export function listReusableTasks(): Promise<ReusableTaskSummary[]> {
  return invoke<ReusableTaskSummary[]>("list_reusable_tasks");
}

export function saveReusableTask(
  taskId: string | null,
  task: ReusableTask,
): Promise<ReusableTaskSummary> {
  return invoke<ReusableTaskSummary>("save_reusable_task", { taskId, task });
}

export function openReusableTask(taskId: string): Promise<ReusableTask> {
  return invoke<ReusableTask>("open_reusable_task", { taskId });
}

export function deleteReusableTask(taskId: string): Promise<void> {
  return invoke<void>("delete_reusable_task", { taskId });
}

export function checkReusableTaskSchema(
  taskId: string,
  schema: ReusableTaskSchema,
): Promise<ReusableTaskSchemaCompatibility> {
  return invoke<ReusableTaskSchemaCompatibility>("check_reusable_task_schema", { taskId, schema });
}
