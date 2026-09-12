import type { ProjectActivePhase } from "../../bridge/project-contracts";

export type WorkflowPhase = ProjectActivePhase;

export const workflowPhases = [
  { id: "load", number: "01", label: "Cargar", description: "Elegir una fuente local" },
  { id: "review", number: "02", label: "Revisar", description: "Entender señales y calidad" },
  { id: "prepare", number: "03", label: "Preparar", description: "Corregir y transformar" },
  { id: "deliver", number: "04", label: "Entregar", description: "Validar y exportar" },
] as const satisfies readonly {
  id: ProjectActivePhase;
  number: string;
  label: string;
  description: string;
}[];

export const workflowPhaseIds = workflowPhases.map(({ id }) => id);

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
} as const satisfies Record<ProjectActivePhase, WorkspaceId>;

export function workspaceForPhase(phase: WorkflowPhase): WorkspaceId {
  return workspaceByPhase[phase];
}
