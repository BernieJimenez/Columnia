import { invoke } from "@tauri-apps/api/core";
import type {
  ProjectCatalogSnapshot,
  ProjectSummary,
  ProjectOpenResult,
  ProjectWorkspace,
  ProjectVersionSummary,
} from "./contracts";

export function listProjects(): Promise<ProjectCatalogSnapshot> {
  return invoke<ProjectCatalogSnapshot>("list_projects");
}

export function listProjectVersions(projectId: string): Promise<ProjectVersionSummary[]> {
  return invoke<ProjectVersionSummary[]>("list_project_versions", { projectId });
}

export function saveProject(
  projectId: string | null,
  name: string,
  workspace: ProjectWorkspace,
): Promise<ProjectSummary> {
  return invoke<ProjectSummary>("save_project", { projectId, name, workspace });
}

export function autosaveProject(
  projectId: string,
  name: string,
  workspace: ProjectWorkspace,
): Promise<ProjectSummary> {
  return invoke<ProjectSummary>("autosave_project", { projectId, name, workspace });
}

export function restoreProjectVersion(
  projectId: string,
  versionId: number,
): Promise<ProjectOpenResult> {
  return invoke<ProjectOpenResult>("restore_project_version", { projectId, versionId });
}

export function openProject(projectId: string): Promise<ProjectOpenResult> {
  return invoke<ProjectOpenResult>("open_project", { projectId });
}

export function deleteProject(projectId: string): Promise<void> {
  return invoke<void>("delete_project", { projectId });
}
