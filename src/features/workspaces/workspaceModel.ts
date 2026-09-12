export const workflowPhaseIds = ["load", "review", "prepare", "deliver"] as const;

export type WorkflowPhase = (typeof workflowPhaseIds)[number];

export const workspaces = [
  { id: "analyze", label: "Analizar", status: "Actual", available: true },
  {
    id: "automate",
    label: "Automatizar",
    status: "CLI disponible · Interfaz en preparación",
    available: false,
  },
  {
    id: "bi",
    label: "Preparar para BI",
    status: "Interfaz planificada",
    available: false,
  },
] as const;

export type WorkspaceId = (typeof workspaces)[number]["id"];

export const workspaceByPhase = {
  load: "analyze",
  review: "analyze",
  prepare: "analyze",
  deliver: "analyze",
} as const satisfies Record<WorkflowPhase, WorkspaceId>;

export function workspaceForPhase(phase: WorkflowPhase): WorkspaceId {
  return workspaceByPhase[phase];
}
