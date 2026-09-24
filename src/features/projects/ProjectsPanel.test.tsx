import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { ProjectSummary, ProjectVersionSummary } from "../../bridge";
import { ProjectsPanel } from "./ProjectsPanel";

afterEach(cleanup);

const recovery: ProjectSummary = {
  id: "recovery-id",
  name: "Ventas recuperables",
  datasetFileName: "ventas.csv",
  rowCount: 10,
  columnCount: 2,
  createdAt: "2026-08-20T10:00:00Z",
  updatedAt: "2026-08-21T10:00:00Z",
  storageBytes: 1536,
};

function renderPanel(overrides: Partial<ComponentProps<typeof ProjectsPanel>> = {}) {
  const props: ComponentProps<typeof ProjectsPanel> = {
    catalog: { kind: "ready", projects: [recovery], recoveryCandidate: recovery },
    operation: { kind: "idle" },
    deletion: { kind: "idle" },
    activeProject: null,
    versions: { kind: "ready", versions: [] },
    autoSave: { kind: "disabled" },
    autoSaveEnabled: false,
    datasetFileName: "actual.csv",
    disabled: false,
    onSave: vi.fn(),
    onOpen: vi.fn(),
    onRestore: vi.fn(),
    onAutoSaveChange: vi.fn(),
    onDeleteRequest: vi.fn(),
    onDeleteCancel: vi.fn(),
    onDeleteConfirm: vi.fn(),
    onRetry: vi.fn(),
    onClearFeedback: vi.fn(),
    ...overrides,
  };
  render(<ProjectsPanel {...props} />);
  return props;
}

describe("ProjectsPanel", () => {
  it("prioriza la recuperación y oculta guardar y administrar hasta abrir sus opciones", () => {
    const props = renderPanel();
    expect(screen.getByRole("button", { name: "Recuperar proyecto" })).toBeInTheDocument();
    const summary = screen.getByText("Guardar y administrar proyectos", { selector: "summary" });
    const options = summary.closest("details");
    expect(options).not.toHaveAttribute("open");
    expect(screen.getByRole("textbox", { name: "Nombre del proyecto" }).closest("details")).toBe(options);
    expect(screen.getByRole("button", { name: "Abrir" }).closest("details")).toBe(options);
    expect(screen.getByRole("button", { name: "Eliminar" }).closest("details")).toBe(options);

    fireEvent.click(summary);
    expect(options).toHaveAttribute("open");
    expect(screen.getByText(/^Espacio persistente \(snapshot \+ historial\): 1[.,]5 KiB$/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Recuperar proyecto" }));
    expect(props.onOpen).toHaveBeenCalledWith("recovery-id");
    expect(screen.queryByText(/C:\\/)).not.toBeInTheDocument();
  });

  it("conserva el guardado y la administración dentro de una divulgación contextual", () => {
    const props = renderPanel();
    fireEvent.click(screen.getByText("Guardar y administrar proyectos", { selector: "summary" }));
    expect(screen.getByText(/Guarda “actual.csv” para continuar después/)).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Nombre del proyecto" })).toHaveValue("actual");
    fireEvent.click(screen.getByRole("button", { name: "Guardar proyecto nuevo" }));
    expect(props.onSave).toHaveBeenCalledWith("actual");
    fireEvent.click(screen.getByRole("button", { name: "Eliminar" }));
    expect(props.onDeleteRequest).toHaveBeenCalledWith(recovery);
  });

  it("ofrece abrir otros proyectos guardados aunque no haya recuperación disponible", () => {
    const props = renderPanel({
      catalog: { kind: "ready", projects: [recovery], recoveryCandidate: null },
      datasetFileName: null,
    });
    const summary = screen.getByText("Abrir o administrar proyectos guardados", { selector: "summary" });
    const options = summary.closest("details");
    expect(options).not.toHaveAttribute("open");
    expect(screen.getByRole("button", { name: "Abrir" }).closest("details")).toBe(options);

    fireEvent.click(summary);
    expect(options).toHaveAttribute("open");
    fireEvent.click(screen.getByRole("button", { name: "Abrir" }));
    expect(props.onOpen).toHaveBeenCalledWith("recovery-id");
  });

  it("presenta la eliminación como alertdialog y conserva el dataset en memoria", () => {
    const props = renderPanel({ deletion: { kind: "confirming", project: recovery } });
    expect(screen.getByRole("alertdialog", { name: "Eliminar “Ventas recuperables”" })).toHaveTextContent(
      "El dataset abierto en memoria no se descartará.",
    );
    fireEvent.click(screen.getByRole("button", { name: "Eliminar proyecto" }));
    expect(props.onDeleteConfirm).toHaveBeenCalledOnce();
  });

  it("ofrece autoguardado opt-in y restauración explícita de versiones anteriores", () => {
    const version: ProjectVersionSummary = {
      id: 17,
      createdAt: "2026-08-20T10:00:00Z",
      datasetFileName: "ventas.csv",
      rowCount: 8,
      columnCount: 2,
      storageBytes: 2048,
    };
    const props = renderPanel({
      activeProject: recovery,
      versions: { kind: "ready", versions: [version] },
      autoSave: { kind: "saved", savedAt: "2026-08-21T11:00:00Z" },
      autoSaveEnabled: true,
    });
    fireEvent.click(screen.getByText("Guardar y administrar proyectos", { selector: "summary" }));
    const checkbox = screen.getByRole("checkbox", { name: "Autoguardar este proyecto" });
    expect(checkbox).toBeChecked();
    expect(screen.getByText(/Hasta 5 versiones anteriores y 512 MiB/)).toBeInTheDocument();
    expect(screen.getByText(/Guardado automáticamente ·/)).toBeInTheDocument();
    fireEvent.click(screen.getByText("Versiones anteriores (1)", { selector: "summary" }));
    fireEvent.click(screen.getByRole("button", { name: "Restaurar" }));
    expect(screen.getByRole("alertdialog", { name: "Restaurar versión anterior" })).toHaveTextContent(
      "La versión actual quedará guardada como una versión anterior",
    );
    fireEvent.click(screen.getByRole("button", { name: "Restaurar versión" }));
    expect(props.onRestore).toHaveBeenCalledWith(recovery.id, version.id);
    fireEvent.click(checkbox);
    expect(props.onAutoSaveChange).toHaveBeenCalledWith(false);
  });
});
