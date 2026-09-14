import { workspaceForPhase, workspaces, type WorkflowPhase } from "./workspaceModel";

interface WorkspaceNavProps {
  activePhase: WorkflowPhase;
}

export function WorkspaceNav({ activePhase }: WorkspaceNavProps) {
  const currentWorkspaceId = workspaceForPhase(activePhase);
  const currentWorkspace = workspaces.find((workspace) => workspace.id === currentWorkspaceId);

  return (
    <section className="workspace-list" role="region" aria-labelledby="workspace-list-title">
      <h2 id="workspace-list-title">Espacio actual</h2>
      <p>
        <strong>{currentWorkspace?.label ?? "Analizar"}</strong>
        <span> · En uso</span>
      </p>
    </section>
  );
}
