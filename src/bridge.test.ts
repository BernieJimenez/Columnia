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
  exportDataset,
  getAppInfo,
  getDatasetConflictPage,
  getDatasetPage,
  getDatasetProfile,
  getHistoryState,
  enableRowAudit,
  imputeMissingValues,
  normalizeColumnNames,
  normalizeSentinelValues,
  normalizeBooleanValues,
  normalizeTextValues,
  openProject,
  importDataprepSessionProject,
  discardDatasetSelection,
  loadDatasetSelection,
  pickDatasetSource,
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
  saveQualityRulesDocument,
  saveTransformRecipe,
  trimTextValues,
  useConsolidatedDataset,
  undoLastChange,
  validateQualityRules,
  type QualityRule,
  type ProjectWorkspace,
  type RecipeExportOptions,
  type RecipeMigrationReport,
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

    await resolveDatasetConflicts([{ conflictIndex: 0, source: "compared" }]);

    expect(invoke).toHaveBeenCalledWith("resolve_dataset_conflicts", {
      decisions: [{ conflictIndex: 0, source: "compared" }],
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
    });
  });

  it("aplica y deshace transformaciones mediante comandos sin argumentos", async () => {
    vi.mocked(invoke).mockResolvedValue({});

    await removeDuplicates();
    await normalizeColumnNames();
    await trimTextValues();
    await normalizeTextValues(["city"], true);
    await normalizeSentinelValues();
    await normalizeBooleanValues();
    await imputeMissingValues();
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
    expect(invoke).toHaveBeenNthCalledWith(5, "normalize_sentinel_values");
    expect(invoke).toHaveBeenNthCalledWith(6, "normalize_boolean_values");
    expect(invoke).toHaveBeenNthCalledWith(7, "impute_missing_values");
    expect(invoke).toHaveBeenNthCalledWith(8, "apply_safe_corrections");
    expect(invoke).toHaveBeenNthCalledWith(9, "undo_last_change");
    expect(invoke).toHaveBeenNthCalledWith(10, "redo_last_change");
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
    expect(invoke).toHaveBeenNthCalledWith(1, "save_transform_recipe", {
      recipe,
      name: "Ventas",
      migrationReport: null,
      exportOptions: null,
    });
    await expect(pickTransformRecipe()).resolves.toEqual(stored);
    expect(invoke).toHaveBeenNthCalledWith(2, "pick_transform_recipe");
    expect(vi.mocked(invoke).mock.calls.flatMap((call) => Object.keys((call[1] ?? {}) as object))).not.toContain("path");
  });

  it("conserva metadatos de migración al guardar una receta importada", async () => {
    const recipe: TransformRecipe = {
      renames: [{ from: "nombre", to: "cliente" }],
      casts: [],
      dateParses: [],
      filters: [],
      calculatedColumn: null,
      findReplace: null,
      keepColumns: null,
      splitColumn: null,
      mergeColumns: null,
      outlierTreatments: [],
      groupSummary: null,
      contactNormalizations: [],
      textExtractions: [],
    };
    const migrationReport: RecipeMigrationReport = {
      artifactSha256: "a".repeat(64),
      sourceFormat: "dataprep",
      sourceVersion: 3,
      convertedItems: 2,
      omittedItems: 1,
      warningCount: 1,
      convertedOperations: ["renames"],
      omittedOperations: ["export.report_format"],
      warnings: [{ path: "export.report_format", severity: "omitted", message: "Revisión manual." }],
      manualActions: ["Validar antes de exportar."],
    };
    const exportOptions: RecipeExportOptions = { formats: ["csv", "excel"], selectedColumns: ["cliente"], privacyMode: "mask" };
    vi.mocked(invoke).mockResolvedValue(null);

    await saveTransformRecipe(recipe, "Pipeline", migrationReport, exportOptions);

    expect(invoke).toHaveBeenCalledWith("save_transform_recipe", {
      recipe,
      name: "Pipeline",
      migrationReport,
      exportOptions,
    });
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

  it("importa sesiones DataPrep mediante selector nativo sin argumentos de ruta", async () => {
    vi.mocked(invoke).mockResolvedValue({ id: "project-2", name: "Sesión" });

    await expect(importDataprepSessionProject()).resolves.toEqual({ id: "project-2", name: "Sesión" });

    expect(invoke).toHaveBeenCalledWith("import_dataprep_session_project", {
      name: null,
      sheetName: null,
      headerMode: null,
    });
    expect(JSON.stringify(vi.mocked(invoke).mock.calls[0][1])).not.toContain("sessionPath");
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

  it("importa reglas DataPrep mediante un selector nativo sin exponer rutas", async () => {
    vi.mocked(invoke).mockResolvedValue({
      sourceFormat: "dataprep",
      sourceVersion: "3",
      convertedRules: [{ column: "status", kind: "not_null", maxInvalid: 0 }],
      warnings: [],
      omittedRules: 0,
      report: {
        artifactSha256: "a".repeat(64),
        totalItems: 1,
        convertedItems: 1,
        omittedItems: 0,
        warningCount: 0,
        manualActions: ["Validar el contrato convertido antes de exportar."],
      },
    });

    await expect(pickQualityRulesMigration()).resolves.toMatchObject({
      sourceVersion: "3",
      omittedRules: 0,
      report: {
        artifactSha256: "a".repeat(64),
        totalItems: 1,
        convertedItems: 1,
      },
    });

    expect(invoke).toHaveBeenCalledWith("pick_quality_rules_migration");
    expect(JSON.stringify(vi.mocked(invoke).mock.calls[0][1] ?? {})).not.toContain("path");
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
});
