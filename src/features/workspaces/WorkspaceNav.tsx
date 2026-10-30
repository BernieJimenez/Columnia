import { workspaceForPhase, workspaces, type WorkflowPhase } from "./workspaceModel";

interface WorkspaceNavProps {
  activePhase: WorkflowPhase;
}

/**
 * Names the workspace the flow belongs to. Only Analizar is actionable, so it
 * is a label, not navigation (DESIGN.md, «Navegación y espacios de trabajo»).
 */
export function WorkspaceNav({ activePhase }: WorkspaceNavProps) {
  const currentWorkspaceId = workspaceForPhase(activePhase);
  const currentWorkspace = workspaces.find((workspace) => workspace.id === currentWorkspaceId);

  return (
    <p className="workspace-label">
      <span className="visually-hidden">Espacio de trabajo: </span>
      {currentWorkspace?.label ?? "Analizar"}
    </p>
  );
}
