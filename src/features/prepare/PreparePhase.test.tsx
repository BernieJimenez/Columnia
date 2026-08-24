import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as bridge from "../../bridge";
import type { DatasetPreview, DatasetProfile, HistoryState, LoadedRecipe, TransformRecipe } from "../../bridge";
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

const cleaningSignalsProfile: DatasetProfile = {
  rowCount: 5,
  duplicateRowCount: 1,
  duplicatePercentage: 20,
  columns: [{
    name: "email",
    dataType: "String",
    nullCount: 1,
    completenessPercentage: 50,
    uniqueCount: 1,
    minimum: "ana@example.com",
    maximum: "ana@example.com",
    mean: null,
    emptyCount: 0,
    minimumLength: 15,
    maximumLength: 15,
    averageLength: 15,
    suggestedType: "boolean",
    typeMatchPercentage: 100,
    invalidTypeCount: 1,
    sentinelCount: 1,
    privacySignal: "email",
    standardDeviation: null,
    firstQuartile: null,
    median: null,
    thirdQuartile: null,
    outlierCount: null,
  }, {
    name: "empty_column",
    dataType: "String",
    nullCount: 5,
    completenessPercentage: 0,
    uniqueCount: 0,
    minimum: null,
    maximum: null,
    mean: null,
    emptyCount: 0,
    minimumLength: null,
    maximumLength: null,
    averageLength: null,
    suggestedType: null,
    typeMatchPercentage: null,
    invalidTypeCount: null,
    sentinelCount: null,
    privacySignal: null,
    standardDeviation: null,
    firstQuartile: null,
    median: null,
    thirdQuartile: null,
    outlierCount: null,
  }, {
    name: "high_null",
    dataType: "String",
    nullCount: 4,
    completenessPercentage: 20,
    uniqueCount: 1,
    minimum: "ok",
    maximum: "ok",
    mean: null,
    emptyCount: 0,
    minimumLength: 2,
    maximumLength: 2,
    averageLength: 2,
    suggestedType: null,
    typeMatchPercentage: null,
    invalidTypeCount: null,
    sentinelCount: null,
    privacySignal: null,
    standardDeviation: null,
    firstQuartile: null,
    median: null,
    thirdQuartile: null,
    outlierCount: null,
  }],
};

describe("PreparePhase", () => {
  it("conserva tabs ARIA y selección explícita para normalizar texto", () => {
    const onNormalizeText = vi.fn();
    render(<PreparePhase
      dataset={dataset}
      profileStatus={{ kind: "idle" }}
      changeStatus={{ kind: "idle" }}
      historyStatus={EMPTY_HISTORY}
      recipeDraft={null}
      recipeSession={0}
      onAnalyzeQuality={() => undefined}
      onCancelProfile={() => undefined}
      onRemoveDuplicates={() => undefined}
      onRemoveEmptyRows={() => undefined}
      onRemoveConstantColumns={() => undefined}
    onRemoveEmptyColumns={() => undefined}
      onRemoveHighNullColumns={() => undefined}
      onNormalizeSentinels={() => undefined}
      onNormalizeBooleans={() => undefined}
      onEnableRowAudit={() => undefined}
      onNormalizeColumns={() => undefined}
      onApplyRecommended={() => undefined}
      onTrimText={() => undefined}
      onNormalizeText={onNormalizeText}
      onApplyTransforms={() => undefined}
      onRecipeDraftChange={() => undefined}
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
      recipeDraft={null} recipeSession={0}
      onRemoveDuplicates={() => undefined} onRemoveConstantColumns={() => undefined} onRemoveEmptyColumns={() => undefined} onRemoveHighNullColumns={() => undefined} onNormalizeSentinels={() => undefined} onNormalizeBooleans={() => undefined} onEnableRowAudit={() => undefined} onNormalizeColumns={() => undefined}
      onRemoveEmptyRows={() => undefined}
      onApplyRecommended={() => undefined} onTrimText={() => undefined}
      onNormalizeText={() => undefined} onApplyTransforms={() => undefined}
      onRecipeDraftChange={() => undefined}
      onUndo={onUndo} onRedo={onRedo}
    />);

    fireEvent.click(screen.getByRole("button", { name: "Deshacer" }));
    expect(onUndo).toHaveBeenCalledOnce();
    expect(screen.getByRole("button", { name: "Rehacer" })).toBeDisabled();
    expect(screen.getByText(/Etapa actual: Normalizar texto/)).toBeInTheDocument();
  });

  it("expone señales agregadas de limpieza y privacidad sin mostrar celdas", () => {
    const onRemoveConstantColumns = vi.fn();
    const onRemoveEmptyColumns = vi.fn();
    const onRemoveHighNullColumns = vi.fn();
    const onNormalizeSentinels = vi.fn();
    const onNormalizeBooleans = vi.fn();
    const onEnableRowAudit = vi.fn();
    render(<PreparePhase
      dataset={dataset}
      profileStatus={{ kind: "ready", profile: cleaningSignalsProfile }}
      changeStatus={{ kind: "idle" }}
      historyStatus={EMPTY_HISTORY}
      recipeDraft={null}
      recipeSession={0}
      onAnalyzeQuality={() => undefined}
      onCancelProfile={() => undefined}
      onRemoveDuplicates={() => undefined}
      onRemoveEmptyRows={() => undefined}
      onRemoveConstantColumns={onRemoveConstantColumns}
      onRemoveEmptyColumns={onRemoveEmptyColumns}
      onRemoveHighNullColumns={onRemoveHighNullColumns}
      onNormalizeSentinels={onNormalizeSentinels}
      onNormalizeBooleans={onNormalizeBooleans}
      onEnableRowAudit={onEnableRowAudit}
      onNormalizeColumns={() => undefined}
      onApplyRecommended={() => undefined}
      onTrimText={() => undefined}
      onNormalizeText={() => undefined}
      onApplyTransforms={() => undefined}
      onRecipeDraftChange={() => undefined}
      onUndo={() => undefined}
      onRedo={() => undefined}
    />);

    expect(screen.getByRole("heading", { name: "Señales para revisar" })).toBeInTheDocument();
    expect(screen.getByRole("list")).toHaveTextContent("1 filas adicionales");
    expect(screen.getByRole("list")).toHaveTextContent("Posible dato personal: revisa el tratamiento de email");
    expect(screen.getByRole("list")).toHaveTextContent("Tipos sugeridos:");
    fireEvent.click(screen.getByRole("button", { name: "Eliminar columnas constantes" }));
    expect(onRemoveConstantColumns).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Eliminar columnas vacías" }));
    expect(onRemoveEmptyColumns).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Eliminar columnas con alta nulidad" }));
    expect(onRemoveHighNullColumns).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Convertir centinelas a nulos" }));
    expect(onNormalizeSentinels).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Normalizar booleanos" }));
    expect(onNormalizeBooleans).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Activar trazabilidad" }));
    expect(onEnableRowAudit).toHaveBeenCalledOnce();
  });
});

