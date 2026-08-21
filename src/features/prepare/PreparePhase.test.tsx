import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as bridge from "../../bridge";
import type { DatasetPreview, HistoryState, LoadedRecipe, TransformRecipe } from "../../bridge";
import { PreparePhase } from "./PreparePhase";
import { TransformRecipeEditor } from "./TransformRecipeEditor";
import { EMPTY_HISTORY } from "./prepareModel";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const dataset: DatasetPreview = {
  fileName: "clientes.csv",
  fileSizeBytes: 128,
  rowCount: 2,
  columnCount: 2,
  columns: [
    { name: "nombre", dataType: "String" },
    { name: "total", dataType: "Int64" },
  ],
  rows: [[" Ana ", "10"], ["Luis", "20"]],
};

const emptyRecipe: TransformRecipe = {
  renames: [], casts: [], dateParses: [], filters: [], calculatedColumn: null,
  findReplace: null, keepColumns: null, splitColumn: null, mergeColumns: null,
  outlierTreatments: [], groupSummary: null, contactNormalizations: [], textExtractions: [],
};

describe("PreparePhase", () => {
  it("conserva tabs ARIA y selección explícita para normalizar texto", () => {
    const onNormalizeText = vi.fn();
    render(<PreparePhase
      dataset={dataset}
      profileStatus={{ kind: "idle" }}
      changeStatus={{ kind: "idle" }}
      historyStatus={EMPTY_HISTORY}
      onAnalyzeQuality={() => undefined}
      onCancelProfile={() => undefined}
      onRemoveDuplicates={() => undefined}
      onNormalizeColumns={() => undefined}
      onApplyRecommended={() => undefined}
      onTrimText={() => undefined}
      onNormalizeText={onNormalizeText}
      onApplyTransforms={() => undefined}
      onUndo={() => undefined}
      onRedo={() => undefined}
    />);

    const normalize = screen.getByRole("button", { name: "Normalizar texto seleccionado" });
    expect(normalize).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox", { name: "nombre" }));
    expect(normalize).toBeEnabled();
    fireEvent.click(normalize);
    expect(onNormalizeText).toHaveBeenCalledWith(["nombre"], true);

    const correctionsTab = screen.getByRole("tab", { name: "Correcciones" });
    fireEvent.keyDown(correctionsTab, { key: "ArrowRight" });
    expect(screen.getByRole("tab", { name: "Transformaciones" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("heading", { name: "Preparar estructura y tipos" })).toBeInTheDocument();
  });

  it("respeta disponibilidad y callbacks de Deshacer/Rehacer", () => {
    const onUndo = vi.fn();
    const onRedo = vi.fn();
    const history: HistoryState = {
      ...EMPTY_HISTORY,
      canUndo: true,
      entryCount: 1,
      entries: [{ index: 0, label: "Normalizar texto", isCurrent: true }],
    };
    render(<PreparePhase
      dataset={dataset} profileStatus={{ kind: "idle" }} changeStatus={{ kind: "idle" }}
      historyStatus={history} onAnalyzeQuality={() => undefined} onCancelProfile={() => undefined}
      onRemoveDuplicates={() => undefined} onNormalizeColumns={() => undefined}
      onApplyRecommended={() => undefined} onTrimText={() => undefined}
      onNormalizeText={() => undefined} onApplyTransforms={() => undefined}
      onUndo={onUndo} onRedo={onRedo}
    />);

    fireEvent.click(screen.getByRole("button", { name: "Deshacer" }));
    expect(onUndo).toHaveBeenCalledOnce();
    expect(screen.getByRole("button", { name: "Rehacer" })).toBeDisabled();
    expect(screen.getByText(/Etapa actual: Normalizar texto/)).toBeInTheDocument();
  });
});

describe("TransformRecipeEditor", () => {
  it("guarda el borrador validado con el nombre visible", async () => {
    const saved: LoadedRecipe = {
      version: 1,
      name: "Mi receta",
      savedAt: "2026-08-21T00:00:00Z",
      recipe: { ...emptyRecipe, renames: [{ from: "nombre", to: "cliente" }] },
    };
    const save = vi.spyOn(bridge, "saveTransformRecipe").mockResolvedValue(saved);
    render(<TransformRecipeEditor dataset={dataset} busy={false} onApply={() => undefined} />);
    fireEvent.change(screen.getByLabelText("Columna para renombrar 1"), { target: { value: "nombre" } });
    fireEvent.change(screen.getByLabelText("Nuevo nombre 1"), { target: { value: "cliente" } });
    fireEvent.click(screen.getByRole("button", { name: "Guardar receta" }));

    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Receta guardada: Mi receta"));
    expect(save).toHaveBeenCalledWith(expect.objectContaining({
      renames: [{ from: "nombre", to: "cliente" }],
    }), "Mi receta");
  });

  it("interpone el alertdialog antes de aplicar filtros destructivos", () => {
    const onApply = vi.fn();
    render(<TransformRecipeEditor dataset={dataset} busy={false} onApply={onApply} />);
    fireEvent.click(screen.getByRole("button", { name: "+ Añadir filtro AND" }));
    fireEvent.change(screen.getByLabelText("Columna del filtro 1"), { target: { value: "nombre" } });
    fireEvent.change(screen.getByLabelText("Valor del filtro 1"), { target: { value: "Ana" } });
    fireEvent.click(screen.getByRole("button", { name: "Aplicar receta" }));

    expect(onApply).not.toHaveBeenCalled();
    expect(screen.getByRole("alertdialog")).toHaveTextContent("1 filtros unidos por AND");
    fireEvent.click(screen.getByRole("button", { name: "Confirmar y aplicar" }));
    expect(onApply).toHaveBeenCalledWith(expect.objectContaining({
      filters: [{ column: "nombre", operator: "eq", value: "Ana" }],
    }));
  });

  it("confirma antes de reemplazar un borrador al cargar receta", async () => {
    const loaded: LoadedRecipe = {
      version: 1,
      name: "Receta cargada",
      savedAt: "2026-08-21T00:00:00Z",
      recipe: { ...emptyRecipe, renames: [{ from: "nombre", to: "cliente" }] },
    };
    vi.spyOn(bridge, "pickTransformRecipe").mockResolvedValue(loaded);
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    render(<TransformRecipeEditor dataset={dataset} busy={false} onApply={() => undefined} />);
    fireEvent.change(screen.getByLabelText("Columna para renombrar 1"), { target: { value: "nombre" } });
    fireEvent.change(screen.getByLabelText("Nuevo nombre 1"), { target: { value: "borrador" } });
    fireEvent.click(screen.getByRole("button", { name: "Cargar receta" }));

    await waitFor(() => expect(confirm).toHaveBeenCalledOnce());
    expect(screen.getByLabelText("Nuevo nombre 1")).toHaveValue("borrador");
    expect(screen.queryByText(/Receta cargada:/)).not.toBeInTheDocument();
  });
});
