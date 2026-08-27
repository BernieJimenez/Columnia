import { readFileSync } from "node:fs";
import { join } from "node:path";

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { ProjectSummary } from "../bridge";
import { ProjectsPanel } from "../features/projects/ProjectsPanel";
import { ModalDialog } from "./ModalDialog";

afterEach(cleanup);

const appSource = readFileSync(join(process.cwd(), "src", "App.tsx"), "utf8");

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
  it("mantiene landmarks y el skip link hacia el contenido principal", () => {
    expect(appSource).toMatch(/<a className="skip-link" href="#main-content">/);
    expect(appSource).toMatch(/<aside className="sidebar" aria-label="Navegación principal">/);
    expect(appSource).toMatch(/<nav className="side-nav" aria-label="Flujo de preparación de datos">/);
    expect(appSource).toMatch(/<main id="main-content" className="main-content" tabIndex=\{-1\}>/);
    expect(appSource).toContain('aria-current={activePhase === phase.id ? "step" : undefined}');
    expect(appSource).toContain("aria-busy={operationBusy}");
  });

  it("expone nombres accesibles para las acciones de proyectos y su estado ocupado", () => {
    render(
      <ProjectsPanel
        catalog={{ kind: "ready", projects: [project], recoveryCandidate: null }}
        operation={{ kind: "idle" }}
        deletion={{ kind: "idle" }}
        activeProject={null}
        datasetFileName="ventas.csv"
        disabled={true}
        onSave={vi.fn()}
        onOpen={vi.fn()}
        onImportSession={vi.fn()}
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
