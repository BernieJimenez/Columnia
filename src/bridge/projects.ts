import { invoke } from "@tauri-apps/api/core";
import type {
  ProjectSummary,
  ProjectOpenResult,
  ProjectWorkspace,
} from "./contracts";

export function listProjects(): Promise<ProjectSummary[]> {
  return invoke<ProjectSummary[]>("list_projects");
}

export function getRecoveryCandidate(): Promise<ProjectSummary | null> {
  return invoke<ProjectSummary | null>("get_recovery_candidate");
}

export function saveProject(
  projectId: string | null,
  name: string,
  workspace: ProjectWorkspace,
): Promise<ProjectSummary> {
  return invoke<ProjectSummary>("save_project", { projectId, name, workspace });
}

export function openProject(projectId: string): Promise<ProjectOpenResult> {
  return invoke<ProjectOpenResult>("open_project", { projectId });
}

export function deleteProject(projectId: string): Promise<void> {
  return invoke<void>("delete_project", { projectId });
}
