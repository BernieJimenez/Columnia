import { Channel, invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  cancelOperation,
  applySafeCorrections,
  applyTransformRecipe,
  exportDataset,
  getAppInfo,
  getDatasetPage,
  getDatasetProfile,
  getHistoryState,
  normalizeColumnNames,
  normalizeTextValues,
  discardDatasetSelection,
  loadDatasetSelection,
  pickDatasetSource,
  pickTransformRecipe,
  removeDuplicates,
  redoLastChange,
  saveTransformRecipe,
  trimTextValues,
  undoLastChange,
  validateQualityRules,
  type QualityRule,
  type TransformRecipe,
} from "./bridge";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  Channel: class<T> {
    onmessage: (message: T) => void;

    constructor(onmessage: (message: T) => void) {
      this.onmessage = onmessage;
    }
  },
}));

describe("desktop bridge", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());

  it("invoca el comando Rust con un contrato estrecho", async () => {
    vi.mocked(invoke).mockResolvedValue({
      name: "Columnia",
      version: "0.1.0",
      platform: "windows",
    });

    await expect(getAppInfo()).resolves.toEqual({
      name: "Columnia",
      version: "0.1.0",
      platform: "windows",
    });
    expect(invoke).toHaveBeenCalledWith("get_app_info");
  });

  it("solicita la selección nativa sin entregar una ruta desde React", async () => {
    vi.mocked(invoke).mockResolvedValue(null);

    await expect(pickDatasetSource()).resolves.toBeNull();

    expect(invoke).toHaveBeenCalledWith("pick_dataset_source");
  });

  it("carga una selección opaca y permite descartarla sin entregar rutas", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ fileName: "libro.xlsx" }).mockResolvedValueOnce(undefined);
    const onProgress = vi.fn();
    await loadDatasetSelection("selection-1", "2", "generated", onProgress);

    expect(invoke).toHaveBeenCalledWith("load_dataset_selection", {
      selectionId: "selection-1",
      sheetId: "2",
      headerMode: "generated",
      onProgress: expect.any(Channel),
    });
    const args = vi.mocked(invoke).mock.calls[0][1] as {
      onProgress: Channel<{
        operation: "load";
        stage: string;
        percent: number;
      }>;
    };
    const channel = args.onProgress;
    channel.onmessage({ operation: "load", stage: "Validando archivo", percent: 10 });
    expect(onProgress).toHaveBeenCalledWith({
      operation: "load",
      stage: "Validando archivo",
      percent: 10,
    });
    await discardDatasetSelection("selection-1");
    expect(invoke).toHaveBeenLastCalledWith("discard_dataset_selection", {
      selectionId: "selection-1",
    });
  });

  it("solicita una página por posición sin volver a entregar la ruta", async () => {
    vi.mocked(invoke).mockResolvedValue({ offset: 50, rows: [["Santiago"]] });

    await expect(getDatasetPage(50, 50)).resolves.toEqual({
      offset: 50,
      rows: [["Santiago"]],
    });

    expect(invoke).toHaveBeenCalledWith("get_dataset_page", { offset: 50, limit: 50 });
  });

  it("solicita el perfil del dataset activo sin argumentos", async () => {
    vi.mocked(invoke).mockResolvedValue({
      rowCount: 0,
      duplicateRowCount: 0,
      duplicatePercentage: 0,
      columns: [],
    });

    await expect(getDatasetProfile()).resolves.toEqual({
      rowCount: 0,
      duplicateRowCount: 0,
      duplicatePercentage: 0,
      columns: [],
    });

    expect(invoke).toHaveBeenCalledWith("get_dataset_profile", {
      onProgress: expect.any(Channel),
    });
  });

  it("aplica y deshace transformaciones mediante comandos sin argumentos", async () => {
    vi.mocked(invoke).mockResolvedValue({});

    await removeDuplicates();
    await normalizeColumnNames();
    await trimTextValues();
    await normalizeTextValues(["city"], true);
    await applySafeCorrections();
    await undoLastChange();
    await redoLastChange();

    expect(invoke).toHaveBeenNthCalledWith(1, "remove_duplicates");
    expect(invoke).toHaveBeenNthCalledWith(2, "normalize_column_names");
    expect(invoke).toHaveBeenNthCalledWith(3, "trim_text_values");
    expect(invoke).toHaveBeenNthCalledWith(4, "normalize_text_values", {
      columns: ["city"],
      removeAccents: true,
    });
    expect(invoke).toHaveBeenNthCalledWith(5, "apply_safe_corrections");
    expect(invoke).toHaveBeenNthCalledWith(6, "undo_last_change");
    expect(invoke).toHaveBeenNthCalledWith(7, "redo_last_change");
  });

  it("envía una receta estructural completa en una sola invocación", async () => {
    vi.mocked(invoke).mockResolvedValue({
      dataset: {},
      renamedColumnCount: 1,
      convertedColumnCount: 1,
      parsedDateColumnCount: 1,
    });
    const recipe: TransformRecipe = {
      renames: [{ from: "Total venta", to: "total" }],
      casts: [{ column: "total", target: "decimal" }],
      dateParses: [{ column: "fecha", format: "dmy", target: "date" }],
      filters: [{ column: "total", operator: "gte", value: "10" }],
      calculatedColumn: {
        name: "total_doble",
        source: "total",
        operation: "multiply",
        operand: { kind: "literal", value: "2" },
      },
      findReplace: {
        scope: "column",
        column: "estado",
        find: "pendiente",
        replace: "",
      },
      keepColumns: ["total", "estado"],
      splitColumn: { source: "estado", delimiter: "-", names: ["estado", "detalle"], dropSource: true },
      mergeColumns: { sources: ["nombre", "apellido"], name: "nombre_completo", separator: " ", dropSources: false },
      outlierTreatments: [{ column: "total", action: "cap" }],
      groupSummary: { groupBy: ["estado"], aggregations: [{ column: "total", operation: "sum" }] },
      contactNormalizations: [{ column: "correo", kind: "email" }],
      textExtractions: [{ source: "nombre", kind: "first_token", name: "primer_nombre", delimiter: null }],
    };

    await applyTransformRecipe(recipe);

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("apply_transform_recipe", { recipe });
  });

  it("guarda y carga recetas mediante selectores nativos sin exponer rutas", async () => {
    const recipe: TransformRecipe = {
      renames: [{ from: "Total venta", to: "total" }],
      casts: [{ column: "total", target: "decimal" }],
      dateParses: [{ column: "fecha", format: "dmy", target: "date" }],
      filters: [{ column: "total", operator: "gte", value: "10" }],
      calculatedColumn: { name: "doble", source: "total", operation: "multiply", operand: { kind: "literal", value: "2" } },
      findReplace: { scope: "column", column: "estado", find: "P", replace: "Pendiente" },
      keepColumns: ["total", "fecha", "estado"],
      splitColumn: { source: "estado", delimiter: "-", names: ["estado", "detalle"], dropSource: false },
      mergeColumns: { sources: ["estado", "detalle"], name: "estado_detalle", separator: " ", dropSources: false },
      outlierTreatments: [{ column: "total", action: "cap" }],
      groupSummary: { groupBy: ["estado"], aggregations: [{ column: "total", operation: "sum" }] },
      contactNormalizations: [{ column: "correo", kind: "email" }],
      textExtractions: [{ source: "estado", kind: "first_token", name: "estado_corto", delimiter: null }],
    };
    const stored = { version: 1 as const, name: "Ventas", savedAt: "2026-08-14T12:00:00Z", recipe };
    vi.mocked(invoke).mockResolvedValueOnce(stored).mockResolvedValueOnce(stored);

    await expect(saveTransformRecipe(recipe, "Ventas")).resolves.toEqual(stored);
    expect(invoke).toHaveBeenNthCalledWith(1, "save_transform_recipe", { recipe, name: "Ventas" });
    await expect(pickTransformRecipe()).resolves.toEqual(stored);
    expect(invoke).toHaveBeenNthCalledWith(2, "pick_transform_recipe");
    expect(vi.mocked(invoke).mock.calls.flatMap((call) => Object.keys((call[1] ?? {}) as object))).not.toContain("path");
  });

  it("cancela únicamente la operación indicada", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    await cancelOperation("profile");

    expect(invoke).toHaveBeenCalledWith("cancel_operation", { operation: "profile" });
  });

  it("consulta el historial real mantenido por Rust", async () => {
    vi.mocked(invoke).mockResolvedValue({ canUndo: true, canRedo: false, currentIndex: 1 });
    await getHistoryState();
    expect(invoke).toHaveBeenCalledWith("get_history_state");
  });

  it("exporta mediante selector nativo sin recibir una ruta de React", async () => {
    vi.mocked(invoke).mockResolvedValue({
      fileName: "datos-columnia.parquet",
      fileSizeBytes: 512,
      format: "Parquet",
    });

    const qualityRules: QualityRule[] = [{ column: "total", kind: "not_null", maxInvalid: 0 }];
    await expect(exportDataset("parquet", qualityRules, false)).resolves.toEqual({
      fileName: "datos-columnia.parquet",
      fileSizeBytes: 512,
      format: "Parquet",
    });
    expect(invoke).toHaveBeenCalledWith("export_dataset", {
      format: "parquet",
      qualityRules,
      allowUnvalidated: false,
      onProgress: expect.any(Channel),
    });
  });

  it("valida reglas sin enviar muestras ni valores al backend", async () => {
    const qualityRules: QualityRule[] = [
      { column: "total", kind: "numeric_range", maxInvalidPct: 1.5, min: 0, max: 1000 },
    ];
    vi.mocked(invoke).mockResolvedValue({
      passed: true, rowCount: 20, totalRules: 1, failedRules: 0,
      rules: [{ ...qualityRules[0], checkedCount: 20, invalidCount: 0, invalidPct: 0, passed: true }],
    });

    await validateQualityRules(qualityRules);

    expect(invoke).toHaveBeenCalledWith("validate_quality_rules", { qualityRules });
    expect(JSON.stringify(vi.mocked(invoke).mock.calls[0][1])).not.toContain("rows");
    expect(JSON.stringify(vi.mocked(invoke).mock.calls[0][1])).not.toContain("path");
  });
});
