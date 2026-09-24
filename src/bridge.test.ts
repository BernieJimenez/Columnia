import { Channel, invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  cancelOperation,
  clearDatasetComparison,
  compareDataset,
  resolveDatasetConflicts,
  joinDataset,
  applySafeCorrections,
  applyTransformRecipe,
  capOutlierValues,
  cancelUpdateDownload,
  checkForUpdate,
  exportDataset,
  exportDatasetToDatabase,
  downloadUpdate,
  getAppInfo,
  getPerformanceSettings,
  getResourceUsage,
  getDatasetConflictPage,
  getDatasetPage,
  getDatasetProfile,
  getTemporalAggregation,
  getHistoryState,
  enableRowAudit,
  imputeMissingValues,
  imputeOutlierValues,
  normalizeColumnNames,
  normalizeSentinelValues,
  normalizeBooleanValues,
  fixEncodingValues,
  parseDateValues,
  castNumericValues,
  imputeCategoricalValues,
  maskPersonalValues,
  nullifyInvalidTypeValues,
  normalizeTextValues,
  openLastExport,
  openProject,
  listProjectVersions,
  autosaveProject,
  restoreProjectVersion,
  listReusableTasks,
  saveReusableTask,
  openReusableTask,
  deleteReusableTask,
  checkReusableTaskSchema,
  preflightDatabaseExport,
  listDeliveryPresets,
  openDeliveryPreset,
  saveDeliveryPreset,
  deleteDeliveryPreset,
  discardDatasetSelection,
  dropOutlierValues,
  inspectDroppedDataset,
  installUpdate,
  loadDatasetSelection,
  pickDatasetSource,
  previewDelimitedHeaderReview,
  pickQualityRulesMigration,
  pickTransformRecipe,
  queryDataset,
  removeDuplicates,
  removeNearDuplicates,
  removeConstantColumns,
  removeEmptyColumns,
  removeHighNullColumns,
  removeIdentifierColumns,
  redoLastChange,
  saveProject,
  saveDiagnosticReport,
  saveQualityRulesDocument,
  saveTransformRecipe,
  setPerformanceProfile,
  trimTextValues,
  testDatabaseConnection,
  useConsolidatedDataset,
  undoLastChange,
  validateQualityRules,
  type QualityRule,
  type ProjectWorkspace,
  type OperationProgress,
  type RecipeExportOptions,
  type TransformRecipe,
  type DatabaseTarget,
  type ReusableTask,
  type DeliveryPreset,
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

  it("conecta las acciones del updater al IPC y reenvía su canal de progreso", async () => {
    const update = {
      currentVersion: "0.57.0",
      version: "0.58.0",
      notes: "Correcciones de estabilidad",
      date: null,
      sizeBytes: 2048,
    };
    vi.mocked(invoke)
      .mockResolvedValueOnce(update)
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce(undefined);
    const onProgress = vi.fn();

    await expect(checkForUpdate()).resolves.toEqual(update);
    await downloadUpdate(onProgress);
    await cancelUpdateDownload();
    await installUpdate();

    expect(invoke).toHaveBeenNthCalledWith(1, "check_for_update");
    expect(invoke).toHaveBeenNthCalledWith(2, "download_update", {
      onProgress: expect.any(Channel),
    });
    const args = vi.mocked(invoke).mock.calls[1][1] as {
      onProgress: Channel<{
        phase: "started" | "progress" | "finished" | "cancelled";
        downloadedBytes: number;
        contentLength: number | null;
      }>;
    };
    const progress = { phase: "progress", downloadedBytes: 1024, contentLength: 2048 } as const;
    args.onProgress.onmessage(progress);
    expect(onProgress).toHaveBeenCalledWith(progress);
    expect(invoke).toHaveBeenNthCalledWith(3, "cancel_update_download");
    expect(invoke).toHaveBeenNthCalledWith(4, "install_update");
  });

  it("envía el contrato tipado solo al comando local de guardado diagnóstico", async () => {
    const report: import("./bridge/diagnostics-contracts").DiagnosticReport = {
      contract: "columnia-diagnostic-report",
      schemaVersion: 1,
      appVersion: "1.25.0",
      phase: "prepare",
      status: "issue_reported",
      errorCodes: ["TRANSFORM_APPLY_FAILED"],
      metrics: null,
    };
    vi.mocked(invoke).mockResolvedValueOnce(undefined);

    await expect(saveDiagnosticReport(report)).resolves.toBeUndefined();

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("save_diagnostic_report", { report });
  });

  it("conecta el monitor de recursos y el perfil de rendimiento al IPC", async () => {
    const usage = {
      processCpuPercentage: 3.5,
      systemCpuPercentage: 18,
      logicalCpuCount: 8,
      processMemoryBytes: 1024,
      systemMemoryUsedBytes: 4096,
      systemMemoryTotalBytes: 8192,
      systemMemoryAvailableBytes: 4096,
      gpu: {
        status: "unavailable",
        usagePercentage: null,
        memoryUsedBytes: null,
        memoryTotalBytes: null,
        reason: "No hay una sonda disponible.",
      },
    };
    const settings = {
      requestedProfile: "maximum",
      activeProfile: "maximum",
      requestedThreads: 8,
      activeThreads: 8,
      applied: true,
      locked: false,
      reason: null,
    };
    vi.mocked(invoke)
      .mockResolvedValueOnce(usage)
      .mockResolvedValueOnce(settings)
      .mockResolvedValueOnce(settings);

    await expect(getResourceUsage()).resolves.toEqual(usage);
    await expect(getPerformanceSettings()).resolves.toEqual(settings);
    await expect(setPerformanceProfile("maximum")).resolves.toEqual(settings);

    expect(invoke).toHaveBeenNthCalledWith(1, "get_resource_usage");
    expect(invoke).toHaveBeenNthCalledWith(2, "get_performance_settings");
    expect(invoke).toHaveBeenNthCalledWith(3, "set_performance_profile", { profile: "maximum" });
  });

  it("solicita la selección nativa sin entregar una ruta desde React", async () => {
    vi.mocked(invoke).mockResolvedValue(null);

    await expect(pickDatasetSource()).resolves.toBeNull();

    expect(invoke).toHaveBeenCalledWith("pick_dataset_source");
  });

  it("inspecciona un archivo arrastrado sin transportar su ruta al frontend", async () => {
    vi.mocked(invoke).mockResolvedValue({
      selectionId: "opaque-selection",
      fileName: "ventas.csv",
      fileSizeBytes: 42,
      format: "csv",
      sheets: [],
      isCompressedContainer: false,
    });

    await expect(inspectDroppedDataset()).resolves.toMatchObject({
      selectionId: "opaque-selection",
      fileName: "ventas.csv",
    });
    expect(invoke).toHaveBeenCalledWith("inspect_dropped_dataset");
  });

  it("carga una selección opaca y permite descartarla sin entregar rutas", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ fileName: "libro.xlsx" }).mockResolvedValueOnce(undefined);
    const onProgress = vi.fn();
    await loadDatasetSelection("selection-1", "2", "generated", onProgress);

    expect(invoke).toHaveBeenCalledWith("load_dataset_selection", {
      selectionId: "selection-1",
      sheetId: "2",
      headerMode: "generated",
      expectedProfile: null,
      dateConvention: null,
      numberConvention: null,
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

  it("compara y consolida datasets mediante comandos opacos", async () => {
    vi.mocked(invoke).mockResolvedValue({
      currentFileName: "a.csv",
      comparedFileName: "b.csv",
      currentRowCount: 2,
      comparedRowCount: 2,
      commonRowCount: 1,
      currentOnlyRowCount: 1,
      comparedOnlyRowCount: 1,
      sharedColumns: ["id"],
      currentOnlyColumns: [],
      comparedOnlyColumns: [],
      schemaCompatible: true,
      keyColumns: ["id"],
      matchedKeyCount: 1,
      currentOnlyKeyCount: 0,
      comparedOnlyKeyCount: 0,
      conflictingKeyCount: 0,
      duplicateKeyCount: 0,
      conflicts: [],
      conflictOffset: 0,
      conflictsTruncated: false,
      canConsolidate: true,
    });

    await compareDataset(["id"]);
    await joinDataset(["id"], "left");
    await useConsolidatedDataset();
    await clearDatasetComparison();

    expect(invoke).toHaveBeenNthCalledWith(1, "compare_dataset", { keyColumns: ["id"] });
    expect(invoke).toHaveBeenNthCalledWith(2, "join_dataset", {
      keyColumns: ["id"],
      joinType: "left",
    });
    expect(invoke).toHaveBeenNthCalledWith(3, "use_consolidated_dataset");
    expect(invoke).toHaveBeenNthCalledWith(4, "clear_dataset_comparison");
  });

  it("solicita páginas de conflictos por offset y límite acotados", async () => {
    vi.mocked(invoke).mockResolvedValue({ offset: 50, conflicts: [], hasNext: true });

    await expect(getDatasetConflictPage(50, 50)).resolves.toEqual({
      offset: 50,
      conflicts: [],
      hasNext: true,
    });

    expect(invoke).toHaveBeenCalledWith("get_dataset_conflict_page", { offset: 50, limit: 50 });
  });

  it("resuelve conflictos por clave con decisiones serializables", async () => {
    vi.mocked(invoke).mockResolvedValue({ fileName: "Resuelto · datos.csv" });

    await resolveDatasetConflicts([{ action: "useSource", conflictIndex: 0, source: "compared" }]);

    expect(invoke).toHaveBeenCalledWith("resolve_dataset_conflicts", {
      decisions: [{ action: "useSource", conflictIndex: 0, source: "compared" }],
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

  it("ejecuta consultas locales sin enviar rutas ni datos adicionales", async () => {
    vi.mocked(invoke).mockResolvedValue({
      columns: [{ name: "city", dataType: "String" }],
      rowCount: 3,
      offset: 1,
      rows: [["Santiago"]],
      truncated: true,
    });

    await expect(queryDataset("SELECT city FROM dataset LIMIT 1 OFFSET 1")).resolves.toMatchObject({
      rowCount: 3,
      truncated: true,
    });
    expect(invoke).toHaveBeenCalledWith("query_dataset", {
      query: "SELECT city FROM dataset LIMIT 1 OFFSET 1",
      engine: "polars",
    });
  });

  it("solicita el perfil del dataset activo sin argumentos", async () => {
    vi.mocked(invoke).mockResolvedValue({
      rowCount: 0,
      duplicateRowCount: 0,
      nearDuplicateRowCount: 0,
      duplicatePercentage: 0,
      columns: [],
    });

    await expect(getDatasetProfile()).resolves.toEqual({
      rowCount: 0,
      duplicateRowCount: 0,
      nearDuplicateRowCount: 0,
      duplicatePercentage: 0,
      columns: [],
    });

    expect(invoke).toHaveBeenCalledWith("get_dataset_profile", {
      onProgress: expect.any(Channel),
      correlationSampleRows: undefined,
    });
  });

  it("envía el límite opcional de filas para correlaciones", async () => {
    vi.mocked(invoke).mockResolvedValue({});

    await getDatasetProfile(undefined, 10_000);

    expect(invoke).toHaveBeenCalledWith("get_dataset_profile", {
      onProgress: expect.any(Channel),
      correlationSampleRows: 10_000,
    });
  });

  it("aplica y deshace transformaciones mediante comandos con sus opciones", async () => {
    vi.mocked(invoke).mockResolvedValue({});

    await removeDuplicates();
    await normalizeColumnNames();
    await trimTextValues();
    await normalizeTextValues(["city"], true);
    await parseDateValues();
    await castNumericValues();
    await normalizeSentinelValues();
    await normalizeBooleanValues();
    await fixEncodingValues();
    await nullifyInvalidTypeValues();
    await imputeMissingValues();
    await applySafeCorrections({ trimText: true, normalizeSentinels: true, normalizeColumnNames: false, removeDuplicates: true });
    await undoLastChange();
    await redoLastChange();

    expect(invoke).toHaveBeenNthCalledWith(1, "remove_duplicates");
    expect(invoke).toHaveBeenNthCalledWith(2, "normalize_column_names");
    expect(invoke).toHaveBeenNthCalledWith(3, "trim_text_values");
    expect(invoke).toHaveBeenNthCalledWith(4, "normalize_text_values", {
      columns: ["city"],
      removeAccents: true,
    });
    expect(invoke).toHaveBeenNthCalledWith(5, "parse_date_values");
    expect(invoke).toHaveBeenNthCalledWith(6, "cast_numeric_values");
    expect(invoke).toHaveBeenNthCalledWith(7, "normalize_sentinel_values");
    expect(invoke).toHaveBeenNthCalledWith(8, "normalize_boolean_values");
    expect(invoke).toHaveBeenNthCalledWith(9, "fix_encoding_values");
    expect(invoke).toHaveBeenNthCalledWith(10, "nullify_invalid_type_values");
    expect(invoke).toHaveBeenNthCalledWith(11, "impute_missing_values");
    expect(invoke).toHaveBeenNthCalledWith(12, "apply_safe_corrections", {
      trimText: true,
      normalizeSentinels: true,
      normalizeColumnNames: false,
      removeDuplicates: true,
      imputeMissing: false,
    });
    expect(invoke).toHaveBeenNthCalledWith(13, "undo_last_change");
    expect(invoke).toHaveBeenNthCalledWith(14, "redo_last_change");
  });

  it("invoca la eliminación de duplicados parecidos sin enviar valores", async () => {
    vi.mocked(invoke).mockResolvedValue({ dataset: {}, affectedRowCount: 2 });

    await expect(removeNearDuplicates()).resolves.toEqual({ dataset: {}, affectedRowCount: 2 });

    expect(invoke).toHaveBeenCalledWith("remove_near_duplicates");
  });

  it("activa la columna reservada de trazabilidad sin enviar rutas", async () => {
    vi.mocked(invoke).mockResolvedValue({ dataset: {}, affectedRowCount: 0 });

    await enableRowAudit();

    expect(invoke).toHaveBeenCalledWith("enable_row_audit");
  });

  it("imputa outliers mediante un comando tipado", async () => {
    vi.mocked(invoke).mockResolvedValue({
      dataset: {},
      affectedRowCount: 1,
      changedCellCount: 1,
      changedColumns: [{ name: "amount", changedCellCount: 1 }],
    });

    await imputeOutlierValues();

    expect(invoke).toHaveBeenCalledWith("impute_outlier_values");
  });

  it("limita outliers mediante un comando tipado", async () => {
    vi.mocked(invoke).mockResolvedValue({
      dataset: {},
      affectedRowCount: 1,
      changedCellCount: 1,
      changedColumns: [{ name: "amount", changedCellCount: 1 }],
    });

    await capOutlierValues();

    expect(invoke).toHaveBeenCalledWith("cap_outlier_values");
  });

  it("elimina filas atípicas mediante un comando tipado", async () => {
    vi.mocked(invoke).mockResolvedValue({
      dataset: {},
      affectedRowCount: 1,
      changedCellCount: 1,
      changedColumns: [{ name: "amount", changedCellCount: 1 }],
    });

    await dropOutlierValues();

    expect(invoke).toHaveBeenCalledWith("drop_outlier_values");
  });

  it("imputa categorías mediante un comando tipado", async () => {
    vi.mocked(invoke).mockResolvedValue({
      dataset: {},
      affectedRowCount: 1,
      changedCellCount: 1,
      changedColumns: [{ name: "status", changedCellCount: 1 }],
    });

    await imputeCategoricalValues();

    expect(invoke).toHaveBeenCalledWith("impute_categorical_values");
  });

  it("protege valores personales mediante un comando tipado", async () => {
    vi.mocked(invoke).mockResolvedValue({
      dataset: {},
      changedCellCount: 3,
      changedColumnCount: 2,
    });

    await expect(maskPersonalValues()).resolves.toEqual({
      dataset: {},
      changedCellCount: 3,
      changedColumnCount: 2,
    });

    expect(invoke).toHaveBeenCalledWith("mask_personal_values");
  });

  it("elimina columnas constantes mediante un comando tipado", async () => {
    vi.mocked(invoke).mockResolvedValue({
      dataset: { fileName: "datos.csv" },
      removedColumnCount: 1,
      removedColumns: ["pais"],
    });

    await removeConstantColumns();

    expect(invoke).toHaveBeenCalledWith("remove_constant_columns");
  });

  it("elimina columnas completamente vacías mediante un comando tipado", async () => {
    vi.mocked(invoke).mockResolvedValue({
      dataset: { fileName: "datos.csv" },
      removedColumnCount: 1,
      removedColumns: ["notas"],
    });

    await removeEmptyColumns();

    expect(invoke).toHaveBeenCalledWith("remove_empty_columns");
  });

  it("elimina columnas con alta nulidad mediante un comando tipado", async () => {
    vi.mocked(invoke).mockResolvedValue({
      dataset: { fileName: "datos.csv" },
      removedColumnCount: 1,
      removedColumns: ["comentarios"],
    });

    await removeHighNullColumns();

    expect(invoke).toHaveBeenCalledWith("remove_high_null_columns");
  });

  it("retira columnas identificadoras mediante un comando tipado", async () => {
    vi.mocked(invoke).mockResolvedValue({
      dataset: { fileName: "datos.csv" },
      removedColumnCount: 1,
      removedColumns: ["customer_id"],
    });

    await removeIdentifierColumns();

    expect(invoke).toHaveBeenCalledWith("remove_identifier_columns");
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
        regex: false,
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
    expect(invoke).toHaveBeenCalledWith("apply_transform_recipe", { recipe, exceptionPolicy: null });
  });

  it("transmite convenciones de importación explícitas con claves IPC camelCase", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ fileName: "ventas.csv" });
    await loadDatasetSelection("selection-2", null, "firstRow", undefined, {
      version: 1,
      format: "csv",
      headerMode: "firstRow",
      dateConvention: "dmy",
      numberConvention: "commaDecimalDotGrouping",
      schema: [{ name: "fecha", dataType: "date" }],
    }, "dmy", "commaDecimalDotGrouping");

    expect(invoke).toHaveBeenCalledWith("load_dataset_selection", {
      selectionId: "selection-2",
      sheetId: null,
      headerMode: "firstRow",
      expectedProfile: {
        version: 1,
        format: "csv",
        headerMode: "firstRow",
        dateConvention: "dmy",
        numberConvention: "commaDecimalDotGrouping",
        schema: [{ name: "fecha", dataType: "date" }],
      },
      dateConvention: "dmy",
      numberConvention: "commaDecimalDotGrouping",
      onProgress: expect.any(Channel),
    });
  });

  it("transmite la política de excepciones junto a la receta", async () => {
    vi.mocked(invoke).mockResolvedValue({ changed: false });
    const recipe = {
      renames: [], casts: [], dateParses: [], filters: [], calculatedColumn: null,
      findReplace: null, keepColumns: null, splitColumn: null, mergeColumns: null,
      outlierTreatments: [], groupSummary: null, contactNormalizations: [], textExtractions: [],
    } satisfies TransformRecipe;
    const exceptionPolicy = {
      version: 1 as const,
      baseline: "lexical" as const,
      schema: [{ name: "total", dataType: "String" }],
      conversions: [{ kind: "cast" as const, column: "total", target: "decimal" as const, onInvalid: "nullify" as const }],
    };

    await applyTransformRecipe(recipe, exceptionPolicy);

    expect(invoke).toHaveBeenCalledWith("apply_transform_recipe", { recipe, exceptionPolicy });
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

  it("guarda proyectos con el workspace tipado sin enviar rutas", async () => {
    const workspace: ProjectWorkspace = {
      qualityRules: [{ column: "total", kind: "not_null", maxInvalid: 0 }],
      recipeDraft: null,
      reviewTab: "preview",
      previewOffset: 50,
      activePhase: "prepare",
    };
    vi.mocked(invoke).mockResolvedValue({ id: "project-1", name: "Ventas" });

    await saveProject(null, "Ventas", workspace);

    expect(invoke).toHaveBeenCalledWith("save_project", { projectId: null, name: "Ventas", workspace });
    expect(JSON.stringify(vi.mocked(invoke).mock.calls[0][1])).not.toContain("path");
  });

  it("abre un proyecto con perfil durable opcional sin exponer rutas", async () => {
    const result = {
      project: { id: "project-1", name: "Ventas" },
      dataset: { fileName: "ventas.csv" },
      workspace: { qualityRules: [], recipeDraft: null },
      profile: null,
    };
    vi.mocked(invoke).mockResolvedValue(result);

    await expect(openProject("project-1")).resolves.toEqual(result);

    expect(invoke).toHaveBeenCalledWith("open_project", { projectId: "project-1" });
    expect(JSON.stringify(vi.mocked(invoke).mock.calls[0][1])).not.toContain("path");
  });

  it("lista, autoguarda y restaura versiones mediante identificadores opacos", async () => {
    const workspace: ProjectWorkspace = { qualityRules: [], recipeDraft: null };
    vi.mocked(invoke).mockResolvedValue([{ id: 3, createdAt: "2026-09-14T12:00:00Z" }]);
    await expect(listProjectVersions("project-1")).resolves.toMatchObject([{ id: 3 }]);
    expect(invoke).toHaveBeenLastCalledWith("list_project_versions", { projectId: "project-1" });

    vi.mocked(invoke).mockResolvedValue({ id: "project-1", name: "Ventas" });
    await autosaveProject("project-1", "Ventas", workspace);
    expect(invoke).toHaveBeenLastCalledWith("autosave_project", {
      projectId: "project-1",
      name: "Ventas",
      workspace,
    });

    await restoreProjectVersion("project-1", 3);
    expect(invoke).toHaveBeenLastCalledWith("restore_project_version", {
      projectId: "project-1",
      versionId: 3,
    });
  });

  it("solicita la revisión delimitada por un identificador opaco", async () => {
    vi.mocked(invoke).mockResolvedValue({
      delimiter: ";",
      firstRow: {
        headerMode: "firstRow",
        columns: [{ name: "id", dataType: "String" }],
        rows: [["1"]],
        includesFirstRow: false,
        sampleTruncated: false,
      },
      generated: {
        headerMode: "generated",
        columns: [{ name: "column_1", dataType: "String" }],
        rows: [["id"], ["1"]],
        includesFirstRow: true,
        sampleTruncated: false,
      },
    });

    await expect(previewDelimitedHeaderReview("selection-1")).resolves.toMatchObject({
      delimiter: ";",
      firstRow: { headerMode: "firstRow", includesFirstRow: false },
      generated: { headerMode: "generated", includesFirstRow: true },
    });
    expect(invoke).toHaveBeenCalledWith("preview_delimited_header_review", {
      selectionId: "selection-1",
    });
  });

  it("guarda tareas locales reutilizables y exige revisar cambios de esquema", async () => {
    const task: ReusableTask = {
      version: 1,
      name: "Cierre mensual",
      importProfile: {
        version: 1,
        format: "csv",
        schema: [{ name: "id", dataType: "Int64" }],
      },
      recipe: null,
      qualityRules: [{ column: "id", kind: "not_null", maxInvalid: 0 }],
      outputFormat: "csv",
      privacyMode: "mask",
    };
    const schema = [{ name: "id", dataType: "String" }];
    vi.mocked(invoke).mockResolvedValue({ id: "task-1", name: task.name });

    await saveReusableTask(null, task);
    expect(invoke).toHaveBeenLastCalledWith("save_reusable_task", { taskId: null, task });
    expect(JSON.stringify(vi.mocked(invoke).mock.calls[0][1])).not.toMatch(/credentials|password|overwrite|sourcePath/i);

    await checkReusableTaskSchema("task-1", schema);
    expect(invoke).toHaveBeenLastCalledWith("check_reusable_task_schema", { taskId: "task-1", schema });
    vi.mocked(invoke).mockResolvedValue([]);
    await expect(listReusableTasks()).resolves.toEqual([]);
    expect(invoke).toHaveBeenLastCalledWith("list_reusable_tasks");
    vi.mocked(invoke).mockResolvedValue(task);
    await expect(openReusableTask("task-1")).resolves.toEqual(task);
    expect(invoke).toHaveBeenLastCalledWith("open_reusable_task", { taskId: "task-1" });
    vi.mocked(invoke).mockResolvedValue(undefined);
    await expect(deleteReusableTask("task-1")).resolves.toBeUndefined();
    expect(invoke).toHaveBeenLastCalledWith("delete_reusable_task", { taskId: "task-1" });
  });

  it("exporta mediante selector nativo sin recibir una ruta de React", async () => {
    vi.mocked(invoke).mockResolvedValue({
      fileName: "datos-columnia.parquet",
      fileSizeBytes: 512,
      format: "Parquet",
      protectedColumnCount: 0,
      protectedColumns: [],
    });

    const qualityRules: QualityRule[] = [{ column: "total", kind: "not_null", maxInvalid: 0 }];
    await expect(exportDataset("parquet", qualityRules, false)).resolves.toEqual({
      fileName: "datos-columnia.parquet",
      fileSizeBytes: 512,
      format: "Parquet",
      protectedColumnCount: 0,
      protectedColumns: [],
    });
    expect(invoke).toHaveBeenCalledWith("export_dataset", {
      format: "parquet",
      qualityRules,
      allowUnvalidated: false,
      privacyMode: "none",
      recipe: null,
      onProgress: expect.any(Channel),
    });
  });

  it("conserva JSON como formato de exportación tipado", async () => {
    vi.mocked(invoke).mockResolvedValue({
      fileName: "datos-columnia.json",
      fileSizeBytes: 128,
      format: "JSON",
      protectedColumnCount: 0,
      protectedColumns: [],
    });

    await expect(exportDataset("json", [], true)).resolves.toEqual({
      fileName: "datos-columnia.json",
      fileSizeBytes: 128,
      format: "JSON",
      protectedColumnCount: 0,
      protectedColumns: [],
    });
    expect(invoke).toHaveBeenCalledWith("export_dataset", {
      format: "json",
      qualityRules: [],
      allowUnvalidated: true,
      privacyMode: "none",
      recipe: null,
      onProgress: expect.any(Channel),
    });
  });

  it("conserva SQL como formato de exportación tipado", async () => {
    vi.mocked(invoke).mockResolvedValue({
      fileName: "datos-columnia.sql",
      fileSizeBytes: 256,
      format: "SQL",
      protectedColumnCount: 0,
      protectedColumns: [],
    });

    await expect(exportDataset("sql", [], true)).resolves.toEqual({
      fileName: "datos-columnia.sql",
      fileSizeBytes: 256,
      format: "SQL",
      protectedColumnCount: 0,
      protectedColumns: [],
    });
    expect(invoke).toHaveBeenCalledWith("export_dataset", {
      format: "sql",
      qualityRules: [],
      allowUnvalidated: true,
      privacyMode: "none",
      recipe: null,
      onProgress: expect.any(Channel),
    });
  });

  it("conserva Excel y SQLite como destinos de exportación tipados", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce({ fileName: "datos.xlsx", fileSizeBytes: 512, format: "Excel", protectedColumnCount: 0, protectedColumns: [] })
      .mockResolvedValueOnce({ fileName: "datos.sqlite", fileSizeBytes: 1024, format: "SQLite", protectedColumnCount: 0, protectedColumns: [] });

    await expect(exportDataset("excel", [], true)).resolves.toMatchObject({
      fileName: "datos.xlsx",
      format: "Excel",
    });
    await expect(exportDataset("sqlite", [], true)).resolves.toMatchObject({
      fileName: "datos.sqlite",
      format: "SQLite",
    });
    expect(invoke).toHaveBeenNthCalledWith(1, "export_dataset", expect.objectContaining({ format: "excel", privacyMode: "none" }));
    expect(invoke).toHaveBeenNthCalledWith(2, "export_dataset", expect.objectContaining({ format: "sqlite", privacyMode: "none" }));
  });

  it("solicita la suma o el promedio temporal con nombres de argumentos camelCase", async () => {
    vi.mocked(invoke).mockResolvedValue({
      dateColumn: "fecha",
      valueColumn: "importe",
      aggregation: "mean",
      granularity: "month",
      periods: [],
      parsedRowCount: 0,
      unparsedRowCount: 0,
      truncated: false,
    });

    await getTemporalAggregation("fecha", "importe", "mean");

    expect(invoke).toHaveBeenCalledWith("get_temporal_aggregation", {
      dateColumn: "fecha",
      valueColumn: "importe",
      aggregation: "mean",
    });
  });

  it("envía la conexión de base de datos solo al comando explícito", async () => {
    const target: DatabaseTarget = {
      kind: "postgresql",
      connectionString: "Driver={PostgreSQL Unicode};Server=localhost;Pwd=secret",
      schema: "public",
      table: "ventas",
      tablePolicy: "create_only",
    };
    vi.mocked(invoke)
      .mockResolvedValueOnce({ kind: "postgresql", message: "Conexión ODBC verificada para PostgreSQL." })
      .mockResolvedValueOnce({ fileName: "\"public\".\"ventas\"", fileSizeBytes: 0, format: "PostgreSQL", protectedColumnCount: 0, protectedColumns: [] });

    await expect(testDatabaseConnection(target)).resolves.toMatchObject({ kind: "postgresql" });
    await expect(exportDatasetToDatabase(target, [], true)).resolves.toMatchObject({ format: "PostgreSQL" });

    expect(invoke).toHaveBeenNthCalledWith(1, "test_database_connection", { target });
    expect(invoke).toHaveBeenNthCalledWith(2, "export_dataset_to_database", expect.objectContaining({
      target,
      qualityRules: [],
      allowUnvalidated: true,
      privacyMode: "none",
      onProgress: expect.any(Channel),
    }));
  });

  it("preflight remoto usa destino y protección y devuelve el contrato de compatibilidad", async () => {
    const target: DatabaseTarget = {
      kind: "postgresql",
      connectionString: "Driver={PostgreSQL Unicode};Server=localhost;Pwd=secret",
      schema: "public",
      table: "ventas",
      tablePolicy: "append",
    };
    const report = {
      kind: "postgresql",
      schema: "public",
      table: "ventas",
      tablePolicy: "append",
      tableExists: true,
      ready: false,
      issues: [{ severity: "blocking", category: "length", column: "cliente", message: "La columna supera el límite." }],
    };
    vi.mocked(invoke).mockResolvedValue(report);

    await expect(preflightDatabaseExport(target, "mask")).resolves.toEqual(report);
    expect(invoke).toHaveBeenCalledWith("preflight_database_export", { target, privacyMode: "mask" });
  });

  it("persiste y relee presets por identificador sin incluir credenciales ni permiso replace por defecto", async () => {
    const preset: DeliveryPreset = {
      version: 1,
      name: "Entrega protegida",
      format: "postgresql",
      selectedColumns: ["cliente", "importe"],
      privacyMode: "hash",
      databaseTarget: {
        kind: "postgresql",
        schema: "public",
        table: "ventas",
        tablePolicy: "create_only",
      },
    };
    const summary = {
      id: "0123456789abcdef0123456789abcdef",
      name: preset.name,
      format: preset.format,
      updatedAt: "2026-09-14T00:00:00Z",
      selectedColumnCount: 2,
      remote: true,
    };
    vi.mocked(invoke).mockResolvedValueOnce(summary).mockResolvedValueOnce([summary])
      .mockResolvedValueOnce(preset).mockResolvedValueOnce(undefined);

    await expect(saveDeliveryPreset(null, preset)).resolves.toEqual(summary);
    await expect(listDeliveryPresets()).resolves.toEqual([summary]);
    await expect(openDeliveryPreset(summary.id)).resolves.toEqual(preset);
    await expect(deleteDeliveryPreset(summary.id)).resolves.toBeUndefined();

    expect(invoke).toHaveBeenNthCalledWith(1, "save_delivery_preset", { presetId: null, preset });
    expect(invoke).toHaveBeenNthCalledWith(2, "list_delivery_presets");
    expect(invoke).toHaveBeenNthCalledWith(3, "open_delivery_preset", { presetId: summary.id });
    expect(invoke).toHaveBeenNthCalledWith(4, "delete_delivery_preset", { presetId: summary.id });
    expect(JSON.stringify(vi.mocked(invoke).mock.calls[0][1])).not.toContain("connectionString");
    expect(JSON.stringify(vi.mocked(invoke).mock.calls[0][1])).not.toContain("replace");
  });

  it("abre el último output sin recibir rutas desde React", async () => {
    await openLastExport();

    expect(invoke).toHaveBeenCalledWith("open_last_export");
    expect(invoke).toHaveBeenCalledTimes(1);
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

  it("guarda un documento de calidad versionado sin entregar rutas", async () => {
    const qualityRules: QualityRule[] = [
      { column: "total", kind: "not_null", maxInvalid: 0 },
    ];
    vi.mocked(invoke).mockResolvedValue({
      format: "columnia-quality-rules",
      version: 1,
      rules: qualityRules,
    });

    await expect(saveQualityRulesDocument(qualityRules)).resolves.toMatchObject({
      format: "columnia-quality-rules",
      version: 1,
    });
    expect(invoke).toHaveBeenCalledWith("save_quality_rules_document", { qualityRules });
    expect(JSON.stringify(vi.mocked(invoke).mock.calls[0][1])).not.toContain("path");
  });

  it("guarda el esquema de origen dentro del contrato versionado de receta", async () => {
    const recipe: TransformRecipe = {
      renames: [], casts: [], dateParses: [], filters: [], calculatedColumn: null,
      findReplace: null, keepColumns: null, splitColumn: null, mergeColumns: null,
      outlierTreatments: [], groupSummary: null, contactNormalizations: [], textExtractions: [],
    };
    const sourceSchema = [{ name: "id", dataType: "Int64" }];

    await saveTransformRecipe(recipe, "Receta semanal", sourceSchema);

    expect(invoke).toHaveBeenCalledWith("save_transform_recipe", {
      recipe,
      name: "Receta semanal",
      sourceSchema,
      exportOptions: null,
    });
  });
});
