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
    onSaveCopy: vi.fn(),
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
  it("lleva el foco al resultado si el botón usado desapareció (ACC-05)", () => {
    const props = renderPanel();
    cleanup();
    const view = render(<ProjectsPanel {...props} />);
    (document.activeElement as HTMLElement | null)?.blur();
    view.rerender(<ProjectsPanel {...props} operation={{ kind: "success", message: "Proyecto eliminado." }} />);
    expect(document.activeElement).toBe(screen.getByText("Proyecto eliminado."));
  });

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

describe("ProjectsPanel: estados y guardado (UX-17, QA-35)", () => {
  function openOptions() {
    fireEvent.click(screen.getByText("Guardar y administrar proyectos", { selector: "summary" }));
  }

  it("explica por qué no se puede guardar con el nombre vacío", () => {
    renderPanel();
    openOptions();
    fireEvent.change(screen.getByRole("textbox", { name: "Nombre del proyecto" }), { target: { value: "  " } });
    expect(screen.getByRole("alert")).toHaveTextContent("Escribe un nombre para el proyecto.");
    expect(screen.getByRole("button", { name: "Guardar proyecto nuevo" })).toBeDisabled();
  });

  it("guarda una copia con otro nombre sin renombrar el proyecto abierto", () => {
    const props = renderPanel({ activeProject: recovery });
    openOptions();
    const name = screen.getByRole("textbox", { name: "Nombre del proyecto" });
    expect(screen.getByRole("button", { name: "Guardar como copia" })).toBeDisabled();
    fireEvent.change(name, { target: { value: "Ventas 2027" } });
    fireEvent.click(screen.getByRole("button", { name: "Guardar como copia" }));
    expect(props.onSaveCopy).toHaveBeenCalledWith("Ventas 2027");
    expect(props.onSave).not.toHaveBeenCalled();
  });

  it("muestra los estados de carga, cancelación y error del catálogo", () => {
    renderPanel({ catalog: { kind: "loading" } });
    expect(screen.getByRole("status")).toHaveTextContent("Cargando proyectos locales…");
    cleanup();
    const cancelled = renderPanel({ catalog: { kind: "cancelled" } });
    expect(screen.getByText("Se canceló la carga de proyectos locales.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Reintentar" }));
    expect(cancelled.onRetry).toHaveBeenCalledOnce();
    cleanup();
    renderPanel({ catalog: { kind: "error", message: "No se pudo leer una ruta local: acceso denegado" } });
    expect(screen.getByRole("alert")).toHaveTextContent("No se pudieron cargar los proyectos: No se pudo leer una ruta local: acceso denegado");
  });
});
