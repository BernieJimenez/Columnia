import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { DatasetSourceInspection } from "../../bridge";
import { LoadPhase } from "./LoadPhase";
import { workbookInspection } from "./loadModel";

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

describe("LoadPhase", () => {
  it("expone el diálogo accesible y emite acciones nominales para la hoja", () => {
    const onSheetAction = vi.fn();
    render(
      <LoadPhase
        runtime={{ kind: "connected" }}
        datasetStatus={{ kind: "empty" }}
        inspection={workbookInspection(workbook)}
        onSelect={() => undefined}
        onSheetAction={onSheetAction}
        onCancelLoad={() => undefined}
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
        runtime={{ kind: "connected" }}
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
        onSelect={() => undefined}
        onSheetAction={() => undefined}
        onCancelLoad={onCancelLoad}
      />,
    );

    expect(screen.getByRole("heading", { name: "anterior.csv" })).toBeInTheDocument();
    expect(screen.getByLabelText("Progreso: Leyendo filas")).toHaveValue(25);
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(onCancelLoad).toHaveBeenCalledOnce();
  });
});
