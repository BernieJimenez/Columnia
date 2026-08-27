import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { DatasetSourceInspection } from "../../bridge";
import { LoadPhase } from "./LoadPhase";
import { workbookInspection } from "./loadModel";
import type { RecentDataset } from "./recentFilesModel";

afterEach(cleanup);

const workbook: DatasetSourceInspection = {
  selectionId: "opaque-selection",
  fileName: "libro.xlsx",
  fileSizeBytes: 2048,
  format: "excel",
  isCompressedContainer: true,
  defaultSheetId: "sheet-1",
  sheets: [
    { id: "sheet-1", name: "Enero" },
    { id: "sheet-2", name: "Febrero" },
  ],
};

const recentDataset: RecentDataset = {
  id: "recent-ventas",
  fileName: "ventas.csv",
  format: "csv",
  lastOpenedAt: 1_724_640_000_000,
};

function loadPhaseProps(overrides: Partial<React.ComponentProps<typeof LoadPhase>> = {}) {
  return {
    runtime: { kind: "connected" as const },
    datasetStatus: { kind: "empty" as const },
    inspection: { kind: "idle" as const },
    recentDatasets: [],
    onSelect: () => undefined,
    onSelectRecent: () => undefined,
    onClearRecent: () => undefined,
    onRemoveRecent: () => undefined,
    onSheetAction: () => undefined,
    onCancelLoad: () => undefined,
    ...overrides,
  };
}

describe("LoadPhase", () => {
  it("expone el diálogo accesible y emite acciones nominales para la hoja", () => {
    const onSheetAction = vi.fn();
    render(
      <LoadPhase
        {...loadPhaseProps({ onSheetAction })}
        inspection={workbookInspection(workbook)}
      />,
    );

    expect(screen.getByRole("heading", { name: "Trae tus datos a un espacio de trabajo local." })).toBeInTheDocument();
    expect(screen.getByText("Sin límite fijo")).toBeInTheDocument();
    expect(screen.getByRole("dialog", { name: "Elegir hoja de libro.xlsx" })).toHaveAttribute(
      "aria-describedby",
      "sheet-description",
    );
    expect(screen.getByRole("combobox", { name: "Hoja" })).toHaveFocus();
    fireEvent.change(screen.getByRole("combobox", { name: "Hoja" }), {
      target: { value: "sheet-2" },
    });
    fireEvent.click(screen.getByRole("radio", { name: "Generar encabezados (column_1, column_2…)" }));
    fireEvent.click(screen.getByRole("button", { name: "Cargar hoja" }));

    expect(onSheetAction).toHaveBeenNthCalledWith(1, { kind: "sheet_changed", sheetId: "sheet-2" });
    expect(onSheetAction).toHaveBeenNthCalledWith(2, {
      kind: "header_mode_changed",
      headerMode: "generated",
    });
    expect(onSheetAction).toHaveBeenNthCalledWith(3, { kind: "confirmed" });
  });

  it("conserva el dataset anterior visible mientras la carga se puede cancelar", () => {
    const onCancelLoad = vi.fn();
    render(
      <LoadPhase
        {...loadPhaseProps({ onCancelLoad })}
        datasetStatus={{
          kind: "loading",
          progress: { operation: "load", stage: "Leyendo filas", percent: 25 },
          cancelRequested: false,
          previous: {
            kind: "ready",
            dataset: {
              fileName: "anterior.csv",
              fileSizeBytes: 10,
              rowCount: 1,
              columnCount: 1,
              columns: [{ name: "id", dataType: "String" }],
              rows: [["1"]],
            },
            pageOffset: 0,
            pageLoading: false,
          },
        }}
        inspection={{ kind: "inspecting" }}
      />,
    );

    expect(screen.getByRole("heading", { name: "anterior.csv" })).toBeInTheDocument();
    expect(screen.getByLabelText("Progreso: Leyendo filas")).toHaveValue(25);
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(onCancelLoad).toHaveBeenCalledOnce();
  });

  it("ofrece volver a elegir desde el historial y permite limpiarlo", () => {
    const onSelectRecent = vi.fn();
    const onClearRecent = vi.fn();
    const onRemoveRecent = vi.fn();
    render(
      <LoadPhase
        {...loadPhaseProps({
          recentDatasets: [recentDataset],
          onSelectRecent,
          onClearRecent,
          onRemoveRecent,
        })}
      />,
    );

    expect(screen.getByRole("heading", { name: "Archivos recientes" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Elegir de nuevo" }));
    fireEvent.click(screen.getByRole("button", { name: "Quitar ventas.csv del historial" }));
    fireEvent.click(screen.getByRole("button", { name: "Limpiar historial" }));

    expect(onSelectRecent).toHaveBeenCalledWith(recentDataset);
    expect(onRemoveRecent).toHaveBeenCalledWith("recent-ventas");
    expect(onClearRecent).toHaveBeenCalledOnce();
  });

  it("deshabilita volver a elegir fuera de Tauri, pero conserva el historial visible", () => {
    render(
      <LoadPhase
        {...loadPhaseProps({
          runtime: { kind: "browser" },
          recentDatasets: [recentDataset],
        })}
      />,
    );

    expect(screen.getByRole("button", { name: "Elegir de nuevo" })).toBeDisabled();
    expect(screen.getByText("ventas.csv")).toBeInTheDocument();
  });
});
