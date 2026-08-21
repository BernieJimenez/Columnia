import { useEffect, useState } from "react";

import { ModalDialog } from "../../components/ModalDialog";
import type { ProjectSummary } from "../../bridge";
import {
  MAX_PROJECT_NAME_LENGTH,
  suggestedProjectName,
  validateProjectName,
  type ProjectCatalogState,
  type ProjectDeletionState,
  type ProjectOperationState,
} from "./projectModel";

interface ProjectsPanelProps {
  catalog: ProjectCatalogState;
  operation: ProjectOperationState;
  deletion: ProjectDeletionState;
  activeProject: ProjectSummary | null;
  datasetFileName: string | null;
  disabled: boolean;
  onSave: (name: string) => void;
  onOpen: (projectId: string) => void;
  onDeleteRequest: (project: ProjectSummary) => void;
  onDeleteCancel: () => void;
  onDeleteConfirm: () => void;
  onRetry: () => void;
  onClearFeedback: () => void;
}

function projectDate(value: string): string {
  const parsed = new Date(value);
  return Number.isNaN(parsed.valueOf()) ? value : new Intl.DateTimeFormat("es", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(parsed);
}

export function ProjectsPanel({
  catalog,
  operation,
  deletion,
  activeProject,
  datasetFileName,
  disabled,
  onSave,
  onOpen,
  onDeleteRequest,
  onDeleteCancel,
  onDeleteConfirm,
  onRetry,
  onClearFeedback,
}: ProjectsPanelProps) {
  const [name, setName] = useState("");
  const validation = validateProjectName(name);

  useEffect(() => {
    setName(activeProject?.name ?? suggestedProjectName(datasetFileName));
  }, [activeProject?.id, activeProject?.name, datasetFileName]);

  if (catalog.kind === "unavailable") return null;

  const projects = catalog.kind === "ready" ? catalog.projects : [];
  const recovery = catalog.kind === "ready" ? catalog.recoveryCandidate : null;

  return (
    <section className="projects" aria-labelledby="projects-title" aria-busy={disabled}>
      <div className="projects__heading">
        <div>
          <p className="eyebrow">Continuidad local</p>
          <h3 id="projects-title">Proyectos</h3>
          <p>Un proyecto conserva un snapshot materializado del dataset. No recupera historial, perfil, reglas ni borradores de receta.</p>
        </div>
        {catalog.kind === "error" && (
          <button type="button" className="secondary-action" onClick={onRetry} disabled={disabled}>Reintentar</button>
        )}
      </div>

      {catalog.kind === "loading" && <p className="notice" role="status">Cargando proyectos locales…</p>}
      {catalog.kind === "error" && <p className="notice notice--error" role="alert">No se pudieron cargar los proyectos: {catalog.message}</p>}

      {recovery && (
        <div className="project-recovery">
          <div>
            <strong>Continuar la última sesión</strong>
            <span>{recovery.name} · {recovery.datasetFileName}</span>
          </div>
          <button type="button" className="primary-action" onClick={() => onOpen(recovery.id)} disabled={disabled}>
            Recuperar snapshot
          </button>
        </div>
      )}

      <form className="project-save" onSubmit={(event) => { event.preventDefault(); onSave(name); }} noValidate>
        <label htmlFor="project-name">Nombre del proyecto</label>
        <div>
          <input
            id="project-name"
            value={name}
            maxLength={MAX_PROJECT_NAME_LENGTH}
            onChange={(event) => { setName(event.target.value); onClearFeedback(); }}
            disabled={disabled || !datasetFileName}
            aria-invalid={!validation.valid && name.length > 0}
            aria-describedby="project-name-help"
          />
          <button type="submit" className="primary-action" disabled={disabled || !datasetFileName || !validation.valid}>
            {activeProject ? "Actualizar proyecto" : "Guardar proyecto nuevo"}
          </button>
        </div>
        <small id="project-name-help">Entre 1 y {MAX_PROJECT_NAME_LENGTH} caracteres; se recortan espacios al guardar.</small>
      </form>

      {operation.kind === "working" && <p className="notice" role="status">Procesando proyecto…</p>}
      {operation.kind === "success" && <p className="notice notice--success" role="status">{operation.message}</p>}
      {operation.kind === "error" && <p className="notice notice--error" role="alert">{operation.message}</p>}

      {catalog.kind === "ready" && projects.length === 0 && (
        <p className="projects__empty">Todavía no hay proyectos guardados.</p>
      )}
      {projects.length > 0 && (
        <ul className="project-list" aria-label="Proyectos guardados">
          {projects.map((project) => {
            const active = activeProject?.id === project.id;
            return (
              <li key={project.id} className={active ? "project-list__active" : undefined}>
                <div>
                  <strong>{project.name}{active ? " · activo" : ""}</strong>
                  <span>{project.datasetFileName} · {project.rowCount.toLocaleString("es")} filas · {project.columnCount} columnas</span>
                  <small>Actualizado {projectDate(project.updatedAt)}</small>
                </div>
                <div className="project-list__actions">
                  <button type="button" onClick={() => onOpen(project.id)} disabled={disabled}>Abrir</button>
                  <button type="button" className="project-list__delete" onClick={() => onDeleteRequest(project)} disabled={disabled}>Eliminar</button>
                </div>
              </li>
            );
          })}
        </ul>
      )}

      {deletion.kind === "confirming" && (
        <ModalDialog
          role="alertdialog"
          labelledBy="delete-project-title"
          describedBy="delete-project-description"
          onDismiss={onDeleteCancel}
        >
          <p className="eyebrow">Confirmación requerida</p>
          <h3 id="delete-project-title">Eliminar “{deletion.project.name}”</h3>
          <p id="delete-project-description">Se borrará el snapshot persistente. El dataset abierto en memoria no se descartará.</p>
          <div className="sheet-dialog__actions">
            <button type="button" className="secondary-action" onClick={onDeleteCancel} disabled={disabled}>Cancelar</button>
            <button type="button" className="danger-action" onClick={onDeleteConfirm} disabled={disabled}>Eliminar proyecto</button>
          </div>
        </ModalDialog>
      )}
    </section>
  );
}
