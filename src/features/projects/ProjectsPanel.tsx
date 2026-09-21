import { useEffect, useState } from "react";

import { ModalDialog } from "../../components/ModalDialog";
import type { ProjectSummary, ProjectVersionSummary } from "../../bridge";
import {
  MAX_PROJECT_NAME_LENGTH,
  suggestedProjectName,
  validateProjectName,
  type ProjectCatalogState,
  type ProjectDeletionState,
  type ProjectAutoSaveState,
  type ProjectOperationState,
  type ProjectVersionsState,
} from "./projectModel";

interface ProjectsPanelProps {
  catalog: ProjectCatalogState;
  operation: ProjectOperationState;
  deletion: ProjectDeletionState;
  activeProject: ProjectSummary | null;
  versions: ProjectVersionsState;
  autoSave: ProjectAutoSaveState;
  autoSaveEnabled: boolean;
  datasetFileName: string | null;
  disabled: boolean;
  openCancellationPending?: boolean;
  onSave: (name: string) => void;
  onOpen: (projectId: string) => void;
  onCancelOpen?: () => void;
  onRestore: (projectId: string, versionId: number) => void;
  onAutoSaveChange: (enabled: boolean) => void;
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
  versions,
  autoSave,
  autoSaveEnabled,
  datasetFileName,
  disabled,
  openCancellationPending = false,
  onSave,
  onOpen,
  onCancelOpen,
  onRestore,
  onAutoSaveChange,
  onDeleteRequest,
  onDeleteCancel,
  onDeleteConfirm,
  onRetry,
  onClearFeedback,
}: ProjectsPanelProps) {
  const [name, setName] = useState("");
  const [versionToRestore, setVersionToRestore] = useState<ProjectVersionSummary | null>(null);
  const validation = validateProjectName(name);

  useEffect(() => {
    setName(activeProject?.name ?? suggestedProjectName(datasetFileName));
  }, [activeProject?.id, activeProject?.name, datasetFileName]);

  if (catalog.kind === "unavailable") return null;

  const projects = catalog.kind === "ready" ? catalog.projects : [];
  const recovery = catalog.kind === "ready" ? catalog.recoveryCandidate : null;
  const hasProjectOptions = Boolean(datasetFileName) || projects.length > 0;

  return (
    <section className="projects" aria-labelledby="projects-title" aria-busy={disabled}>
      <div className="projects__heading">
        <div>
          <p className="eyebrow">Continuidad local</p>
          <h3 id="projects-title">Proyectos</h3>
          <p>Continúa la última sesión o abre un proyecto guardado. Guarda y administra copias desde las opciones secundarias.</p>
        </div>
      </div>

      {catalog.kind === "loading" && <p className="notice" role="status">Cargando proyectos locales…</p>}
      {catalog.kind === "error" && (
        <div className="notice notice--error" role="alert">
          <span>No se pudieron cargar los proyectos: {catalog.message}</span>
          <button type="button" className="secondary-action" onClick={onRetry} disabled={disabled}>Reintentar</button>
        </div>
      )}

      {recovery && (
        <div className="project-recovery">
          <div>
            <strong>Continuar la última sesión</strong>
            <span>{recovery.name} · {recovery.datasetFileName}</span>
          </div>
          <button type="button" className="primary-action" onClick={() => onOpen(recovery.id)} disabled={disabled}>
            Recuperar proyecto
          </button>
        </div>
      )}

      {hasProjectOptions && (
        <details className="projects__options">
          <summary>{datasetFileName ? "Guardar y administrar proyectos" : "Abrir o administrar proyectos guardados"}</summary>
          <p>
            {datasetFileName
              ? `Guarda “${datasetFileName}” para continuar después. Aquí también puedes abrir o eliminar copias locales.`
              : "Abre o elimina las copias locales guardadas en este dispositivo."}
          </p>

          {datasetFileName && (
            <form className="project-save" onSubmit={(event) => { event.preventDefault(); onSave(name); }} noValidate>
              <label htmlFor="project-name">Nombre del proyecto</label>
              <div>
                <input
                  id="project-name"
                  value={name}
                  maxLength={MAX_PROJECT_NAME_LENGTH}
                  onChange={(event) => { setName(event.target.value); onClearFeedback(); }}
                  disabled={disabled}
                  aria-invalid={!validation.valid && name.length > 0}
                  aria-describedby="project-name-help"
                />
                <button type="submit" className="primary-action" disabled={disabled || !validation.valid}>
                  {activeProject ? "Actualizar proyecto" : "Guardar proyecto nuevo"}
                </button>
              </div>
              <small id="project-name-help">Entre 1 y {MAX_PROJECT_NAME_LENGTH} caracteres; se recortan espacios al guardar.</small>
            </form>
          )}

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
                      <small>Espacio persistente (snapshot + historial): {formatProjectStorage(project.storageBytes)}</small>
                      <small>Actualizado {projectDate(project.updatedAt)}</small>
                      {active && (
                        <div className="project-continuity">
                          <label className="project-autosave">
                            <input
                              type="checkbox"
                              checked={autoSaveEnabled}
                              onChange={(event) => onAutoSaveChange(event.target.checked)}
                              disabled={disabled}
                            />
                            <span>Autoguardar este proyecto</span>
                          </label>
                          <small>Hasta 5 versiones anteriores y 512 MiB. La última versión válida se conserva aunque supere la cuota.</small>
                          {autoSave.kind === "saving" && <small role="status">Guardando automáticamente…</small>}
                          {autoSave.kind === "saved" && <small role="status">Guardado automáticamente · {projectDate(autoSave.savedAt)}</small>}
                          {autoSave.kind === "error" && <small className="project-autosave__error" role="alert">Error al guardar automáticamente: {autoSave.message}</small>}
                          {versions.kind === "loading" && <small role="status">Cargando versiones…</small>}
                          {versions.kind === "error" && <small className="project-autosave__error" role="alert">No se pudieron cargar las versiones: {versions.message}</small>}
                          {versions.kind === "ready" && versions.versions.length > 0 && (
                            <details className="project-versions">
                              <summary>Versiones anteriores ({versions.versions.length})</summary>
                              <ul>
                                {versions.versions.map((version) => (
                                  <li key={version.id}>
                                    <span>{projectDate(version.createdAt)} · {version.rowCount.toLocaleString("es")} filas · {formatProjectStorage(version.storageBytes)}</span>
                                    <button
                                      type="button"
                                      className="secondary-action"
                                      disabled={disabled}
                                      onClick={() => setVersionToRestore(version)}
                                    >
                                      Restaurar
                                    </button>
                                  </li>
                                ))}
                              </ul>
                            </details>
                          )}
                        </div>
                      )}
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
        </details>
      )}

      {!datasetFileName && catalog.kind === "ready" && projects.length === 0 && (
        <p className="projects__empty">Todavía no hay proyectos guardados. Carga un archivo para empezar.</p>
      )}

      {operation.kind === "working" && (
        <div className="notice projects__operation">
          <p role="status">
            {operation.operation === "open"
              ? openCancellationPending ? "Cancelando apertura del proyecto…" : "Abriendo proyecto…"
              : "Procesando proyecto…"}
          </p>
          {operation.operation === "open" && onCancelOpen && (
            <button
              type="button"
              className="secondary-action"
              onClick={onCancelOpen}
              disabled={openCancellationPending}
            >
              {openCancellationPending ? "Cancelando…" : "Cancelar apertura"}
            </button>
          )}
        </div>
      )}
      {operation.kind === "success" && <p className="notice notice--success" role="status">{operation.message}</p>}
      {operation.kind === "error" && <p className="notice notice--error" role="alert">{operation.message}</p>}

      {deletion.kind === "confirming" && (
        <ModalDialog
          role="alertdialog"
          labelledBy="delete-project-title"
          describedBy="delete-project-description"
          onDismiss={onDeleteCancel}
        >
          <p className="eyebrow">Confirmación requerida</p>
          <h3 id="delete-project-title">Eliminar “{deletion.project.name}”</h3>
          <p id="delete-project-description">Se borrarán el dataset, el perfil y el historial persistentes. El dataset abierto en memoria no se descartará.</p>
          <div className="sheet-dialog__actions">
            <button type="button" className="secondary-action" onClick={onDeleteCancel} disabled={disabled}>Cancelar</button>
            <button type="button" className="danger-action" onClick={onDeleteConfirm} disabled={disabled}>Eliminar proyecto</button>
          </div>
        </ModalDialog>
      )}

      {versionToRestore && activeProject && (
        <ModalDialog
          role="alertdialog"
          labelledBy="restore-project-version-title"
          describedBy="restore-project-version-description"
          onDismiss={() => setVersionToRestore(null)}
        >
          <p className="eyebrow">Restauración segura</p>
          <h3 id="restore-project-version-title">Restaurar versión anterior</h3>
          <p id="restore-project-version-description">
            La versión actual quedará guardada como una versión anterior antes de restaurar la del {projectDate(versionToRestore.createdAt)}.
          </p>
          <div className="sheet-dialog__actions">
            <button type="button" className="secondary-action" onClick={() => setVersionToRestore(null)} disabled={disabled}>Cancelar</button>
            <button
              type="button"
              className="primary-action"
              disabled={disabled}
              onClick={() => {
                onRestore(activeProject.id, versionToRestore.id);
                setVersionToRestore(null);
              }}
            >
              Restaurar versión
            </button>
          </div>
        </ModalDialog>
      )}
    </section>
  );
}

function formatProjectStorage(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined || !Number.isFinite(bytes) || bytes < 0) {
    return "tamaño no disponible";
  }
  if (bytes < 1024) return `${Math.round(bytes)} B`;
  const units = ["KiB", "MiB", "GiB", "TiB"];
  let value = bytes;
  let unitIndex = -1;
  do {
    value /= 1024;
    unitIndex += 1;
  } while (value >= 1024 && unitIndex < units.length - 1);
  return `${value.toLocaleString("es", { maximumFractionDigits: 1 })} ${units[unitIndex]}`;
}
