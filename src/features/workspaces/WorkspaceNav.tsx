import { workspaceForPhase, workspaces, type WorkflowPhase } from "./workspaceModel";

interface WorkspaceNavProps {
  activePhase: WorkflowPhase;
}

export function WorkspaceNav({ activePhase }: WorkspaceNavProps) {
  const currentWorkspaceId = workspaceForPhase(activePhase);

  return (
    <section className="workspace-list" role="region" aria-labelledby="workspace-list-title">
      <h2 id="workspace-list-title">Espacios de trabajo</h2>
      <ul>
        {workspaces.map((workspace) => {
          const isCurrent = workspace.id === currentWorkspaceId;

          return (
            <li key={workspace.id} aria-current={isCurrent ? "true" : undefined}>
              <strong>{workspace.label}</strong>
              <span>{workspace.status}</span>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
