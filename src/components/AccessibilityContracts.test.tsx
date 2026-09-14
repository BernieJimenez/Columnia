import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { ProjectSummary, ProjectVersionSummary } from "../bridge";
import { ProjectsPanel } from "../features/projects/ProjectsPanel";
import { ModalDialog } from "./ModalDialog";

afterEach(cleanup);

const project: ProjectSummary = {
  id: "project-a11y",
  name: "Ventas accesibles",
  datasetFileName: "ventas.csv",
  rowCount: 12,
  columnCount: 3,
  createdAt: "2026-08-20T10:00:00Z",
  updatedAt: "2026-08-21T10:00:00Z",
};

describe("contratos de accesibilidad de la interfaz", () => {
  it("expone nombres accesibles para las acciones de proyectos y su estado ocupado", () => {
    render(
      <ProjectsPanel
        catalog={{ kind: "ready", projects: [project], recoveryCandidate: null }}
        operation={{ kind: "idle" }}
        deletion={{ kind: "idle" }}
        activeProject={null}
        versions={{ kind: "ready", versions: [] }}
        autoSave={{ kind: "disabled" }}
        autoSaveEnabled={false}
        datasetFileName="ventas.csv"
        disabled={true}
        onSave={vi.fn()}
        onOpen={vi.fn()}
        onRestore={vi.fn()}
        onAutoSaveChange={vi.fn()}
        onDeleteRequest={vi.fn()}
        onDeleteCancel={vi.fn()}
        onDeleteConfirm={vi.fn()}
        onRetry={vi.fn()}
        onClearFeedback={vi.fn()}
      />,
    );

    const panel = screen.getByRole("region", { name: "Proyectos" });
    expect(panel).toHaveAttribute("aria-busy", "true");
    expect(screen.getByRole("textbox", { name: "Nombre del proyecto" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Guardar proyecto nuevo" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Abrir" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Eliminar" })).toBeDisabled();
  });

  it("expone el estado de autoguardado y la acción accesible para versiones", () => {
    const version: ProjectVersionSummary = {
      id: 17,
      createdAt: "2026-08-20T10:00:00Z",
      datasetFileName: "ventas.csv",
      rowCount: 8,
      columnCount: 2,
      storageBytes: 2048,
    };

    render(
      <ProjectsPanel
        catalog={{ kind: "ready", projects: [project], recoveryCandidate: null }}
        operation={{ kind: "idle" }}
        deletion={{ kind: "idle" }}
        activeProject={project}
        versions={{ kind: "ready", versions: [version] }}
        autoSave={{ kind: "saving" }}
        autoSaveEnabled={true}
        datasetFileName="ventas.csv"
        disabled={true}
        onSave={vi.fn()}
        onOpen={vi.fn()}
        onRestore={vi.fn()}
        onAutoSaveChange={vi.fn()}
        onDeleteRequest={vi.fn()}
        onDeleteCancel={vi.fn()}
        onDeleteConfirm={vi.fn()}
        onRetry={vi.fn()}
        onClearFeedback={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByText("Guardar y administrar proyectos", { selector: "summary" }));
    expect(screen.getByRole("checkbox", { name: "Autoguardar este proyecto" })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: "Autoguardar este proyecto" })).toBeDisabled();
    expect(screen.getByText("Guardando automáticamente…")).toHaveAttribute("role", "status");
    expect(screen.getByText(/Hasta 5 versiones anteriores y 512 MiB/)).toBeInTheDocument();

    fireEvent.click(screen.getByText("Versiones anteriores (1)", { selector: "summary" }));
    expect(screen.getByRole("button", { name: "Restaurar" })).toBeDisabled();
  });

  it("expone un alertdialog modal con nombre y descripción asociados", () => {
    render(
      <ModalDialog
        role="alertdialog"
        labelledBy="a11y-dialog-title"
        describedBy="a11y-dialog-description"
        onDismiss={vi.fn()}
      >
        <h2 id="a11y-dialog-title">Eliminar proyecto</h2>
        <p id="a11y-dialog-description">Esta acción no se puede deshacer.</p>
        <button type="button">Cancelar</button>
      </ModalDialog>,
    );

    const dialog = screen.getByRole("alertdialog", {
      name: "Eliminar proyecto",
      description: "Esta acción no se puede deshacer.",
    });
    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(dialog).toHaveAttribute("aria-labelledby", "a11y-dialog-title");
    expect(dialog).toHaveAttribute("aria-describedby", "a11y-dialog-description");
  });
});
