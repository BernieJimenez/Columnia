import { invoke } from "@tauri-apps/api/core";
import { expectShape } from "./shape";
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
  return invoke<ProjectOpenResult>("restore_project_version", { projectId, versionId })
    .then((result) => checkedProjectOpenResult(result));
}

export function openProject(projectId: string): Promise<ProjectOpenResult> {
  return invoke<ProjectOpenResult>("open_project", { projectId }).then((result) => checkedProjectOpenResult(result));
}

function checkedProjectOpenResult(result: unknown): ProjectOpenResult {
  const opened = expectShape<ProjectOpenResult>(result, "el proyecto", { project: "object", dataset: "object", workspace: "object", profile: "object-or-null" });
  expectShape(opened.workspace, "la configuración del proyecto", { qualityRules: "array" });
  return opened;
}

export function deleteProject(projectId: string): Promise<void> {
  return invoke<void>("delete_project", { projectId });
}