describe("TransformRecipeEditor", () => {
  it("inicializa un borrador restaurado y emite cambios en una sola dirección", async () => {
    const onDraftChange = vi.fn();
    const restored: LoadedRecipe = {
      version: 1,
      name: "Limpieza persistida",
      savedAt: "2026-08-21T00:00:00Z",
      recipe: { ...emptyRecipe, renames: [{ from: "nombre", to: "cliente" }] },
    };
    render(<TransformRecipeEditor
      dataset={dataset}
      busy={false}
      initialDraft={restored}
      onApply={() => undefined}
      onDraftChange={onDraftChange}
    />);

    expect(screen.getByRole("textbox", { name: "Nombre de la receta" })).toHaveValue("Limpieza persistida");
    expect(screen.getByRole("combobox", { name: "Columna para renombrar 1" })).toHaveValue("nombre");
    expect(screen.getByRole("textbox", { name: "Nuevo nombre 1" })).toHaveValue("cliente");
    expect(onDraftChange).not.toHaveBeenCalled();

    fireEvent.change(screen.getByRole("textbox", { name: "Nuevo nombre 1" }), { target: { value: "persona" } });
    await waitFor(() => expect(onDraftChange).toHaveBeenCalledWith({
      ...restored,
      recipe: { ...emptyRecipe, renames: [{ from: "nombre", to: "persona" }] },
    }));
    expect(onDraftChange).toHaveBeenCalledTimes(1);
  });

  it("conserva el último borrador válido durante una edición transitoria inválida", async () => {
    const onDraftChange = vi.fn();
    const restored: LoadedRecipe = {
      version: 1,
      name: "Limpieza persistida",
      savedAt: "2026-08-21T00:00:00Z",
      recipe: { ...emptyRecipe, renames: [{ from: "nombre", to: "cliente" }] },
    };
    render(<TransformRecipeEditor
      dataset={dataset}
      busy={false}
      initialDraft={restored}
      onApply={() => undefined}
      onDraftChange={onDraftChange}
    />);

    fireEvent.change(screen.getByRole("textbox", { name: "Nuevo nombre 1" }), {
      target: { value: "" },
    });
    await waitFor(() => expect(screen.getByRole("textbox", { name: "Nuevo nombre 1" })).toHaveValue(""));
    expect(onDraftChange).not.toHaveBeenCalled();

    fireEvent.change(screen.getByRole("textbox", { name: "Nuevo nombre 1" }), {
      target: { value: "persona" },
    });
    await waitFor(() => expect(onDraftChange).toHaveBeenCalledWith({
      ...restored,
      recipe: { ...emptyRecipe, renames: [{ from: "nombre", to: "persona" }] },
    }));
    expect(onDraftChange).toHaveBeenCalledTimes(1);
  });

  it("guarda el borrador validado con el nombre visible", async () => {
    const saved: LoadedRecipe = {
      version: 1,
      name: "Mi receta",
      savedAt: "2026-08-21T00:00:00Z",
      recipe: { ...emptyRecipe, renames: [{ from: "nombre", to: "cliente" }] },
    };
    const save = vi.spyOn(bridge, "saveTransformRecipe").mockResolvedValue(saved);
    render(<TransformRecipeEditor dataset={dataset} busy={false} initialDraft={null} onApply={() => undefined} onDraftChange={() => undefined} />);
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
    render(<TransformRecipeEditor dataset={dataset} busy={false} initialDraft={null} onApply={onApply} onDraftChange={() => undefined} />);
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
    render(<TransformRecipeEditor dataset={dataset} busy={false} initialDraft={null} onApply={() => undefined} onDraftChange={() => undefined} />);
    fireEvent.change(screen.getByLabelText("Columna para renombrar 1"), { target: { value: "nombre" } });
    fireEvent.change(screen.getByLabelText("Nuevo nombre 1"), { target: { value: "borrador" } });
    fireEvent.click(screen.getByRole("button", { name: "Cargar receta" }));

    await waitFor(() => expect(confirm).toHaveBeenCalledOnce());
    expect(screen.getByLabelText("Nuevo nombre 1")).toHaveValue("borrador");
    expect(screen.queryByText(/Receta cargada:/)).not.toBeInTheDocument();
  });
});
