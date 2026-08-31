import type { OperationProgress, ProjectSummary } from "../../bridge";

export const MAX_PROJECT_NAME_LENGTH = 128;

export type ProjectCatalogState =
  | { kind: "unavailable" }
  | { kind: "loading" }
  | {
      kind: "ready";
      projects: ProjectSummary[];
      recoveryCandidate: ProjectSummary | null;
    }
  | { kind: "error"; message: string };

export type ProjectOperationState =
  | { kind: "idle" }
  | {
      kind: "working";
      operation: "save" | "open" | "delete";
      projectId: string | null;
      progress?: OperationProgress;
    }
  | { kind: "success"; message: string }
  | { kind: "error"; message: string };

export type ProjectDeletionState =
  | { kind: "idle" }
  | { kind: "confirming"; project: ProjectSummary };

export function validateProjectName(value: string): { valid: true; name: string } | { valid: false; message: string } {
  const name = value.trim();
  if (!name) return { valid: false, message: "Escribe un nombre para el proyecto." };
  if (name.length > MAX_PROJECT_NAME_LENGTH) {
    return { valid: false, message: `El nombre admite hasta ${MAX_PROJECT_NAME_LENGTH} caracteres.` };
  }
  return { valid: true, name };
}

export function sortProjects(projects: ProjectSummary[]): ProjectSummary[] {
  return [...projects].sort((left, right) => {
    const byUpdated = right.updatedAt.localeCompare(left.updatedAt);
    return byUpdated || left.name.localeCompare(right.name, "es", { sensitivity: "base" });
  });
}

export function suggestedProjectName(fileName: string | null): string {
  if (!fileName) return "";
  const withoutExtension = fileName.replace(/\.[^.]+$/, "").trim();
  return withoutExtension.slice(0, MAX_PROJECT_NAME_LENGTH);
}
