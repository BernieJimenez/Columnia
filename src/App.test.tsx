import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "./App";
import * as bridge from "./bridge";
import type {
  DatasetPreview,
  DatasetProfile,
  DelimitedHeaderReview,
  HistoryState,
  ProjectSummary,
  RemoteExportPreflight,
  ReusableTask,
  ReusableTaskSummary,
  SavedRecipe,
} from "./bridge";

const headerConfirmationTimers = new Set<number>();

function historyState(overrides: Partial<HistoryState> = {}): HistoryState {
  return {
    canUndo: true, canRedo: false, currentIndex: 1, entryCount: 2,
    entries: [{ id: "history-test-0", index: 0, label: "Dataset cargado", isCurrent: false }, { id: "history-test-1", index: 1, label: "Cambio", isCurrent: true }],
    snapshotsEnabled: true, degradedReason: null, maxEntries: 12, diskBytes: 100,
    diskBudgetBytes: 1024, ...overrides,
  };
}

function resourceEstimate(fileSizeBytes: number, processingPath: "inMemory" | "sourceBacked" = "inMemory") {
  return {
    processingPath,
    estimatedMaterializationRamBytes: fileSizeBytes * 4 + 256 * 1024 * 1024,
    estimatedTemporaryDiskBytes: processingPath === "sourceBacked" ? null : fileSizeBytes,
  } as const;
}

afterEach(() => {
  for (const timer of headerConfirmationTimers) window.clearInterval(timer);
  headerConfirmationTimers.clear();
  cleanup();
  vi.restoreAllMocks();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

function defaultDelimitedHeaderReview(): DelimitedHeaderReview {
  return {
    delimiter: ",",
    firstRow: {
      headerMode: "firstRow",
      columns: [{ name: "value", dataType: "String" }],
      rows: [["sample"]],
      includesFirstRow: false,
      sampleTruncated: false,
    },
    generated: {
      headerMode: "generated",
      columns: [{ name: "column_1", dataType: "String" }],
      rows: [["value"], ["sample"]],
      includesFirstRow: true,
      sampleTruncated: false,
    },
  };
}

beforeEach(() => {
  vi.spyOn(bridge, "previewDelimitedHeaderReview").mockResolvedValue(defaultDelimitedHeaderReview());
  vi.spyOn(bridge, "previewDatasetSelection").mockImplementation(
    async (_selectionId, _sheetId, _headerMode, expectedProfile) => ({
      rowCount: 1,
      columns: expectedProfile?.schema ?? [{ name: "value", dataType: "String" }],
      schemaMismatch: null,
    }),
  );
});

function renderAppWithHeaderConfirmation() {
  const view = render(<App />);
  const timer = window.setInterval(() => {
    const button = [...document.querySelectorAll<HTMLButtonElement>("button")].find((candidate) =>
      candidate.textContent?.trim() === "Revisar esquema" ||
      candidate.textContent?.trim() === "Cargar archivo",
    );
    const action = button?.textContent?.trim();
    if (!button || !action || button.disabled || button.dataset.testAutoConfirm === action) return;
    button.dataset.testAutoConfirm = action;
    act(() => fireEvent.click(button));
  }, 10);
  headerConfirmationTimers.add(timer);
  return view;
}
async function openQualityAndAnalyze() {
  await screen.findByText("Filas analizadas");
}

async function switchPhase(label: "Cargar" | "Revisar" | "Preparar" | "Entregar") {
  const stageHeading: Record<typeof label, RegExp> = {
    Cargar: /^(Selecciona un dataset|Dataset listo para continuar)$/,
    Revisar: /^Revisa antes de modificar$/,
    Preparar: /^Prepara datos consistentes$/,
    Entregar: /^Valida y crea una copia$/,
  };
  const phaseButton = await screen.findByRole("button", { name: label });
  await waitFor(() => {
    expect(phaseButton).toBeEnabled();
    if (label !== "Cargar") expect(phaseButton).not.toHaveAttribute("aria-disabled", "true");
  });
  fireEvent.click(screen.getByRole("button", { name: label }));
  await waitFor(() => expect(screen.getByRole("region", { name: `Etapa ${label}` })).toBeInTheDocument());
  await screen.findByRole("heading", { name: stageHeading[label] });
}

function mockDatasetLoad(dataset: DatasetPreview) {
  vi.spyOn(bridge, "getHistoryState").mockResolvedValue(historyState());
  if (!vi.isMockFunction(bridge.getDatasetProfile)) {
    vi.spyOn(bridge, "getDatasetProfile").mockResolvedValue({
      rowCount: dataset.rowCount,
      duplicateRowCount: 0,
      nearDuplicateRowCount: 0,
      duplicatePercentage: 0,
      columns: [],
    });
  }
  vi.spyOn(bridge, "pickDatasetSource").mockResolvedValue({
    selectionId: "selection-test",
    fileName: dataset.fileName,
    fileSizeBytes: dataset.fileSizeBytes,
    format: "csv",
    sheets: [],
    defaultSheetId: null,
    isCompressedContainer: false,
    resourceEstimate: resourceEstimate(dataset.fileSizeBytes),
  });
  return vi.spyOn(bridge, "loadDatasetSelection").mockResolvedValue(dataset);
}

function reusableTaskFixture() {
  const task: ReusableTask = {
    version: 1,
    name: "Cierre recurrente",
    importProfile: {
      version: 1,
      format: "csv",
      headerMode: "firstRow",
      dateConvention: "dmy",
      numberConvention: "commaDecimalDotGrouping",
      schema: [{ name: "id", dataType: "Int64" }],
    },
    recipe: {
      version: 1,
      name: "Renombrar id",
      savedAt: "2026-09-01T10:00:00Z",
      recipe: {
        renames: [{ from: "id", to: "id_limpio" }],
        casts: [], dateParses: [], filters: [], calculatedColumn: null,
        findReplace: null, keepColumns: null, splitColumn: null, mergeColumns: null,
        outlierTreatments: [], groupSummary: null, contactNormalizations: [], textExtractions: [],
      },
    },
    qualityRules: [{ column: "id", kind: "not_null", maxInvalid: 0 }],
    outputFormat: "json",
    privacyMode: "mask",
  };
  const summary: ReusableTaskSummary = {
    id: "task-recurring",
    name: task.name,
    createdAt: "2026-09-01T10:00:00Z",
    updatedAt: "2026-09-01T10:00:00Z",
    inputColumnCount: 1,
    hasRecipe: true,
    qualityRuleCount: 1,
    outputFormat: "json",
  };
  return { task, summary };
}

async function prepareReusableTaskBeforeImport(task: ReusableTask, summary: ReusableTaskSummary) {
  vi.spyOn(bridge, "listReusableTasks").mockResolvedValue([summary]);
  vi.spyOn(bridge, "openReusableTask").mockResolvedValue(task);
  renderAppWithHeaderConfirmation();
  fireEvent.click(await screen.findByText("Reutilizar una tarea"));
  await screen.findByLabelText("Tarea guardada");
  fireEvent.change(screen.getByLabelText("Tarea guardada"), { target: { value: summary.id } });
  await screen.findByText("Configuración que se reutilizará");
  fireEvent.click(screen.getByRole("button", { name: "Preparar próxima importación" }));
  expect(await screen.findByText(/Tarea “Cierre recurrente” preparada/)).toBeInTheDocument();
  expect(screen.getByText(/la receta quedará como borrador/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Seleccionar dataset" }));
}

describe("App", () => {
  it("usa la acción contextual de Review y marca Review como hecha al continuar explícitamente", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia", version: "0.26.0", platform: "windows",
    });
    mockDatasetLoad({
      fileName: "revisar.csv", fileSizeBytes: 32, rowCount: 1, columnCount: 1,
      columns: [{ name: "id", dataType: "Int64" }], rows: [["1"]],
    });

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await screen.findByRole("button", { name: /Continuar a Preparar|Empezar con la prioridad principal/ });

    const actions = screen.getByLabelText("Navegación entre etapas");
    expect(within(actions).queryByText("Siguiente paso")).not.toBeInTheDocument();
    expect(within(actions).queryByRole("button", { name: "Ver plan de preparación" })).not.toBeInTheDocument();
    expect(screen.getAllByRole("button", {
      name: /Continuar a Preparar|Empezar con la prioridad principal/,
    })).toHaveLength(1);

    fireEvent.click(screen.getByRole("button", {
      name: /Continuar a Preparar|Empezar con la prioridad principal/,
    }));
    expect(screen.getByRole("button", { name: "Preparar" })).toHaveAttribute("aria-current", "step");
    expect(within(screen.getByRole("button", { name: "Revisar" })).getByText("Hecho")).toBeInTheDocument();
  });

  it("invalida Revisar cuando Preparar modifica la revisión del dataset", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia", version: "0.26.0", platform: "windows",
    });
    const original: DatasetPreview = {
      fileName: "clientes.csv",
      fileSizeBytes: 128,
      rowCount: 1,
      columnCount: 1,
      columns: [{ name: "city", dataType: "String" }],
      rows: [[" Bogot\u00e1 "]],
    };
    mockDatasetLoad(original);
    vi.spyOn(bridge, "applySafeCorrections").mockResolvedValue({
      dataset: { ...original, rows: [["Bogot\u00e1"]] },
      affectedRowCount: 1,
      changedCellCount: 1,
      removedRowCount: 0,
      renamedColumnCount: 0,
      renames: [],
    });

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    fireEvent.click(await screen.findByRole("button", {
      name: /Continuar a Preparar|Empezar con la prioridad principal/,
    }));
    expect(within(screen.getByRole("button", { name: "Revisar" })).getByText("Hecho")).toBeInTheDocument();

    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("button", { name: "Aplicar plan seleccionado" }));
    expect(await screen.findByText("Plan aplicado: 1 celda actualizada.")).toBeInTheDocument();

    expect(within(screen.getByRole("button", { name: "Cargar" })).getByText("Hecho")).toBeInTheDocument();
    expect(within(screen.getByRole("button", { name: "Revisar" })).queryByText("Hecho")).not.toBeInTheDocument();
    expect(within(screen.getByRole("button", { name: "Preparar" })).getByText("Hecho")).toBeInTheDocument();
  });

  it("no marca Preparar como completada por avanzar solo con el footer genérico", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia", version: "0.26.0", platform: "windows",
    });
    mockDatasetLoad({
      fileName: "navegacion.csv", fileSizeBytes: 32, rowCount: 1, columnCount: 1,
      columns: [{ name: "id", dataType: "Int64" }], rows: [["1"]],
    });

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await screen.findByRole("button", { name: /Continuar a Preparar|Empezar con la prioridad principal/ });
    expect(within(screen.getByRole("button", { name: "Cargar" })).getByText("Hecho")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Preparar" }));
    fireEvent.click(screen.getByRole("button", { name: "Revisar opciones de entrega" }));

    expect(screen.getByRole("button", { name: "Entregar" })).toHaveAttribute("aria-current", "step");
    expect(within(screen.getByRole("button", { name: "Preparar" })).queryByText("Hecho")).not.toBeInTheDocument();
    expect(within(screen.getByRole("button", { name: "Revisar" })).queryByText("Hecho")).not.toBeInTheDocument();
  });

  it("aplica una tarea compatible a la preparación sin ejecutar transformaciones sobre las filas", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia", version: "0.26.0", platform: "windows",
    });
    const task: ReusableTask = {
      version: 1,
      name: "Cierre semanal",
      importProfile: {
        version: 1,
        format: "csv",
        headerMode: "firstRow",
        dateConvention: "unresolved",
        numberConvention: "unresolved",
        schema: [{ name: "id", dataType: "Int64" }],
      },
      recipe: {
        version: 1,
        name: "Renombrar id",
        savedAt: "2026-09-01T10:00:00Z",
        recipe: {
          renames: [{ from: "id", to: "id_limpio" }],
          casts: [{ column: "id", target: "integer" }], dateParses: [], filters: [], calculatedColumn: null,
          findReplace: null, keepColumns: null, splitColumn: null, mergeColumns: null,
          outlierTreatments: [], groupSummary: null, contactNormalizations: [], textExtractions: [],
        },
      },
      exceptionPolicy: {
        version: 1,
        baseline: "lexical",
        schema: [{ name: "id", dataType: "Int64" }],
        conversions: [{ kind: "cast", column: "id", target: "integer", onInvalid: "review" }],
      },
      qualityRules: [{ column: "id", kind: "not_null", maxInvalid: 0 }],
      outputFormat: "json",
      privacyMode: "mask",
    };
    const taskSummary: ReusableTaskSummary = {
      id: "task-weekly",
      name: task.name,
      createdAt: "2026-09-01T10:00:00Z",
      updatedAt: "2026-09-01T10:00:00Z",
      inputColumnCount: 1,
      hasRecipe: true,
      qualityRuleCount: 1,
      outputFormat: "json",
    };
    const checkSchemaSpy = vi.spyOn(bridge, "checkReusableTaskSchema").mockResolvedValue({
      status: "ready", missingColumns: [], addedColumns: [], changedTypes: [], orderChanged: false,
    });
    vi.spyOn(bridge, "listReusableTasks").mockResolvedValue([taskSummary]);
    vi.spyOn(bridge, "openReusableTask").mockResolvedValue(task);
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(historyState({
      canUndo: false,
      currentIndex: 0,
      entryCount: 1,
      entries: [{ id: "history-test-0", index: 0, label: "Dataset cargado", isCurrent: true }],
    }));
    mockDatasetLoad({
      fileName: "entrada.csv", fileSizeBytes: 32, rowCount: 1, columnCount: 1,
      columns: [{ name: "id", dataType: "Int64" }], rows: [["10"]],
    });
    const applyRecipeSpy = vi.spyOn(bridge, "applyTransformRecipe").mockResolvedValue({
      dataset: {
        fileName: "entrada.csv", fileSizeBytes: 32, rowCount: 1, columnCount: 1,
        columns: [{ name: "id_limpio", dataType: "Int64" }], rows: [["10"]],
      },
      changed: true,
      renamedColumnCount: 1,
      convertedColumnCount: 0,
      parsedDateColumnCount: 0,
      removedRowCount: 0,
      calculatedColumnCount: 0,
      replacedCellCount: 0,
      droppedColumnCount: 0,
      splitColumnCount: 0, mergedColumnCount: 0, droppedSourceColumnCount: 0,
      adjustedOutlierCellCount: 0, outlierRemovedRowCount: 0, outlierColumnCount: 0,
      groupCount: 0, aggregatedColumnCount: 0, collapsedRowCount: 0,
      normalizedContactCellCount: 0, normalizedContactColumnCount: 0, extractedColumnCount: 0,
    });
    vi.spyOn(bridge, "saveReusableTask").mockResolvedValue(taskSummary);

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await screen.findByRole("heading", { name: "entrada.csv" });
    fireEvent.click(screen.getByRole("button", { name: "Cargar" }));
    fireEvent.click(screen.getByText("Reutilizar una tarea"));
    await screen.findByLabelText("Tarea guardada");
    fireEvent.change(screen.getByLabelText("Tarea guardada"), { target: { value: taskSummary.id } });
    await screen.findByText("El esquema es compatible. Puedes aplicar la configuración guardada.");
    expect(checkSchemaSpy).toHaveBeenCalledWith(taskSummary.id, [{ name: "id", dataType: "Int64" }]);
    fireEvent.change(screen.getByLabelText("Valores no interpretables en id"), { target: { value: "excludeRow" } });
    fireEvent.click(screen.getByRole("button", { name: "Aplicar al dataset actual" }));
    await waitFor(() => expect(bridge.saveReusableTask).toHaveBeenCalledWith(
      taskSummary.id,
      expect.objectContaining({
        exceptionPolicy: expect.objectContaining({
          conversions: [{ kind: "cast", column: "id", target: "integer", onInvalid: "excludeRow" }],
        }),
      }),
    ));

    expect(screen.getByRole("button", { name: "Preparar" })).toHaveAttribute("aria-current", "step");
    fireEvent.click(await screen.findByRole("tab", { name: "Transformaciones" }));
    expect(screen.getByRole("textbox", { name: "Nombre de la receta" })).toHaveValue("Renombrar id");
    expect(screen.getByRole("textbox", { name: "Nuevo nombre 1" })).toHaveValue("id_limpio");
    expect(applyRecipeSpy).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Aplicar receta" }));
    await waitFor(() => expect(applyRecipeSpy).toHaveBeenCalledWith(
      expect.objectContaining({ casts: [{ column: "id", target: "integer" }] }),
      expect.objectContaining({
        conversions: [{ kind: "cast", column: "id", target: "integer", onInvalid: "excludeRow" }],
      }),
    ));

    await switchPhase("Entregar");
    expect(screen.getByText(/id: no admite valores nulos/)).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Formato de exportación" })).toHaveValue("json");
    expect(screen.getByRole("combobox", { name: "Protección de datos personales" })).toHaveValue("mask");

    await switchPhase("Revisar");
    fireEvent.click(screen.getByRole("tab", { name: "Vista previa" }));
    expect(await screen.findByRole("cell", { name: "10" })).toBeInTheDocument();
    expect(applyRecipeSpy).toHaveBeenCalledOnce();
  });

  it("aplica la configuración no mutadora de una tarea compatible al importar, sin ejecutar su receta", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia", version: "0.26.0", platform: "windows",
    });
    const { task, summary } = reusableTaskFixture();
    mockDatasetLoad({
      fileName: "cierre-nuevo.csv", fileSizeBytes: 32, rowCount: 1, columnCount: 1,
      columns: [{ name: "id", dataType: "Int64" }], rows: [["20"]],
    });
    const applyRecipeSpy = vi.spyOn(bridge, "applyTransformRecipe");

    await prepareReusableTaskBeforeImport(task, summary);
    await waitFor(() => expect(bridge.loadDatasetSelection).toHaveBeenCalledOnce());
    const importCall = vi.mocked(bridge.loadDatasetSelection).mock.calls[0];
    expect(importCall?.[4]).toEqual(task.importProfile);
    expect(importCall?.[5]).toBe(task.importProfile.dateConvention);
    expect(importCall?.[6]).toBe(task.importProfile.numberConvention);

    expect(screen.queryByRole("dialog", { name: "Revisa la configuración guardada" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Preparar" })).toHaveAttribute("aria-current", "step");
    expect(applyRecipeSpy).not.toHaveBeenCalled();

    await screen.findByRole("heading", { name: "Prepara datos consistentes" }, { timeout: 5000 });
    fireEvent.click(await screen.findByRole("tab", { name: "Transformaciones" }, { timeout: 5000 }));
    expect(screen.getByRole("textbox", { name: "Nombre de la receta" })).toHaveValue("Renombrar id");
    expect(applyRecipeSpy).not.toHaveBeenCalled();

    await switchPhase("Entregar");
    expect(screen.getByText(/id: no admite valores nulos/)).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Formato de exportación" })).toHaveValue("json");
    expect(screen.getByRole("combobox", { name: "Protección de datos personales" })).toHaveValue("mask");
  });

  it("detecta un esquema incompatible en el preflight y carga sin aplicar el perfil guardado", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia", version: "0.26.0", platform: "windows",
    });
    const { task, summary } = reusableTaskFixture();
    const loadSpy = mockDatasetLoad({
      fileName: "cierre-cambiado.csv", fileSizeBytes: 32, rowCount: 1, columnCount: 2,
      columns: [
        { name: "identificador", dataType: "String" },
        { name: "importe", dataType: "Float64" },
      ],
      rows: [["20", "12.5"]],
    });
    vi.mocked(bridge.previewDatasetSelection).mockResolvedValueOnce({
      rowCount: 1,
      columns: [
        { name: "identificador", dataType: "String" },
        { name: "importe", dataType: "Float64" },
      ],
      schemaMismatch: {
        missingColumns: ["id"],
        addedColumns: ["identificador", "importe"],
        changedTypes: [{ column: "id", expected: "Int64", actual: "String" }],
      },
    });

    await prepareReusableTaskBeforeImport(task, summary);

    const importDialog = await screen.findByRole("dialog", { name: "Revisar encabezados de cierre-cambiado.csv" });
    expect(within(importDialog).getByRole("alert")).toHaveTextContent("El esquema no coincide con el perfil guardado");
    expect(within(importDialog).getByText("Columnas faltantes: id")).toBeInTheDocument();
    expect(within(importDialog).getByText("Columnas nuevas: identificador, importe")).toBeInTheDocument();
    expect(within(importDialog).getByText("Tipos distintos: id (Int64 → String)")).toBeInTheDocument();
    expect(loadSpy).not.toHaveBeenCalled();

    fireEvent.click(within(importDialog).getByRole("button", { name: "Importar con esquema nuevo" }));
    const applicationReview = await screen.findByRole("dialog", { name: "Revisa la configuración guardada" });
    expect(applicationReview).toHaveTextContent("Confirmaste un esquema distinto");
    expect(loadSpy).toHaveBeenCalledWith(
      "selection-test",
      null,
      "firstRow",
      expect.any(Function),
      null,
      task.importProfile.dateConvention,
      task.importProfile.numberConvention,
    );
    fireEvent.click(within(applicationReview).getByRole("button", { name: "Seguir sin esos ajustes" }));
  });

  it("permite reintentar el preflight de esquema sin activar un dataset parcial", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia", version: "0.26.0", platform: "windows",
    });
    const loadSpy = mockDatasetLoad({
      fileName: "reintento.csv", fileSizeBytes: 32, rowCount: 1, columnCount: 1,
      columns: [{ name: "value", dataType: "String" }], rows: [["ok"]],
    });
    vi.mocked(bridge.previewDatasetSelection)
      .mockRejectedValueOnce(new Error("lectura temporal fallida"))
      .mockResolvedValueOnce({
        rowCount: 1,
        columns: [{ name: "value", dataType: "String" }],
        schemaMismatch: null,
      });

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    const importDialog = await screen.findByRole("dialog", { name: "Revisar encabezados de reintento.csv" });
    const reviewButton = within(importDialog).getByRole("button", { name: "Revisar esquema" });
    await waitFor(() => expect(reviewButton).toBeEnabled());
    fireEvent.click(reviewButton);

    expect(await within(importDialog).findByRole("alert")).toHaveTextContent(
      "No se pudo revisar el esquema: lectura temporal fallida",
    );
    expect(loadSpy).not.toHaveBeenCalled();
    fireEvent.click(within(importDialog).getByRole("button", { name: "Reintentar esquema" }));
    fireEvent.click(await within(importDialog).findByRole("button", { name: "Cargar archivo" }));

    expect(await screen.findByRole("heading", { name: "reintento.csv" })).toBeInTheDocument();
    expect(loadSpy).toHaveBeenCalledOnce();
  });

  it("recupera el selector tras un error nativo no tipado", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia", version: "0.26.0", platform: "windows",
    });
    vi.spyOn(bridge, "pickDatasetSource").mockRejectedValue("selector nativo no disponible");

    render(<App />);
    const selectDataset = await screen.findByRole("button", { name: "Seleccionar dataset" });
    fireEvent.click(selectDataset);
    await waitFor(() => expect(bridge.pickDatasetSource).toHaveBeenCalledOnce());
    await waitFor(() => expect(selectDataset).toBeEnabled());
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("reintenta la muestra de encabezados después de un fallo nativo no tipado", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia", version: "0.26.0", platform: "windows",
    });
    mockDatasetLoad({
      fileName: "encabezados.csv", fileSizeBytes: 32, rowCount: 1, columnCount: 1,
      columns: [{ name: "value", dataType: "String" }], rows: [["ok"]],
    });
    vi.mocked(bridge.previewDelimitedHeaderReview)
      .mockRejectedValueOnce("muestra temporal fallida")
      .mockResolvedValueOnce(defaultDelimitedHeaderReview());

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    const importDialog = await screen.findByRole("dialog", { name: "Revisar encabezados de encabezados.csv" });
    expect(await within(importDialog).findByRole("alert")).toHaveTextContent("muestra temporal fallida");

    fireEvent.click(within(importDialog).getByRole("button", { name: "Reintentar muestra" }));
    expect(await within(importDialog).findByRole("region", { name: "Vista previa de la interpretación" }))
      .toHaveTextContent("La primera fila se usa para nombrar columnas");
    expect(bridge.previewDelimitedHeaderReview).toHaveBeenCalledTimes(2);
  });

  it("restaura el estado vacío si la carga falla con un error nativo no tipado", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia", version: "0.26.0", platform: "windows",
    });
    const loadSpy = mockDatasetLoad({
      fileName: "fallida.csv", fileSizeBytes: 32, rowCount: 1, columnCount: 1,
      columns: [{ name: "value", dataType: "String" }], rows: [["ok"]],
    }).mockRejectedValueOnce("lectura nativa fallida");

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));

    await waitFor(() => expect(loadSpy).toHaveBeenCalledOnce());
    await waitFor(() => expect(screen.getByRole("button", { name: "Seleccionar dataset" })).toBeEnabled());
    expect(screen.queryByRole("heading", { name: "fallida.csv" })).not.toBeInTheDocument();
  });

  it("reutiliza un perfil CSV con encabezados generados, conserva la primera fila y no muestra otra confirmación", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia", version: "0.26.0", platform: "windows",
    });
    const { task, summary } = reusableTaskFixture();
    const generatedProfile = {
      ...task.importProfile,
      headerMode: "generated" as const,
      schema: [{ name: "column_1", dataType: "String" }],
    };
    const generatedTask = { ...task, importProfile: generatedProfile };
    const loadSpy = mockDatasetLoad({
      fileName: "cierre-sin-encabezados.csv",
      fileSizeBytes: 32,
      rowCount: 2,
      columnCount: 1,
      columns: [{ name: "column_1", dataType: "String" }],
      rows: [["id"], ["001"]],
    });

    await prepareReusableTaskBeforeImport(generatedTask, summary);

    await waitFor(() => expect(loadSpy).toHaveBeenCalledOnce());
    expect(loadSpy).toHaveBeenCalledWith(
      "selection-test",
      null,
      "generated",
      expect.any(Function),
      generatedProfile,
      generatedProfile.dateConvention,
      generatedProfile.numberConvention,
    );
    expect(screen.queryByRole("dialog", { name: "Revisa la configuración guardada" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Preparar" })).toHaveAttribute("aria-current", "step");
    await switchPhase("Revisar");
    fireEvent.click(await screen.findByRole("tab", { name: "Vista previa" }));
    expect(await screen.findByRole("cell", { name: "id" })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "001" })).toBeInTheDocument();
  });

  it("detiene el reuso ante un esquema distinto y requiere confirmación antes de importar o aplicar los ajustes", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia", version: "0.26.0", platform: "windows",
    });
    const { task, summary } = reusableTaskFixture();
    const mismatch = {
      code: "importProfileSchemaMismatch",
      missingColumns: ["id"],
      addedColumns: ["identificador"],
      changedTypes: [],
    } as const;
    mockDatasetLoad({
      fileName: "cierre-cambiado.csv", fileSizeBytes: 32, rowCount: 1, columnCount: 1,
      columns: [{ name: "identificador", dataType: "String" }], rows: [["20"]],
    }).mockRejectedValueOnce(new Error(`__columnia_import_profile_mismatch__:${JSON.stringify(mismatch)}`));
    const loadSpy = vi.mocked(bridge.loadDatasetSelection);
    const applyRecipeSpy = vi.spyOn(bridge, "applyTransformRecipe");

    await prepareReusableTaskBeforeImport(task, summary);
    const mismatchDialog = await screen.findByRole("alertdialog", { name: "El esquema difiere del perfil guardado" });
    expect(within(mismatchDialog).getByText("Falta la columna “id”")).toBeInTheDocument();
    expect(within(mismatchDialog).getByText("Columna nueva “identificador”")).toBeInTheDocument();
    expect(loadSpy).toHaveBeenCalledOnce();
    expect(loadSpy.mock.calls[0]?.[4]).toEqual(task.importProfile);
    expect(applyRecipeSpy).not.toHaveBeenCalled();

    fireEvent.click(within(mismatchDialog).getByRole("button", { name: "Importar con esquema nuevo" }));
    const applicationReview = await screen.findByRole("dialog", { name: "Revisa la configuración guardada" });
    expect(applicationReview).toHaveTextContent("Confirmaste un esquema distinto");
    expect(applicationReview).toHaveTextContent("la interpretación predeterminada");
    expect(loadSpy).toHaveBeenCalledTimes(2);
    expect(loadSpy.mock.calls[1]?.[4]).toBeNull();
    expect(applyRecipeSpy).not.toHaveBeenCalled();

    fireEvent.click(within(applicationReview).getByRole("button", { name: "Seguir sin esos ajustes" }));
    await switchPhase("Entregar");
    expect(screen.getByRole("combobox", { name: "Formato de exportación" })).toHaveValue("csv");
    expect(screen.getByRole("combobox", { name: "Protección de datos personales" })).toHaveValue("none");
    expect(applyRecipeSpy).not.toHaveBeenCalled();
  });

  it("descarta de forma segura una selección cuyo perfil falla por cambio de esquema", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia", version: "0.26.0", platform: "windows",
    });
    const { task, summary } = reusableTaskFixture();
    const mismatch = {
      code: "importProfileSchemaMismatch",
      missingColumns: ["id"],
      addedColumns: ["identificador"],
      changedTypes: [],
    } as const;
    mockDatasetLoad({
      fileName: "cierre-cancelado.csv", fileSizeBytes: 32, rowCount: 1, columnCount: 1,
      columns: [{ name: "identificador", dataType: "String" }], rows: [["20"]],
    }).mockRejectedValueOnce(new Error(`__columnia_import_profile_mismatch__:${JSON.stringify(mismatch)}`));
    const cancelSpy = vi.spyOn(bridge, "cancelOperation").mockResolvedValue(undefined);
    const discardSpy = vi.spyOn(bridge, "discardDatasetSelection").mockResolvedValue(undefined);

    await prepareReusableTaskBeforeImport(task, summary);
    const mismatchDialog = await screen.findByRole("alertdialog", { name: "El esquema difiere del perfil guardado" });
    fireEvent.click(within(mismatchDialog).getByRole("button", { name: "Cancelar y conservar dataset" }));

    await waitFor(() => expect(discardSpy).toHaveBeenCalledWith("selection-test"));
    expect(cancelSpy).toHaveBeenCalledWith("load");
    expect(screen.queryByRole("alertdialog", { name: "El esquema difiere del perfil guardado" })).not.toBeInTheDocument();
  });

  it("abre el diagnóstico local desde las preferencias tras una acción explícita", async () => {
    const saveDiagnostic = vi.spyOn(bridge, "saveDiagnosticReport");

    renderAppWithHeaderConfirmation();
    fireEvent.click(screen.getByText("Preferencias y recursos"));
    fireEvent.click(screen.getByRole("button", { name: "Preparar diagnóstico local" }));

    expect(await screen.findByRole("dialog", { name: "Diagnóstico local revisable" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Crear vista previa" })).toBeDisabled();
    expect(saveDiagnostic).not.toHaveBeenCalled();
  });

  it("restaura reglas y borrador de un proyecto y refresca gates e historial", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.25.0", platform: "windows" });
    const project: ProjectSummary = {
      id: "project-1", name: "Ventas", datasetFileName: "ventas.csv", rowCount: 1, columnCount: 1,
      createdAt: "2026-08-20T00:00:00Z", updatedAt: "2026-08-21T00:00:00Z",
    };
    const draft: SavedRecipe = {
      version: 1, name: "Renombrar total", savedAt: "2026-08-21T00:00:00Z",
      recipe: {
        renames: [{ from: "total", to: "importe" }], casts: [], dateParses: [], filters: [],
        calculatedColumn: null, findReplace: null, keepColumns: null, splitColumn: null,
        mergeColumns: null, outlierTreatments: [], groupSummary: null,
        contactNormalizations: [], textExtractions: [],
      },
    };
    const dataset: DatasetPreview = {
      fileName: "ventas.csv", fileSizeBytes: 128, rowCount: 1, columnCount: 1,
      columns: [{ name: "total", dataType: "Int64" }], rows: [["10"]],
    };
    vi.spyOn(bridge, "listProjects").mockResolvedValue({ projects: [project], recoveryCandidate: null });
    vi.spyOn(bridge, "openProject").mockResolvedValue({
      project,
      dataset,
      workspace: {
        qualityRules: [{ column: "total", kind: "not_null", maxInvalid: 0 }],
        recipeDraft: draft,
        performanceProfile: "maximum",
      },
      profile: {
        rowCount: 1,
        duplicateRowCount: 0,
        nearDuplicateRowCount: 0,
        duplicatePercentage: 0,
        columns: [],
      },
    });
    vi.spyOn(bridge, "pickDatasetSource").mockResolvedValue({
      selectionId: "external-selection", fileName: "externo.csv", fileSizeBytes: 64,
      format: "csv", sheets: [], defaultSheetId: null, isCompressedContainer: false,
      resourceEstimate: resourceEstimate(64),
    });
    vi.spyOn(bridge, "loadDatasetSelection").mockResolvedValue({
      fileName: "externo.csv", fileSizeBytes: 64, rowCount: 1, columnCount: 1,
      columns: [{ name: "otro", dataType: "String" }], rows: [["dato"]],
    });
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(historyState({
      canUndo: false, entryCount: 1, currentIndex: 0,
      entries: [{ id: "history-test-0", index: 0, label: "Dataset cargado", isCurrent: true }],
    }));
    const profileSpy = vi.spyOn(bridge, "getDatasetProfile").mockResolvedValue({
      rowCount: 1,
      duplicateRowCount: 0,
      nearDuplicateRowCount: 0,
      duplicatePercentage: 0,
      columns: [],
    });

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Abrir" }));
    expect(await screen.findByRole("heading", { name: "ventas.csv" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Revisar" })).toHaveAttribute("aria-current", "step");
    expect(screen.getByRole("button", { name: /Continuar a Preparar|Empezar con la prioridad principal/ })).toBeInTheDocument();
    expect(screen.getByText("Filas analizadas").parentElement).toHaveTextContent("Filas analizadas1");

    await switchPhase("Entregar");
    expect(screen.getByRole("radio", { name: /^Validar calidad/ })).toBeChecked();
    expect(screen.getByText("total: no admite valores nulos · no se permiten incumplimientos.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Editar reglas" }));
    expect(screen.getByRole("combobox", { name: "Columna regla 1" })).toHaveValue("total");
    expect(screen.queryByText("Contrato aprobado")).not.toBeInTheDocument();

    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("tab", { name: "Transformaciones" }));
    expect(screen.getByRole("textbox", { name: "Nombre de la receta" })).toHaveValue("Renombrar total");
    expect(screen.getByRole("textbox", { name: "Nuevo nombre 1" })).toHaveValue("importe");
    expect(profileSpy).not.toHaveBeenCalled();

    await switchPhase("Cargar");
    fireEvent.click(screen.getByRole("button", { name: "Seleccionar otro dataset" }));
    expect(await screen.findByRole("heading", { name: "externo.csv" })).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: /Continuar a Preparar|Empezar con la prioridad principal/ })).toBeInTheDocument();
    fireEvent.click(screen.getByText("Preferencias y recursos"));
    await waitFor(() => expect(screen.getByRole("combobox", { name: "Modo de rendimiento" })).toHaveValue("balanced"));
    await switchPhase("Entregar");
    expect(screen.getByRole("radio", { name: /^Validar calidad/ })).not.toBeChecked();
    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("tab", { name: "Transformaciones" }));
    expect(screen.getByRole("textbox", { name: "Nombre de la receta" })).toHaveValue("Mi receta");
    expect(screen.getByRole("combobox", { name: "Columna para renombrar 1" })).toHaveValue("");
    fireEvent.click(screen.getByRole("button", { name: "Cargar" }));
    expect(screen.getByRole("button", { name: "Guardar proyecto nuevo" })).toBeInTheDocument();
  });

  it("guarda y confirma el borrado de un proyecto sin descartar el dataset", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.49.0", platform: "windows" });

    const dataset: DatasetPreview = {
      fileName: "ventas.csv", fileSizeBytes: 128, rowCount: 2, columnCount: 1,
      columns: [{ name: "total", dataType: "Int64" }], rows: [["10"], ["20"]],
    };
    const project: ProjectSummary = {
      id: "project-cycle", name: "Ventas durable", datasetFileName: dataset.fileName,
      rowCount: dataset.rowCount, columnCount: dataset.columnCount,
      createdAt: "2026-08-20T00:00:00Z", updatedAt: "2026-08-21T00:00:00Z",
    };
    let catalog: ProjectSummary[] = [];
    const listSpy = vi.spyOn(bridge, "listProjects").mockImplementation(async () => ({ projects: catalog, recoveryCandidate: null }));
    const saveSpy = vi.spyOn(bridge, "saveProject").mockImplementation(async () => {
      catalog = [project];
      return project;
    });
    const deleteSpy = vi.spyOn(bridge, "deleteProject").mockImplementation(async () => {
      catalog = [];
    });
    mockDatasetLoad(dataset);

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    expect(await screen.findByRole("heading", { name: "ventas.csv" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Cargar" }));

    const projectName = await screen.findByRole("textbox", { name: "Nombre del proyecto" });
    fireEvent.change(projectName, { target: { value: project.name } });
    fireEvent.click(screen.getByRole("button", { name: "Guardar proyecto nuevo" }));
    await waitFor(() => expect(saveSpy).toHaveBeenCalledWith(
      null,
      project.name,
      { qualityRules: [], recipeDraft: null, reviewTab: "diagnosis", previewOffset: 0, activePhase: "load", queryEngine: "polars", analysisSampleRows: 100_000, performanceProfile: "balanced", exportFormat: "csv", privacyMode: "none", comparisonKeyColumns: [], joinType: "inner", importProfile: { version: 1, format: "csv", headerMode: "firstRow", dateConvention: "unresolved", numberConvention: "unresolved", schema: [{ name: "total", dataType: "Int64" }] } },
    ));
    expect(await screen.findByText(`Proyecto “${project.name}” guardado.`)).toBeInTheDocument();
    await waitFor(() => expect(listSpy.mock.calls.length).toBeGreaterThanOrEqual(2));

    const deleteButton = await screen.findByRole("button", { name: "Eliminar" });
    fireEvent.click(deleteButton);
    const dialog = screen.getByRole("alertdialog", { name: `Eliminar “${project.name}”` });
    expect(dialog).toHaveTextContent("El dataset abierto en memoria no se descartará.");
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancelar" }));
    expect(deleteSpy).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Eliminar" }));
    fireEvent.click(screen.getByRole("button", { name: "Eliminar proyecto" }));
    await waitFor(() => expect(deleteSpy).toHaveBeenCalledWith(project.id));
    expect(await screen.findByText(`Proyecto “${project.name}” eliminado. El dataset abierto se conserva.`)).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "ventas.csv" })).toBeInTheDocument();
    expect(screen.getByText("Todavía no hay proyectos guardados.")).toBeInTheDocument();
  });

  it("abre desde el catálogo el proyecto recién guardado y restaura su workspace", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.49.0", platform: "windows" });

    const dataset: DatasetPreview = {
      fileName: "clientes.csv", fileSizeBytes: 96, rowCount: 75, columnCount: 1,
      columns: [{ name: "email", dataType: "String" }], rows: [["ana@example.com"]],
    };
    const project: ProjectSummary = {
      id: "project-reopen", name: "Clientes durable", datasetFileName: dataset.fileName,
      rowCount: dataset.rowCount, columnCount: dataset.columnCount,
      createdAt: "2026-08-20T00:00:00Z", updatedAt: "2026-08-21T00:00:00Z",
    };
    let catalog: ProjectSummary[] = [];
    const listSpy = vi.spyOn(bridge, "listProjects").mockImplementation(async () => ({ projects: catalog, recoveryCandidate: null }));
    const saveSpy = vi.spyOn(bridge, "saveProject").mockImplementation(async () => {
      catalog = [project];
      return project;
    });
    const openSpy = vi.spyOn(bridge, "openProject").mockResolvedValue({
      project,
      dataset,
      workspace: { qualityRules: [{ column: "email", kind: "not_null", maxInvalid: 0 }], recipeDraft: null, reviewTab: "preview", previewOffset: 50, activePhase: "prepare", queryEngine: "duckdb", analysisSampleRows: 50_000, performanceProfile: "maximum", exportFormat: "bundle", privacyMode: "hash", comparisonKeyColumns: ["email", "missing", "email"], joinType: "full" },
      profile: { rowCount: 1, duplicateRowCount: 0, nearDuplicateRowCount: 0, duplicatePercentage: 0, columns: [] },
    });
    const pageSpy = vi.spyOn(bridge, "getDatasetPage").mockResolvedValue({
      offset: 50,
      rows: [["lucia@example.com"]],
    });
    mockDatasetLoad(dataset);

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await screen.findByRole("heading", { name: "Revisa antes de modificar" });
    await switchPhase("Cargar");
    fireEvent.click(screen.getByText("Continuar un proyecto"));
    fireEvent.click(screen.getByText("Guardar y administrar proyectos"));

    const projectName = await screen.findByRole("textbox", { name: "Nombre del proyecto" });
    fireEvent.change(projectName, { target: { value: project.name } });
    fireEvent.click(screen.getByRole("button", { name: "Guardar proyecto nuevo" }));
    await waitFor(() => expect(saveSpy).toHaveBeenCalledWith(
      null,
      project.name,
      { qualityRules: [], recipeDraft: null, reviewTab: "diagnosis", previewOffset: 0, activePhase: "load", queryEngine: "polars", analysisSampleRows: 100_000, performanceProfile: "balanced", exportFormat: "csv", privacyMode: "none", comparisonKeyColumns: [], joinType: "inner", importProfile: { version: 1, format: "csv", headerMode: "firstRow", dateConvention: "unresolved", numberConvention: "unresolved", schema: [{ name: "email", dataType: "String" }] } },
    ));
    await waitFor(() => expect(listSpy.mock.calls.length).toBeGreaterThanOrEqual(2));

    fireEvent.click(await screen.findByRole("button", { name: "Abrir" }));
    await waitFor(() => expect(openSpy).toHaveBeenCalledWith(project.id));
    expect(await screen.findByRole("heading", { name: "clientes.csv" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Preparar" })).toHaveAttribute("aria-current", "step");
    fireEvent.click(screen.getByRole("button", { name: "Revisar" }));
    const comparison = screen.getByRole("region", { name: "Comparar datasets" });
    expect(within(comparison).getByRole("checkbox", { name: /email/ })).toBeChecked();
    expect(within(comparison).getByRole("radio", { name: /^Full/ })).toBeChecked();
    expect(screen.getByRole("tab", { name: "Vista previa" })).toHaveAttribute("aria-selected", "true");
    fireEvent.click(screen.getByRole("tab", { name: "Diagnóstico" }));
    fireEvent.click(screen.getByText("Explorar con SQL local"));
    expect(screen.getByRole("combobox", { name: "Motor de consulta" })).toHaveValue("duckdb");
    expect(screen.getByRole("combobox", { name: "Filas de muestra para correlaciones" })).toHaveValue("50000");
    fireEvent.click(screen.getByText("Preferencias y recursos"));
    expect(screen.getByRole("combobox", { name: "Modo de rendimiento" })).toHaveValue("maximum");
    fireEvent.click(screen.getByRole("tab", { name: "Vista previa" }));
    expect(await screen.findByRole("cell", { name: "lucia@example.com" })).toBeInTheDocument();
    expect(screen.getByText(/Filas 51–51 de 75/)).toBeInTheDocument();
    expect(pageSpy).toHaveBeenCalledWith(50, 50);

    await switchPhase("Entregar");
    expect(screen.getByText("email: no admite valores nulos · no se permiten incumplimientos.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Editar reglas" }));
    expect(screen.getByRole("combobox", { name: "Columna regla 1" })).toHaveValue("email");
    expect(screen.getByRole("combobox", { name: "Formato de exportación" })).toHaveValue("bundle");
    expect(screen.getByRole("combobox", { name: "Protección de datos personales" })).toHaveValue("hash");
  });

  it("analiza automáticamente un proyecto abierto que no incluya un perfil durable", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.26.0", platform: "windows" });
    const project: ProjectSummary = {
      id: "project-without-profile", name: "Sin perfil", datasetFileName: "simple.csv",
      rowCount: 1, columnCount: 1, createdAt: "2026-08-20T00:00:00Z", updatedAt: "2026-08-21T00:00:00Z",
    };
    vi.spyOn(bridge, "listProjects").mockResolvedValue({ projects: [project], recoveryCandidate: null });
    vi.spyOn(bridge, "openProject").mockResolvedValue({
      project,
      dataset: {
        fileName: "simple.csv", fileSizeBytes: 16, rowCount: 1, columnCount: 1,
        columns: [{ name: "id", dataType: "Int64" }], rows: [["1"]],
      },
      workspace: { qualityRules: [], recipeDraft: null },
      profile: null,
    });
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(historyState());
    const profileSpy = vi.spyOn(bridge, "getDatasetProfile").mockResolvedValue({
      rowCount: 1,
      duplicateRowCount: 0,
      nearDuplicateRowCount: 0,
      duplicatePercentage: 0,
      columns: [],
    });

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Abrir" }));

    expect(await screen.findByRole("button", { name: /Continuar a Preparar|Empezar con la prioridad principal/ })).toBeInTheDocument();
    expect(screen.getByText("Filas analizadas").parentElement).toHaveTextContent("Filas analizadas1");
    expect(profileSpy).toHaveBeenCalledOnce();
  });

  it("permite reintentar el diagnóstico automático tras un fallo sin duplicar controles", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia", version: "0.26.0", platform: "windows",
    });
    const dataset: DatasetPreview = {
      fileName: "reintento.csv", fileSizeBytes: 32, rowCount: 1, columnCount: 1,
      columns: [{ name: "id", dataType: "Int64" }], rows: [["1"]],
    };
    const profileSpy = vi.spyOn(bridge, "getDatasetProfile")
      .mockRejectedValueOnce(new Error("fallo temporal"))
      .mockResolvedValue({
        rowCount: 1, duplicateRowCount: 0, nearDuplicateRowCount: 0,
        duplicatePercentage: 0, columns: [],
      });
    mockDatasetLoad(dataset);

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));

    const retry = await screen.findByRole("button", { name: "Reintentar análisis" });
    expect(screen.queryByRole("button", { name: "Analizar calidad" })).not.toBeInTheDocument();
    expect(profileSpy).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Preparar" }));
    expect(await screen.findByRole("button", { name: "Reintentar análisis" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Revisar opciones de entrega" })).not.toBeInTheDocument();
    fireEvent.click(retry);

    expect(await screen.findByRole("button", { name: "Revisar opciones de entrega" })).toBeEnabled();
    expect(profileSpy).toHaveBeenCalledTimes(2);
  });

  it("descarta perfiles que terminan después de una revisión más nueva", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.26.0", platform: "windows" });
    const initial: DatasetPreview = {
      fileName: "revision.csv", fileSizeBytes: 64, rowCount: 4, columnCount: 1,
      columns: [{ name: "valor", dataType: "String" }], rows: [["1"], ["2"], ["3"], ["4"]],
    };
    const afterFirstChange = { ...initial, rowCount: 3, rows: [["1"], ["2"], ["3"]] };
    const afterSecondChange = { ...initial, rowCount: 2, rows: [["1"], ["2"]] };
    const initialProfile: DatasetProfile = {
      rowCount: 4, duplicateRowCount: 0, nearDuplicateRowCount: 0, duplicatePercentage: 0, columns: [],
    };
    const staleProfile: DatasetProfile = {
      rowCount: 3, duplicateRowCount: 0, nearDuplicateRowCount: 0, duplicatePercentage: 0, columns: [],
    };
    const latestProfile: DatasetProfile = {
      rowCount: 2, duplicateRowCount: 1, nearDuplicateRowCount: 0, duplicatePercentage: 50, columns: [],
    };
    let resolveStaleProfile!: (profile: DatasetProfile) => void;
    const staleProfilePromise = new Promise<DatasetProfile>((resolve) => { resolveStaleProfile = resolve; });
    const profileSpy = vi.spyOn(bridge, "getDatasetProfile")
      .mockResolvedValueOnce(initialProfile)
      .mockReturnValueOnce(staleProfilePromise)
      .mockResolvedValue(latestProfile);
    mockDatasetLoad(initial);
    const removeRowsSpy = vi.spyOn(bridge, "removeEmptyRows")
      .mockResolvedValueOnce({ dataset: afterFirstChange, affectedRowCount: 1 })
      .mockResolvedValueOnce({ dataset: afterSecondChange, affectedRowCount: 1 });

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await screen.findByRole("button", { name: /Continuar a Preparar|Empezar con la prioridad principal/ });
    await switchPhase("Preparar");
    fireEvent.click(screen.getByText("Más herramientas"));
    fireEvent.click(screen.getByRole("button", { name: "Eliminar filas vacías" }));
    await waitFor(() => expect(profileSpy).toHaveBeenCalledTimes(2));
    await screen.findByRole("heading", { name: "Actualizando el diagnóstico" });

    fireEvent.click(screen.getByRole("button", { name: "Eliminar filas vacías" }));
    await waitFor(() => expect(removeRowsSpy).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(profileSpy).toHaveBeenCalledTimes(3));
    const currentDuplicatePlan = screen.getByRole("checkbox", { name: /Retirar 1 fila duplicada exacta/ });
    await waitFor(() => expect(currentDuplicatePlan).toBeChecked());

    await act(async () => {
      resolveStaleProfile(staleProfile);
      await staleProfilePromise;
    });
    await waitFor(() => {
      expect(screen.getByRole("checkbox", { name: /Retirar 1 fila duplicada exacta/ })).toBeChecked();
    });
  });

  it("explica cómo conectar el motor cuando se abre en navegador", async () => {
    renderAppWithHeaderConfirmation();

    expect(screen.getByRole("heading", { name: "Columnia" })).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Flujo de preparación de datos" })).toBeInTheDocument();
    expect(screen.getByRole("progressbar", { name: "Progreso del flujo" })).toHaveAttribute("aria-valuenow", "1");
    expect(screen.queryByRole("button", { name: "Continuar a Revisar" })).not.toBeInTheDocument();
    const lockedReview = screen.getByRole("button", { name: "Revisar" });
    expect(lockedReview).toHaveAttribute("aria-disabled", "true");
    expect(lockedReview).toHaveAttribute("aria-describedby", "dataset-required-hint");
    fireEvent.click(lockedReview);
    expect(screen.getByRole("progressbar", { name: "Progreso del flujo" })).toHaveAttribute("aria-valuenow", "1");
    expect(screen.getByRole("link", { name: "Saltar al contenido principal" })).toHaveAttribute(
      "href",
      "#main-content",
    );
    expect(screen.getByRole("main")).toHaveAttribute("id", "main-content");
    expect(
      await screen.findByText(/Abre Columnia con Tauri para seleccionar archivos locales/),
    ).toBeInTheDocument();
  });

  it("expone landmarks y la descripción accesible del panel legal en la interfaz renderizada", async () => {
    renderAppWithHeaderConfirmation();

    expect(screen.getByRole("link", { name: "Saltar al contenido principal" })).toHaveAttribute(
      "href",
      "#main-content",
    );
    expect(screen.getByRole("complementary", { name: "Navegación principal" })).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Flujo de preparación de datos" })).toBeInTheDocument();
    expect(screen.getByRole("main")).toHaveAttribute("id", "main-content");
    expect(screen.getByRole("region", { name: "Etapa Cargar" })).toHaveAttribute("aria-busy", "false");

    fireEvent.click(screen.getByText("Licencia y privacidad"));
    const legalPanel = screen.getByRole("region", {
      name: "Licencia y privacidad de Columnia",
      description: /Columnia procesa los datos localmente/,
    });
    expect(legalPanel).toHaveAttribute("aria-labelledby", "legal-panel-title");
    expect(legalPanel).toHaveAttribute("aria-describedby", "legal-panel-summary");
  });

  it("carga y presenta el resumen de un CSV desde el runtime de escritorio", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia",
      version: "0.1.0",
      platform: "windows",
    });
    const loadSpy = mockDatasetLoad({
      fileName: "temperaturas.csv",
      fileSizeBytes: 2048,
      rowCount: 2,
      columnCount: 2,
      columns: [
        { name: "city", dataType: "String" },
        { name: "temperature", dataType: "Int64" },
      ],
      rows: [
        ["Santo Domingo", "30"],
        ["Santiago", null],
      ],
    });

    render(<App />);
    const button = await screen.findByRole("button", { name: "Seleccionar dataset" });
    fireEvent.click(button);
    const headerDialog = await screen.findByRole("dialog", { name: "Revisar encabezados de temperaturas.csv" });
    expect(loadSpy).not.toHaveBeenCalled();
    fireEvent.click(within(headerDialog).getByRole("radio", { name: /Conservar la primera fila como datos/ }));
    fireEvent.click(within(headerDialog).getByText("Interpretación de fechas y números (opcional)"));
    fireEvent.change(within(headerDialog).getByRole("combobox", { name: "Fechas" }), { target: { value: "dmy" } });
    fireEvent.change(within(headerDialog).getByRole("combobox", { name: "Números" }), { target: { value: "commaDecimalDotGrouping" } });
    fireEvent.click(within(headerDialog).getByRole("button", { name: "Revisar esquema" }));
    fireEvent.click(await within(headerDialog).findByRole("button", { name: "Cargar archivo" }));

    expect(await screen.findByRole("heading", { name: "temperaturas.csv" })).toBeInTheDocument();
    expect(loadSpy).toHaveBeenCalledWith(
      "selection-test", null, "generated", expect.any(Function), null, "dmy", "commaDecimalDotGrouping",
    );
    expect(screen.getByRole("progressbar", { name: "Progreso del flujo" })).toHaveAttribute("aria-valuetext", "Paso 2 de 4: Revisar");
    expect(await screen.findByRole("button", { name: /Continuar a Preparar|Empezar con la prioridad principal/ })).toBeEnabled();
    expect(screen.queryByRole("button", { name: "Seleccionar dataset" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Exportar CSV" })).not.toBeInTheDocument();
    expect(screen.getByText("2.0 KB")).toBeInTheDocument();
    const diagnosisTab = screen.getByRole("tab", { name: "Diagnóstico" });
    const previewTab = screen.getByRole("tab", { name: "Vista previa" });
    expect(diagnosisTab).toHaveAttribute("aria-controls", "review-diagnosis-panel");
    expect(diagnosisTab).toHaveAttribute("tabindex", "0");
    diagnosisTab.focus();
    fireEvent.keyDown(diagnosisTab, { key: "ArrowRight" });
    expect(previewTab).toHaveFocus();
    expect(previewTab).toHaveAttribute("aria-selected", "true");
    expect(previewTab).toHaveAttribute("tabindex", "0");
    expect(screen.getByRole("tabpanel", { name: "Vista previa" })).toHaveAttribute(
      "id",
      "review-preview-panel",
    );
    expect(screen.getByRole("cell", { name: "Santo Domingo" })).toBeInTheDocument();
    expect(screen.getByText("null")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Cargar" }));
    expect(screen.getByRole("button", { name: "Seleccionar otro dataset" })).toBeInTheDocument();
    expect(screen.getByText(/Se admiten CSV, TSV, TXT delimitado, JSON, Parquet, Excel y ODS sin un límite fijo de tamaño/)).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "temperaturas.csv" })).toBeInTheDocument();
  });

  it("requiere confirmar la estimación antes de materializar una fuente grande", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia",
      version: "0.26.0",
      platform: "windows",
    });
    const fileSizeBytes = 512 * 1024 * 1024;
    vi.spyOn(bridge, "pickDatasetSource").mockResolvedValue({
      selectionId: "large-source-selection",
      fileName: "clientes-grande.csv",
      fileSizeBytes,
      format: "csv",
      sheets: [],
      defaultSheetId: null,
      isCompressedContainer: false,
      resourceEstimate: resourceEstimate(fileSizeBytes, "sourceBacked"),
    });
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(historyState({
      canUndo: false,
      entryCount: 1,
      currentIndex: 0,
      entries: [{ id: "history-test-0", index: 0, label: "Dataset cargado", isCurrent: true }],
    }));
    const loadSpy = vi.spyOn(bridge, "loadDatasetSelection").mockResolvedValue({
      fileName: "clientes-grande.csv",
      fileSizeBytes,
      rowCount: 1,
      columnCount: 1,
      columns: [{ name: "cliente_id", dataType: "String" }],
      rows: [["00123"]],
    });

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    const dialog = await screen.findByRole("dialog", { name: "Revisar encabezados de clientes-grande.csv" });
    expect(within(dialog).getByText(/Lectura source-backed/)).toBeInTheDocument();
    expect(loadSpy).not.toHaveBeenCalled();

    fireEvent.click(within(dialog).getByRole("button", { name: "Revisar esquema" }));
    fireEvent.click(await within(dialog).findByRole("button", { name: "Cargar archivo" }));
    expect(await screen.findByRole("heading", { name: "clientes-grande.csv" })).toBeInTheDocument();
    expect(loadSpy).toHaveBeenCalledWith("large-source-selection", null, "firstRow", expect.any(Function), null, null, null);
  });

  it("deja revisar valores con ceros iniciales y columnas ambiguas en la vista previa", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia",
      version: "0.26.0",
      platform: "windows",
    });
    mockDatasetLoad({
      fileName: "identificadores.csv",
      fileSizeBytes: 80,
      rowCount: 1,
      columnCount: 3,
      columns: [
        { name: "identificador", dataType: "String" },
        { name: "total", dataType: "String" },
        { name: "total_duplicated_0", dataType: "String" },
      ],
      rows: [["00123", "1.00", "2.50"]],
    });

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    fireEvent.click(await screen.findByRole("tab", { name: "Vista previa" }));

    expect(screen.getByRole("columnheader", { name: /identificador/ })).toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: /^total / })).toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: /total_duplicated_0/ })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "00123" })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "1.00" })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "2.50" })).toBeInTheDocument();
  });

  it("navega por páginas usando el dataset activo en Rust", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia",
      version: "0.1.0",
      platform: "windows",
    });
    mockDatasetLoad({
      fileName: "ciudades.csv",
      fileSizeBytes: 4096,
      rowCount: 75,
      columnCount: 1,
      columns: [{ name: "city", dataType: "String" }],
      rows: [["Santo Domingo"]],
    });
    const pageSpy = vi.spyOn(bridge, "getDatasetPage").mockResolvedValue({
      offset: 50,
      rows: [["Puerto Plata"]],
    });

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    fireEvent.click(await screen.findByRole("tab", { name: "Vista previa" }));
    fireEvent.click(await screen.findByRole("button", { name: "Siguiente" }));

    expect(await screen.findByRole("cell", { name: "Puerto Plata" })).toBeInTheDocument();
    expect(screen.getByText(/Filas 51–51 de 75/)).toBeInTheDocument();
    expect(pageSpy).toHaveBeenCalledWith(50, 50);
    expect(screen.getByRole("button", { name: "Anterior" })).toBeEnabled();
  });

  it("muestra el progreso recibido durante la carga y el análisis", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia",
      version: "0.2.0",
      platform: "windows",
    });

    let resolveLoad!: (dataset: DatasetPreview) => void;
    const loadPromise = new Promise<DatasetPreview>((resolve) => {
      resolveLoad = resolve;
    });
    vi.spyOn(bridge, "pickDatasetSource").mockResolvedValue({
      selectionId: "selection-progress", fileName: "progreso.csv", fileSizeBytes: 128,
      format: "csv", sheets: [], defaultSheetId: null,
      isCompressedContainer: false,
      resourceEstimate: resourceEstimate(128),
    });
    vi.spyOn(bridge, "loadDatasetSelection").mockImplementation((_selectionId, _sheetId, _headerMode, onProgress) => {
      onProgress?.({ operation: "load", stage: "Leyendo y detectando columnas", percent: 25 });
      return loadPromise;
    });

    let resolveProfile!: (profile: DatasetProfile) => void;
    const profilePromise = new Promise<DatasetProfile>((resolve) => {
      resolveProfile = resolve;
    });
    vi.spyOn(bridge, "getDatasetProfile").mockImplementation((onProgress) => {
      onProgress?.({ operation: "profile", stage: "Analizando columnas", percent: 60 });
      return profilePromise;
    });

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));

    expect(
      await screen.findByRole("progressbar", { name: "Progreso: Leyendo y detectando columnas" }),
    ).toHaveAttribute("value", "25");

    resolveLoad({
      fileName: "progreso.csv",
      fileSizeBytes: 128,
      rowCount: 1,
      columnCount: 1,
      columns: [{ name: "value", dataType: "Int64" }],
      rows: [["1"]],
    });
    await screen.findByRole("heading", { name: "progreso.csv" });
    expect(
      await screen.findByRole("progressbar", { name: "Progreso: Analizando columnas" }),
    ).toHaveAttribute("value", "60");

    resolveProfile({
      rowCount: 1,
      duplicateRowCount: 0,
      nearDuplicateRowCount: 0,
      duplicatePercentage: 0,
      columns: [],
    });
    expect(await screen.findByRole("button", { name: /Continuar a Preparar|Empezar con la prioridad principal/ })).toBeInTheDocument();
  });

  it("conserva el dataset activo y permite reintentar si falla la cancelación de una sustitución", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia",
      version: "0.4.0",
      platform: "windows",
    });
    const activeDataset: DatasetPreview = {
      fileName: "activo.csv",
      fileSizeBytes: 128,
      rowCount: 1,
      columnCount: 1,
      columns: [{ name: "value", dataType: "Int64" }],
      rows: [["1"]],
    };
    let rejectReplacement!: (reason: unknown) => void;
    const replacementPromise = new Promise<DatasetPreview>((_resolve, reject) => {
      rejectReplacement = reject;
    });
    vi.spyOn(bridge, "pickDatasetSource")
      .mockResolvedValueOnce({ selectionId: "selection-active", fileName: "activo.csv", fileSizeBytes: 128, format: "csv", sheets: [], defaultSheetId: null, isCompressedContainer: false, resourceEstimate: resourceEstimate(128) })
      .mockResolvedValueOnce({ selectionId: "selection-replacement", fileName: "nuevo.csv", fileSizeBytes: 128, format: "csv", sheets: [], defaultSheetId: null, isCompressedContainer: false, resourceEstimate: resourceEstimate(128) });
    vi.spyOn(bridge, "loadDatasetSelection")
      .mockResolvedValueOnce(activeDataset)
      .mockImplementationOnce((_selectionId, _sheetId, _headerMode, onProgress) => {
        onProgress?.({ operation: "load", stage: "Leyendo y detectando columnas", percent: 25 });
        return replacementPromise;
      });
    const cancelSpy = vi.spyOn(bridge, "cancelOperation")
      .mockRejectedValueOnce(new Error("bridge unavailable"))
      .mockResolvedValue(undefined);

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    const activeHeaderDialog = await screen.findByRole("dialog", { name: "Revisar encabezados de activo.csv" });
    fireEvent.click(within(activeHeaderDialog).getByRole("button", { name: "Revisar esquema" }));
    fireEvent.click(await within(activeHeaderDialog).findByRole("button", { name: "Cargar archivo" }));
    await screen.findByRole("heading", { name: "activo.csv" });
    fireEvent.click(screen.getByRole("button", { name: "Cargar" }));
    fireEvent.click(screen.getByRole("button", { name: "Seleccionar otro dataset" }));
    const replacementHeaderDialog = await screen.findByRole("dialog", { name: "Revisar encabezados de nuevo.csv" });
    fireEvent.click(within(replacementHeaderDialog).getByRole("button", { name: "Revisar esquema" }));
    fireEvent.click(await within(replacementHeaderDialog).findByRole("button", { name: "Cargar archivo" }));
    fireEvent.click(await screen.findByRole("button", { name: "Cancelar" }));

    expect(cancelSpy).toHaveBeenNthCalledWith(1, "load");
    expect(await screen.findByText("No se pudo solicitar la cancelación: bridge unavailable")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Cargando dataset" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(await screen.findByRole("button", { name: "Cancelando…" })).toBeDisabled();
    expect(cancelSpy).toHaveBeenNthCalledWith(2, "load");
    rejectReplacement("Operación cancelada por el usuario.");
    expect(await screen.findByRole("heading", { name: "activo.csv" })).toBeInTheDocument();
  });

  it("exporta el dataset activo sin solicitar una ruta en la interfaz", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia",
      version: "0.5.0",
      platform: "windows",
    });
    mockDatasetLoad({
      fileName: "ventas.csv",
      fileSizeBytes: 128,
      rowCount: 1,
      columnCount: 1,
      columns: [{ name: "total", dataType: "Int64" }],
      rows: [["100"]],
    });
    const exportSpy = vi.spyOn(bridge, "exportDataset").mockImplementation(
      async (format, qualityRules, allowUnvalidated, onProgress) => {
        onProgress?.({ operation: "export", stage: "Escribiendo dataset", percent: 25 });
        expect(format).toBe("parquet");
        expect(qualityRules).toEqual([]);
        expect(allowUnvalidated).toBe(true);
        return {
          fileName: "ventas-columnia.parquet",
          fileSizeBytes: 2048,
          format: "Parquet",
          protectedColumnCount: 0,
          protectedColumns: [],
        };
      },
    );

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await screen.findByRole("heading", { name: "ventas.csv" });
    expect(screen.queryByRole("button", { name: "Exportar Parquet" })).not.toBeInTheDocument();
    await switchPhase("Entregar");
    fireEvent.change(screen.getByRole("combobox", { name: "Formato de exportación" }), {
      target: { value: "parquet" },
    });
    expect(screen.getByRole("button", { name: "Exportar Parquet" })).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox", { name: /Confirmo que quiero exportar sin validar/ }));
    fireEvent.click(screen.getByRole("button", { name: "Exportar Parquet" }));

    expect(await screen.findByRole("heading", { name: "Copia lista" })).toBeInTheDocument();
    expect(screen.getByText("ventas-columnia.parquet")).toBeInTheDocument();
    expect(screen.getByText(/2\.0 KB/)).toBeInTheDocument();
    expect(exportSpy).toHaveBeenCalledOnce();
    expect(screen.queryByLabelText("Ruta de exportación")).not.toBeInTheDocument();
  });

  it("ignora un segundo clic de exportación mientras la primera entrega sigue pendiente", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.5.0", platform: "windows" });
    mockDatasetLoad({
      fileName: "ventas.csv",
      fileSizeBytes: 128,
      rowCount: 1,
      columnCount: 1,
      columns: [{ name: "total", dataType: "Int64" }],
      rows: [["100"]],
    });
    let resolveExport!: (value: null) => void;
    const exportPromise = new Promise<null>((resolve) => { resolveExport = resolve; });
    const exportSpy = vi.spyOn(bridge, "exportDataset").mockReturnValue(exportPromise);

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await screen.findByRole("heading", { name: "ventas.csv" });
    await switchPhase("Entregar");
    fireEvent.click(screen.getByRole("checkbox", { name: "Confirmo que quiero exportar sin validar la calidad" }));
    const exportButton = screen.getByRole("button", { name: "Exportar CSV" });
    act(() => {
      fireEvent.click(exportButton);
      fireEvent.click(exportButton);
    });

    expect(exportSpy).toHaveBeenCalledOnce();
    resolveExport(null);
    await waitFor(() => expect(exportButton).toBeEnabled());
  });

  it("valida un contrato aprobado y envía sus reglas al exportar", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.24.0", platform: "windows" });
    mockDatasetLoad({
      fileName: "ventas.csv", fileSizeBytes: 128, rowCount: 10, columnCount: 1,
      columns: [{ name: "total", dataType: "Int64" }], rows: [["100"]],
    });
    const validationSpy = vi.spyOn(bridge, "validateQualityRules").mockResolvedValue({
      passed: true, rowCount: 10, totalRules: 1, failedRules: 0,
      rules: [{ column: "total", kind: "not_null", maxInvalid: 0, checkedCount: 10, invalidCount: 0, invalidPct: 0, passed: true }],
    });
    const exportSpy = vi.spyOn(bridge, "exportDataset").mockResolvedValue(null);

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await screen.findByRole("heading", { name: "Revisa antes de modificar" });
    await openQualityAndAnalyze();
    await switchPhase("Entregar");
    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    fireEvent.click(screen.getByRole("button", { name: "Validar y exportar CSV" }));

    expect(await screen.findByText("Contrato aprobado")).toBeInTheDocument();
    expect(validationSpy).toHaveBeenCalledWith([{ column: "total", kind: "not_null", maxInvalid: 0 }]);
    await waitFor(() => expect(exportSpy).toHaveBeenCalledWith(
      "csv", [{ column: "total", kind: "not_null", maxInvalid: 0 }], false, expect.any(Function), "none",
    ));
  });

  it("bloquea la exportación cuando el contrato falla o cambia después de validarse", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.24.0", platform: "windows" });
    mockDatasetLoad({
      fileName: "clientes.csv", fileSizeBytes: 128, rowCount: 4, columnCount: 1,
      columns: [{ name: "correo", dataType: "String" }], rows: [[null]],
    });
    vi.spyOn(bridge, "validateQualityRules")
      .mockResolvedValueOnce({
        passed: false, rowCount: 4, totalRules: 1, failedRules: 1,
        rules: [{ column: "correo", kind: "not_null", maxInvalid: 0, checkedCount: 4, invalidCount: 1, invalidPct: 25, passed: false }],
      })
      .mockResolvedValueOnce({
        passed: true, rowCount: 4, totalRules: 1, failedRules: 0,
        rules: [{ column: "correo", kind: "not_null", maxInvalid: 0, checkedCount: 4, invalidCount: 0, invalidPct: 0, passed: true }],
      });
    const exportSpy = vi.spyOn(bridge, "exportDataset").mockResolvedValue(null);

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await openQualityAndAnalyze();
    await switchPhase("Entregar");
    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    fireEvent.click(screen.getByRole("button", { name: "Validar y exportar CSV" }));
    expect(await screen.findByText("Contrato fallido")).toBeInTheDocument();
    expect(exportSpy).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Validar y exportar CSV" }));
    expect(await screen.findByText("Contrato aprobado")).toBeInTheDocument();
    await waitFor(() => expect(exportSpy).toHaveBeenCalledTimes(1));
    fireEvent.change(screen.getByRole("spinbutton", { name: "Inválidos máximos regla 1" }), { target: { value: "1" } });
    expect(screen.getByText("Resultado desactualizado")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Validar y exportar CSV" })).toBeEnabled();
  });

  it("normaliza los nombres de columnas desde Preparar", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia",
      version: "0.7.0",
      platform: "windows",
    });
    const original: DatasetPreview = {
      fileName: "ventas.csv",
      fileSizeBytes: 128,
      rowCount: 1,
      columnCount: 1,
      columns: [{ name: "Año Venta", dataType: "Int64" }],
      rows: [["2026"]],
    };
    mockDatasetLoad(original);
    const normalizeSpy = vi.spyOn(bridge, "applySafeCorrections").mockResolvedValue({
      dataset: {
        ...original,
        columns: [{ name: "ano_venta", dataType: "Int64" }],
      },
      changedCellCount: 0,
      affectedRowCount: 0,
      removedRowCount: 0,
      renamedColumnCount: 1,
      renames: [{ from: "Año Venta", to: "ano_venta" }],
    });

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("checkbox", { name: /Normalizar nombres de las 1 columnas/ }));
    fireEvent.click(screen.getByRole("button", { name: "Aplicar plan seleccionado" }));

    expect(await screen.findByText("Plan aplicado: 1 columna renombrada.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Deshacer" })).toBeInTheDocument();
    expect(within(screen.getByRole("button", { name: "Preparar" })).getByText("Hecho")).toBeInTheDocument();
    expect(normalizeSpy).toHaveBeenCalledWith({ trimText: false, normalizeSentinels: false, normalizeColumnNames: true, removeDuplicates: false });

    await switchPhase("Revisar");
    fireEvent.click(screen.getByRole("tab", { name: "Vista previa" }));
    expect(screen.getByRole("columnheader", { name: /ano_venta/ })).toBeInTheDocument();
  });

  it("recorta espacios y normaliza columnas de texto seleccionadas", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia",
      version: "0.8.0",
      platform: "windows",
    });
    const original: DatasetPreview = {
      fileName: "clientes.csv",
      fileSizeBytes: 128,
      rowCount: 1,
      columnCount: 2,
      columns: [
        { name: "city", dataType: "String" },
        { name: "code", dataType: "String" },
      ],
      rows: [[" Bogotá ", "A1"]],
    };
    mockDatasetLoad(original);
    const trimSpy = vi.spyOn(bridge, "applySafeCorrections").mockResolvedValue({
      dataset: { ...original, rows: [["Bogotá", "A1"]] },
      affectedRowCount: 1,
      changedCellCount: 1,
      removedRowCount: 0,
      renamedColumnCount: 0,
      renames: [],
    });
    const normalizeSpy = vi.spyOn(bridge, "normalizeTextValues").mockResolvedValue({
      dataset: { ...original, rows: [["bogota", "A1"]] },
      affectedRowCount: 1,
      changedCellCount: 1,
      changedColumns: [{ name: "city", changedCellCount: 1 }],
    });

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("button", { name: "Aplicar plan seleccionado" }));

    expect(await screen.findByText("Plan aplicado: 1 celda actualizada.")).toBeInTheDocument();
    expect(trimSpy).toHaveBeenCalledWith({ trimText: true, normalizeSentinels: false, normalizeColumnNames: false, removeDuplicates: false });

    fireEvent.click(screen.getByRole("checkbox", { name: "city" }));
    fireEvent.click(screen.getByRole("button", { name: "Normalizar texto seleccionado" }));

    expect(await screen.findByText("Se normalizó texto en 1 celda en 1 fila.")).toBeInTheDocument();
    expect(normalizeSpy).toHaveBeenCalledWith(["city"], true);
  });

  it("aplica las correcciones recomendadas como una sola revisión", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia",
      version: "0.9.0",
      platform: "windows",
    });
    const original: DatasetPreview = {
      fileName: "lote.csv",
      fileSizeBytes: 128,
      rowCount: 1,
      columnCount: 1,
      columns: [{ name: "Ciudad Nombre", dataType: "String" }],
      rows: [[" Santo Domingo "]],
    };
    mockDatasetLoad(original);
    const applySpy = vi.spyOn(bridge, "applySafeCorrections").mockResolvedValue({
      dataset: {
        ...original,
        columns: [{ name: "ciudad_nombre", dataType: "String" }],
        rows: [["Santo Domingo"]],
      },
      changedCellCount: 1,
      affectedRowCount: 1,
      removedRowCount: 0,
      renamedColumnCount: 1,
      renames: [{ from: "Ciudad Nombre", to: "ciudad_nombre" }],
    });

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("checkbox", { name: /Normalizar nombres de las 1 columnas/ }));
    fireEvent.click(screen.getByRole("button", { name: "Aplicar plan seleccionado" }));

    expect(
      await screen.findByText(/Plan aplicado: 1 celda actualizada y 1 columna renombrada/),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Deshacer" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Rehacer" })).toBeDisabled();
    expect(applySpy).toHaveBeenCalledWith({ trimText: true, normalizeSentinels: false, normalizeColumnNames: true, removeDuplicates: false });
  });

  it("calcula y presenta el perfil de calidad del dataset", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia",
      version: "0.1.0",
      platform: "windows",
    });
    mockDatasetLoad({
      fileName: "calidad.csv",
      fileSizeBytes: 1024,
      rowCount: 3,
      columnCount: 1,
      columns: [{ name: "temperature", dataType: "Int64" }],
      rows: [["30"], [null], ["28"]],
    });
    const profileSpy = vi.spyOn(bridge, "getDatasetProfile").mockResolvedValue({
      rowCount: 3,
      duplicateRowCount: 1,
      nearDuplicateRowCount: 0,
      duplicatePercentage: 33.333,
      columns: [
        {
          name: "temperature",
          dataType: "Int64",
          nullCount: 1,
          completenessPercentage: 66.666,
          uniqueCount: 2,
          minimum: "28",
          maximum: "30",
          mean: 29,
          emptyCount: null,
          minimumLength: null,
          maximumLength: null,
          averageLength: null,
          suggestedType: null,
          typeMatchPercentage: null,
          invalidTypeCount: null,
          sentinelCount: null,
          encodingIssueCount: null,
          privacySignal: null,
          standardDeviation: 1.414,
          firstQuartile: 28.5,
          median: 29,
          thirdQuartile: 29.5,
          outlierCount: 0,
          histogram: null,
        },
      ],
    });
    const safeCorrectionSpy = vi.spyOn(bridge, "applySafeCorrections").mockResolvedValue({
      dataset: {
        fileName: "calidad.csv",
        fileSizeBytes: 1024,
        rowCount: 2,
        columnCount: 1,
        columns: [{ name: "temperature", dataType: "Int64" }],
        rows: [["30"], ["28"]],
      },
      changedCellCount: 0,
      affectedRowCount: 0,
      removedRowCount: 1,
      renamedColumnCount: 0,
      renames: [],
    });
    const undoSpy = vi.spyOn(bridge, "undoLastChange").mockResolvedValue({
      dataset: {
        fileName: "calidad.csv",
        fileSizeBytes: 1024,
        rowCount: 3,
        columnCount: 1,
        columns: [{ name: "temperature", dataType: "Int64" }],
        rows: [["30"], [null], ["28"]],
      },
      history: historyState({ canUndo: false, canRedo: true, currentIndex: 0 }),
      message: "Se deshizo el último cambio.",
    });
    const redoSpy = vi.spyOn(bridge, "redoLastChange").mockResolvedValue({
      dataset: {
        fileName: "calidad.csv",
        fileSizeBytes: 1024,
        rowCount: 2,
        columnCount: 1,
        columns: [{ name: "temperature", dataType: "Int64" }],
        rows: [["30"], ["28"]],
      },
      history: historyState({ canUndo: true, canRedo: false, currentIndex: 1 }),
      message: "Se rehízo el último cambio.",
    });

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await openQualityAndAnalyze();

    const generalProfile = await screen.findByRole("region", {
      name: "Perfil de calidad por columna",
    });
    expect(within(generalProfile).getByRole("rowheader", { name: /temperature/ })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "66.7%" })).toBeInTheDocument();
    expect(screen.getByText("1 (33.3%)")).toBeInTheDocument();
    expect(profileSpy).toHaveBeenCalledOnce();

    await switchPhase("Preparar");
    expect(screen.getByRole("checkbox", { name: /Retirar 1 fila duplicada exacta/ })).toBeChecked();
    fireEvent.click(screen.getByRole("button", { name: "Aplicar plan seleccionado" }));
    expect(await screen.findByText(/^Plan aplicado:/)).toBeInTheDocument();
    expect(safeCorrectionSpy).toHaveBeenCalledWith({
      trimText: false, normalizeSentinels: false, normalizeColumnNames: false, removeDuplicates: true,
    });
    await waitFor(() => expect(profileSpy).toHaveBeenCalledTimes(2));

    fireEvent.click(screen.getByRole("button", { name: "Deshacer" }));
    await waitFor(() => expect(profileSpy).toHaveBeenCalledTimes(3));
    expect(undoSpy).toHaveBeenCalledOnce();

    fireEvent.click(screen.getByRole("button", { name: "Rehacer" }));
    expect(await screen.findByText("Se rehízo el último cambio.")).toBeInTheDocument();
    await waitFor(() => expect(profileSpy).toHaveBeenCalledTimes(4));
    expect(redoSpy).toHaveBeenCalledOnce();
  });

  it("presenta las métricas específicas de columnas de texto", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia",
      version: "0.1.0",
      platform: "windows",
    });
    mockDatasetLoad({
      fileName: "texto.csv",
      fileSizeBytes: 128,
      rowCount: 2,
      columnCount: 1,
      columns: [{ name: "city", dataType: "String" }],
      rows: [["Santo Domingo"], ["  "]],
    });
    vi.spyOn(bridge, "getDatasetProfile").mockResolvedValue({
      rowCount: 2,
      duplicateRowCount: 0,
      nearDuplicateRowCount: 0,
      duplicatePercentage: 0,
      columns: [
        {
          name: "city",
          dataType: "String",
          nullCount: 0,
          completenessPercentage: 100,
          uniqueCount: 2,
          minimum: null,
          maximum: null,
          mean: null,
          emptyCount: 1,
          minimumLength: 2,
          maximumLength: 13,
          averageLength: 7.5,
          suggestedType: null,
          typeMatchPercentage: null,
          invalidTypeCount: null,
          sentinelCount: null,
          encodingIssueCount: null,
          privacySignal: null,
          standardDeviation: null,
          firstQuartile: null,
          median: null,
          thirdQuartile: null,
          outlierCount: null,
          histogram: null,
        },
      ],
    });

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await openQualityAndAnalyze();

    expect(await screen.findByRole("region", { name: "Perfil de columnas de texto" })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "7.5" })).toBeInTheDocument();
  });

  it("muestra una sugerencia conservadora de tipo para texto", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia",
      version: "0.1.0",
      platform: "windows",
    });
    mockDatasetLoad({
      fileName: "fechas.csv",
      fileSizeBytes: 256,
      rowCount: 10,
      columnCount: 1,
      columns: [{ name: "date_added", dataType: "String" }],
      rows: [["September 9, 2019"]],
    });
    vi.spyOn(bridge, "getDatasetProfile").mockResolvedValue({
      rowCount: 10,
      duplicateRowCount: 0,
      nearDuplicateRowCount: 0,
      duplicatePercentage: 0,
      columns: [
        {
          name: "date_added",
          dataType: "String",
          nullCount: 0,
          completenessPercentage: 100,
          uniqueCount: 10,
          minimum: null,
          maximum: null,
          mean: null,
          emptyCount: 0,
          minimumLength: 7,
          maximumLength: 18,
          averageLength: 16,
          suggestedType: "date",
          typeMatchPercentage: 90,
          invalidTypeCount: 1,
          sentinelCount: null,
          encodingIssueCount: null,
          privacySignal: null,
          standardDeviation: null,
          firstQuartile: null,
          median: null,
          thirdQuartile: null,
          outlierCount: null,
          histogram: null,
        },
      ],
    });

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await openQualityAndAnalyze();

    expect(await screen.findByRole("cell", { name: "Fecha" })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "90.0%" })).toBeInTheDocument();
  });

  it("presenta cuartiles y outliers en una tabla numérica separada", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia",
      version: "0.1.0",
      platform: "windows",
    });
    mockDatasetLoad({
      fileName: "numeros.csv",
      fileSizeBytes: 128,
      rowCount: 5,
      columnCount: 1,
      columns: [{ name: "value", dataType: "Int64" }],
      rows: [["10"], ["11"], ["12"], ["13"], ["100"]],
    });
    vi.spyOn(bridge, "getDatasetProfile").mockResolvedValue({
      rowCount: 5,
      duplicateRowCount: 0,
      nearDuplicateRowCount: 0,
      duplicatePercentage: 0,
      columns: [
        {
          name: "value",
          dataType: "Int64",
          nullCount: 0,
          completenessPercentage: 100,
          uniqueCount: 5,
          minimum: "10",
          maximum: "100",
          mean: 29.2,
          emptyCount: null,
          minimumLength: null,
          maximumLength: null,
          averageLength: null,
          suggestedType: null,
          typeMatchPercentage: null,
          invalidTypeCount: null,
          sentinelCount: null,
          encodingIssueCount: null,
          privacySignal: null,
          standardDeviation: 39.592,
          firstQuartile: 11,
          median: 12,
          thirdQuartile: 13,
          outlierCount: 1,
          histogram: null,
        },
      ],
    });

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await openQualityAndAnalyze();

    expect(await screen.findByRole("region", { name: "Perfil de columnas numéricas" })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "39.592" })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "1" })).toBeInTheDocument();
  });

  it("permite elegir una hoja de Excel sin exponer la ruta al frontend", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia", version: "0.11.0", platform: "windows",
    });
    vi.spyOn(bridge, "pickDatasetSource").mockResolvedValue({
      selectionId: "opaque-workbook-1",
      fileName: "ventas.xlsx",
      fileSizeBytes: 4096,
      format: "excel",
      sheets: [{ id: "0", name: "Resumen" }, { id: "1", name: "Ventas 2026" }],
      defaultSheetId: "0",
      isCompressedContainer: true,
      resourceEstimate: resourceEstimate(4096),
    });
    vi.spyOn(bridge, "inspectWorkbookSheets").mockResolvedValue([
      { id: "0", name: "Resumen" },
      { id: "1", name: "Ventas 2026" },
    ]);
    const loadSpy = vi.spyOn(bridge, "loadDatasetSelection").mockResolvedValue({
      fileName: "ventas.xlsx",
      fileSizeBytes: 4096,
      rowCount: 1,
      columnCount: 1,
      columns: [{ name: "total", dataType: "Float64" }],
      rows: [["125.5"]],
    });

    render(<App />);
    const selectDataset = await screen.findByRole("button", { name: "Seleccionar dataset" });
    selectDataset.focus();
    fireEvent.click(selectDataset);
    const dialog = await screen.findByRole("dialog", { name: /Elegir hoja de ventas.xlsx/ });
    expect(within(dialog).getByRole("option", { name: "Ventas 2026" })).toBeInTheDocument();
    expect(within(dialog).getByText(/ocupar bastante más memoria/)).toHaveAttribute("role", "note");
    const sheetSelect = within(dialog).getByLabelText("Hoja");
    const reviewSchema = within(dialog).getByRole("button", { name: "Revisar esquema" });
    expect(sheetSelect).toHaveFocus();
    reviewSchema.focus();
    fireEvent.keyDown(reviewSchema, { key: "Tab" });
    expect(sheetSelect).toHaveFocus();
    fireEvent.keyDown(sheetSelect, { key: "Tab", shiftKey: true });
    expect(reviewSchema).toHaveFocus();
    expect(loadSpy).not.toHaveBeenCalled();
    fireEvent.change(sheetSelect, { target: { value: "1" } });
    fireEvent.click(within(dialog).getByRole("radio", { name: /Generar encabezados/ }));
    fireEvent.click(reviewSchema);
    fireEvent.click(await within(dialog).findByRole("button", { name: "Cargar hoja" }));

    expect(await screen.findByRole("heading", { name: "ventas.xlsx" })).toBeInTheDocument();
    expect(loadSpy).toHaveBeenCalledWith("opaque-workbook-1", "1", "generated", expect.any(Function), null, null, null);
    expect(JSON.stringify(loadSpy.mock.calls)).not.toContain("C:\\\\");
  });

  it("construye y aplica una receta estructural mediante una sola operación atómica", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({
      name: "Columnia", version: "0.15.0", platform: "windows",
    });
    const original: DatasetPreview = {
      fileName: "ventas.csv",
      fileSizeBytes: 256,
      rowCount: 1,
      columnCount: 2,
      columns: [
        { name: "Total venta", dataType: "String" },
        { name: "fecha", dataType: "String" },
      ],
      rows: [["10.50", "31-12-2026"]],
    };
    mockDatasetLoad(original);
    const applySpy = vi.spyOn(bridge, "applyTransformRecipe").mockResolvedValue({
      dataset: {
        ...original,
        columns: [
          { name: "total", dataType: "Float64" },
          { name: "fecha", dataType: "Date" },
        ],
      },
      changed: true,
      renamedColumnCount: 1,
      convertedColumnCount: 1,
      parsedDateColumnCount: 1,
      removedRowCount: 0,
      calculatedColumnCount: 0,
      replacedCellCount: 0,
      droppedColumnCount: 0,
      splitColumnCount: 0, mergedColumnCount: 0, droppedSourceColumnCount: 0,
      adjustedOutlierCellCount: 0, outlierRemovedRowCount: 0, outlierColumnCount: 0,
      groupCount: 0, aggregatedColumnCount: 0, collapsedRowCount: 0,
      normalizedContactCellCount: 0, normalizedContactColumnCount: 0, extractedColumnCount: 0,
    });

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Preparar");

    const correctionsTab = screen.getByRole("tab", { name: "Correcciones" });
    const transformationsTab = screen.getByRole("tab", { name: "Transformaciones" });
    expect(correctionsTab).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(correctionsTab, { key: "ArrowRight" });
    expect(transformationsTab).toHaveAttribute("aria-selected", "true");

    fireEvent.change(screen.getByLabelText("Columna para renombrar 1"), {
      target: { value: "Total venta" },
    });
    fireEvent.change(screen.getByLabelText("Nuevo nombre 1"), {
      target: { value: "total" },
    });
    fireEvent.change(screen.getByLabelText("Columna para convertir 1"), {
      target: { value: "Total venta" },
    });
    fireEvent.change(screen.getByLabelText("Tipo destino 1"), {
      target: { value: "decimal" },
    });
    fireEvent.change(screen.getByLabelText("Columna de fecha 1"), {
      target: { value: "fecha" },
    });
    fireEvent.change(screen.getByLabelText("Formato de fecha 1"), {
      target: { value: "dmy" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Aplicar receta" }));

    expect(applySpy).toHaveBeenCalledOnce();
    expect(applySpy).toHaveBeenCalledWith({
      renames: [{ from: "Total venta", to: "total" }],
      casts: [{ column: "Total venta", target: "decimal" }],
      dateParses: [{ column: "fecha", format: "dmy", target: "date" }],
      filters: [],
      calculatedColumn: null,
      findReplace: null,
      keepColumns: null,
      splitColumn: null,
      mergeColumns: null,
      outlierTreatments: [],
      groupSummary: null,
      contactNormalizations: [], textExtractions: [],
    }, null);
    expect(await screen.findByText(/Receta aplicada: 1 renombres, 1 conversiones, 1 fechas interpretadas/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Deshacer" })).toBeEnabled();
  });

  it("guarda el borrador de receta por el bridge sin entregar rutas", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.23.0", platform: "windows" });
    const original: DatasetPreview = {
      fileName: "ventas.csv", fileSizeBytes: 256, rowCount: 1, columnCount: 2,
      columns: [{ name: "total", dataType: "Float64" }, { name: "estado", dataType: "String" }],
      rows: [["10", "pendiente"]],
    };
    mockDatasetLoad(original);
    const saveSpy = vi.spyOn(bridge, "saveTransformRecipe").mockResolvedValue({
      version: 2, name: "Limpieza ventas", savedAt: "2026-08-14T12:00:00Z", sourceSchema: original.columns,
      recipe: { renames: [{ from: "estado", to: "situacion" }], casts: [], dateParses: [], filters: [], calculatedColumn: null, findReplace: null, keepColumns: null, splitColumn: null, mergeColumns: null, outlierTreatments: [], groupSummary: null, contactNormalizations: [], textExtractions: [] },
    });

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("tab", { name: "Transformaciones" }));
    expect(screen.getByRole("button", { name: "Guardar receta" })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Columna para renombrar 1"), { target: { value: "estado" } });
    fireEvent.change(screen.getByLabelText("Nuevo nombre 1"), { target: { value: "situacion" } });
    fireEvent.change(screen.getByLabelText("Nombre de la receta"), { target: { value: "Limpieza ventas" } });
    fireEvent.click(screen.getByRole("button", { name: "Guardar receta" }));

    expect(await screen.findByText(/Receta guardada: Limpieza ventas/)).toBeInTheDocument();
    expect(saveSpy).toHaveBeenCalledWith(expect.objectContaining({ renames: [{ from: "estado", to: "situacion" }] }), "Limpieza ventas", [{ name: "estado", dataType: "String" }], null);
    expect(JSON.stringify(saveSpy.mock.calls)).not.toMatch(/path|\\\\/i);
    fireEvent.change(screen.getByLabelText("Nuevo nombre 1"), { target: { value: "estado_final" } });
    expect(screen.queryByText(/Receta guardada: Limpieza ventas/)).not.toBeInTheDocument();
  });

  it("carga una receta completa como borrador editable, confirma reemplazos y nunca la aplica", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.23.0", platform: "windows" });
    const original: DatasetPreview = {
      fileName: "ventas.csv", fileSizeBytes: 256, rowCount: 4, columnCount: 4,
      columns: [
        { name: "total", dataType: "Float64" }, { name: "estado", dataType: "String" },
        { name: "correo", dataType: "String" }, { name: "fecha", dataType: "String" },
      ], rows: [["10", "P-norte", "A@B.COM", "2026-08-14"]],
    };
    mockDatasetLoad(original);
    const fullRecipe: bridge.TransformRecipe = {
      renames: [{ from: "estado", to: "situacion" }], casts: [{ column: "total", target: "decimal" }],
      dateParses: [{ column: "fecha", format: "ymd", target: "date" }], filters: [{ column: "total", operator: "gte", value: "10" }],
      calculatedColumn: { name: "doble", source: "total", operation: "multiply", operand: { kind: "literal", value: "2" } },
      findReplace: { scope: "column", column: "estado", find: "P", replace: "Pendiente", regex: false }, keepColumns: ["total", "estado", "correo", "fecha"],
      splitColumn: { source: "estado", delimiter: "-", names: ["estado_base", "zona"], dropSource: false },
      mergeColumns: { sources: ["estado", "correo"], name: "contacto", separator: " ", dropSources: false },
      outlierTreatments: [{ column: "total", action: "cap" }], groupSummary: null,
      contactNormalizations: [{ column: "correo", kind: "email" }],
      textExtractions: [{ source: "estado", kind: "first_token", name: "estado_corto", delimiter: null }],
    };
    const pickSpy = vi.spyOn(bridge, "pickTransformRecipe")
      .mockResolvedValueOnce({ version: 1, name: "Receta completa", savedAt: "2026-08-14T12:00:00Z", recipe: fullRecipe })
      .mockResolvedValueOnce({ version: 1, name: "Otra receta", savedAt: "2026-08-14T12:01:00Z", recipe: { ...fullRecipe, renames: [{ from: "estado", to: "otro" }] } })
      .mockResolvedValueOnce(null);
    const applySpy = vi.spyOn(bridge, "applyTransformRecipe");
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("tab", { name: "Transformaciones" }));
    fireEvent.click(screen.getByRole("button", { name: "Cargar receta" }));

    expect(await screen.findByDisplayValue("Receta completa")).toBeInTheDocument();
    expect(screen.getByLabelText("Nuevo nombre 1")).toHaveValue("situacion");
    expect(screen.getByLabelText("Nombres de columnas divididas")).toHaveValue("estado_base, zona");
    expect(screen.getByLabelText("Nombre de extracción 1")).toHaveValue("estado_corto");
    expect(applySpy).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Cargar receta" }));
    await waitFor(() => expect(confirmSpy).toHaveBeenCalledOnce());
    expect(screen.getByLabelText("Nombre de la receta")).toHaveValue("Receta completa");
    expect(screen.getByLabelText("Nuevo nombre 1")).toHaveValue("situacion");
    fireEvent.click(screen.getByRole("button", { name: "Cargar receta" }));
    await waitFor(() => expect(pickSpy).toHaveBeenCalledTimes(3));
    expect(screen.getByLabelText("Nombre de la receta")).toHaveValue("Receta completa");
    expect(applySpy).not.toHaveBeenCalled();
  });

  it("presenta inline los errores al cargar una receta", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.23.0", platform: "windows" });
    mockDatasetLoad({ fileName: "datos.csv", fileSizeBytes: 10, rowCount: 1, columnCount: 1, columns: [{ name: "dato", dataType: "String" }], rows: [["a"]] });
    vi.spyOn(bridge, "pickTransformRecipe").mockRejectedValue(new Error("JSON inválido"));
    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("tab", { name: "Transformaciones" }));
    fireEvent.click(screen.getByRole("button", { name: "Cargar receta" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("No se pudo completar la operación: JSON inválido");
  });

  it("confirma filtros AND y combina una columna calculada en la misma receta", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.16.0", platform: "windows" });
    const original: DatasetPreview = {
      fileName: "pedidos.csv", fileSizeBytes: 300, rowCount: 20, columnCount: 3,
      columns: [{ name: "total", dataType: "Float64" }, { name: "estado", dataType: "String" }, { name: "categoria", dataType: "String" }],
      rows: [["10", "activo-norte", "A"]],
    };
    mockDatasetLoad(original);
    const applySpy = vi.spyOn(bridge, "applyTransformRecipe").mockResolvedValue({
      dataset: { ...original, rowCount: 12, columnCount: 3 },
      changed: true,
      renamedColumnCount: 0, convertedColumnCount: 0, parsedDateColumnCount: 0,
      removedRowCount: 8, calculatedColumnCount: 1,
      replacedCellCount: 0, droppedColumnCount: 0,
      splitColumnCount: 0, mergedColumnCount: 0, droppedSourceColumnCount: 0,
      adjustedOutlierCellCount: 0, outlierRemovedRowCount: 0, outlierColumnCount: 0,
      groupCount: 0, aggregatedColumnCount: 0, collapsedRowCount: 0,
      normalizedContactCellCount: 0, normalizedContactColumnCount: 0, extractedColumnCount: 0,
    });
    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("tab", { name: "Transformaciones" }));
    expect(screen.getByText(/comparaciones numéricas estrictas/)).toHaveTextContent(/literales ISO 8601/);
    fireEvent.click(screen.getByRole("button", { name: "+ Añadir filtro AND" }));
    fireEvent.change(screen.getByLabelText("Columna del filtro 1"), { target: { value: "estado" } });
    fireEvent.change(screen.getByLabelText("Operador del filtro 1"), { target: { value: "not_null" } });
    expect(screen.getByLabelText("Valor del filtro 1")).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox", { name: "Crear una columna en esta receta" }));
    fireEvent.change(screen.getByLabelText("Nombre de la columna calculada"), { target: { value: "doble" } });
    fireEvent.change(screen.getByLabelText("Columna para calcular"), { target: { value: "total" } });
    fireEvent.change(screen.getByLabelText("Operación calculada"), { target: { value: "multiply" } });
    fireEvent.change(screen.getByLabelText("Valor fijo del cálculo"), { target: { value: "2" } });
    fireEvent.click(screen.getByRole("checkbox", { name: "Añadir búsqueda y reemplazo" }));
    fireEvent.change(screen.getByLabelText("Columna para buscar"), { target: { value: "estado" } });
    fireEvent.change(screen.getByLabelText("Texto a buscar"), { target: { value: " " } });
    expect(screen.getByLabelText("Texto de reemplazo")).toHaveValue("");
    fireEvent.click(screen.getByRole("checkbox", { name: "Dividir una columna" }));
    fireEvent.change(screen.getByLabelText("Columna para dividir"), { target: { value: "estado" } });
    fireEvent.change(screen.getByLabelText("Delimitador para dividir"), { target: { value: "-" } });
    fireEvent.change(screen.getByLabelText("Nombres de columnas divididas"), { target: { value: "estado_base, zona" } });
    fireEvent.click(screen.getByRole("checkbox", { name: "Combinar columnas" }));
    const mergeGroup = screen.getByRole("group", { name: "Columnas para combinar" });
    fireEvent.click(within(mergeGroup).getByRole("checkbox", { name: "estado" }));
    fireEvent.click(within(mergeGroup).getByRole("checkbox", { name: "categoria" }));
    fireEvent.change(screen.getByLabelText("Nombre de columna combinada"), { target: { value: "estado_categoria" } });
    fireEvent.change(screen.getByLabelText("Separador para combinar"), { target: { value: "" } });
    fireEvent.click(screen.getByRole("checkbox", { name: "Eliminar las columnas originales" }));
    const applyRecipeButton = screen.getByRole("button", { name: "Aplicar receta" });
    applyRecipeButton.focus();
    fireEvent.click(applyRecipeButton);
    let dialog = screen.getByRole("alertdialog", { name: "Confirmar cambios de alto impacto" });
    expect(within(dialog).getByText(/1 filtros unidos por AND sobre 20 filas/)).toBeInTheDocument();
    expect(within(dialog).getByText(/En total se eliminarán 2 columnas originales/)).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Cancelar" })).toHaveFocus();
    expect(applySpy).not.toHaveBeenCalled();
    fireEvent.keyDown(dialog, { key: "Escape" });
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(applyRecipeButton).toHaveFocus();
    expect(applySpy).not.toHaveBeenCalled();
    fireEvent.click(applyRecipeButton);
    dialog = screen.getByRole("alertdialog", { name: "Confirmar cambios de alto impacto" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Confirmar y aplicar" }));
    expect(applySpy).toHaveBeenCalledOnce();
    expect(applySpy).toHaveBeenCalledWith(expect.objectContaining({
      filters: [{ column: "estado", operator: "not_null", value: null }],
      calculatedColumn: { name: "doble", source: "total", operation: "multiply", operand: { kind: "literal", value: "2" } },
      findReplace: { scope: "column", column: "estado", find: " ", replace: "", regex: false },
      keepColumns: null,
      splitColumn: { source: "estado", delimiter: "-", names: ["estado_base", "zona"], dropSource: false },
      mergeColumns: { sources: ["estado", "categoria"], name: "estado_categoria", separator: "", dropSources: true },
      outlierTreatments: [],
      groupSummary: null,
      contactNormalizations: [], textExtractions: [],
    }), null);
  });

  it("actualiza columnas de texto por conversiones y confirma dropSource por sí solo", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.18.0", platform: "windows" });
    const original: DatasetPreview = {
      fileName: "tipos.csv", fileSizeBytes: 100, rowCount: 3, columnCount: 2,
      columns: [{ name: "codigo", dataType: "Int64" }, { name: "descripcion", dataType: "String" }],
      rows: [["1", "A-B"]],
    };
    mockDatasetLoad(original);
    const applySpy = vi.spyOn(bridge, "applyTransformRecipe");
    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("tab", { name: "Transformaciones" }));

    fireEvent.change(screen.getByLabelText("Columna para convertir 1"), { target: { value: "codigo" } });
    fireEvent.click(screen.getByRole("button", { name: "+ Añadir conversión" }));
    fireEvent.change(screen.getByLabelText("Columna para convertir 2"), { target: { value: "descripcion" } });
    fireEvent.change(screen.getByLabelText("Tipo destino 2"), { target: { value: "integer" } });
    fireEvent.click(screen.getByRole("checkbox", { name: "Dividir una columna" }));
    const source = screen.getByLabelText("Columna para dividir");
    expect(within(source).getByRole("option", { name: "codigo" })).toBeInTheDocument();
    expect(within(source).queryByRole("option", { name: "descripcion" })).not.toBeInTheDocument();

    fireEvent.change(source, { target: { value: "codigo" } });
    fireEvent.change(screen.getByLabelText("Delimitador para dividir"), { target: { value: " " } });
    fireEvent.change(screen.getByLabelText("Nombres de columnas divididas"), { target: { value: "parte_1, parte_2" } });
    fireEvent.click(screen.getByRole("checkbox", { name: "Eliminar la columna original" }));
    fireEvent.click(screen.getByRole("button", { name: "Aplicar receta" }));

    const dialog = screen.getByRole("alertdialog", { name: "Confirmar cambios de alto impacto" });
    expect(within(dialog).getByText(/En total se eliminarán 1 columnas originales/)).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancelar" }));
    expect(applySpy).not.toHaveBeenCalled();
  });

  it("actualiza columnas numéricas por casts y confirma tratamientos de outliers", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.19.0", platform: "windows" });
    const original: DatasetPreview = {
      fileName: "metricas.csv", fileSizeBytes: 100, rowCount: 10, columnCount: 3,
      columns: [{ name: "importe", dataType: "String" }, { name: "cantidad", dataType: "Int64" }, { name: "nota", dataType: "Float64" }],
      rows: [["10", "2", "9"]],
    };
    mockDatasetLoad(original);
    const applySpy = vi.spyOn(bridge, "applyTransformRecipe");
    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("tab", { name: "Transformaciones" }));
    fireEvent.change(screen.getByLabelText("Columna para convertir 1"), { target: { value: "importe" } });
    fireEvent.change(screen.getByLabelText("Tipo destino 1"), { target: { value: "decimal" } });
    fireEvent.click(screen.getByRole("button", { name: "+ Añadir conversión" }));
    fireEvent.change(screen.getByLabelText("Columna para convertir 2"), { target: { value: "cantidad" } });
    fireEvent.change(screen.getByLabelText("Tipo destino 2"), { target: { value: "string" } });
    fireEvent.click(screen.getByRole("button", { name: "+ Añadir regla" }));
    const target = screen.getByLabelText("Columna de valores atípicos 1");
    expect(within(target).getByRole("option", { name: "importe" })).toBeInTheDocument();
    expect(within(target).queryByRole("option", { name: "cantidad" })).not.toBeInTheDocument();
    fireEvent.change(target, { target: { value: "importe" } });
    fireEvent.click(screen.getByRole("button", { name: "+ Añadir regla" }));
    fireEvent.change(screen.getByLabelText("Columna de valores atípicos 2"), { target: { value: "nota" } });
    fireEvent.change(screen.getByLabelText("Acción para valores atípicos 2"), { target: { value: "drop" } });
    fireEvent.click(screen.getByRole("button", { name: "Aplicar receta" }));
    let dialog = screen.getByRole("alertdialog", { name: "Confirmar cambios de alto impacto" });
    expect(within(dialog).getByText(/Se limitarán valores atípicos en 1 columnas/)).toBeInTheDocument();
    expect(within(dialog).getByText(/eliminar filas atípicas detectadas en 1 columnas/)).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancelar" }));
    expect(applySpy).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Aplicar receta" }));
    dialog = screen.getByRole("alertdialog", { name: "Confirmar cambios de alto impacto" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Confirmar y aplicar" }));
    expect(applySpy).toHaveBeenCalledWith(expect.objectContaining({ outlierTreatments: [{ column: "importe", action: "cap" }, { column: "nota", action: "drop" }] }), null);
  });

  it("no crea historial cuando el tratamiento IQR no encuentra outliers", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.19.0", platform: "windows" });
    const original: DatasetPreview = {
      fileName: "estable.csv", fileSizeBytes: 100, rowCount: 4, columnCount: 1,
      columns: [{ name: "valor", dataType: "Float64" }], rows: [["1"], ["2"], ["3"], ["4"]],
    };
    mockDatasetLoad(original);
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(historyState({ canUndo: false, canRedo: false, currentIndex: 0, entryCount: 1, entries: [{ id: "history-test-0", index: 0, label: "Dataset cargado", isCurrent: true }] }));
    vi.spyOn(bridge, "applyTransformRecipe").mockResolvedValue({
      dataset: original,
      changed: false,
      renamedColumnCount: 0, convertedColumnCount: 0, parsedDateColumnCount: 0,
      removedRowCount: 0, calculatedColumnCount: 0, replacedCellCount: 0,
      droppedColumnCount: 0, splitColumnCount: 0, mergedColumnCount: 0,
      droppedSourceColumnCount: 0, adjustedOutlierCellCount: 0,
      outlierRemovedRowCount: 0, outlierColumnCount: 1,
      groupCount: 0, aggregatedColumnCount: 0, collapsedRowCount: 0,
      normalizedContactCellCount: 0, normalizedContactColumnCount: 0, extractedColumnCount: 0,
    });
    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("tab", { name: "Transformaciones" }));
    fireEvent.click(screen.getByRole("button", { name: "+ Añadir regla" }));
    fireEvent.change(screen.getByLabelText("Columna de valores atípicos 1"), { target: { value: "valor" } });
    fireEvent.click(screen.getByRole("button", { name: "Aplicar receta" }));
    const dialog = screen.getByRole("alertdialog", { name: "Confirmar cambios de alto impacto" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Confirmar y aplicar" }));

    expect(await screen.findByText("La receta no produjo cambios en el dataset.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Deshacer" })).toBeDisabled();
  });

  it("confirma y aplica un resumen agrupado respetando tipos efectivos", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.20.0", platform: "windows" });
    const original: DatasetPreview = {
      fileName: "ventas.csv", fileSizeBytes: 120, rowCount: 6, columnCount: 3,
      columns: [{ name: "region", dataType: "String" }, { name: "importe", dataType: "String" }, { name: "nota", dataType: "String" }], rows: [["Norte", "10", "A"]],
    };
    mockDatasetLoad(original);
    const applySpy = vi.spyOn(bridge, "applyTransformRecipe").mockResolvedValue({
      dataset: { ...original, rowCount: 2, columnCount: 3 }, renamedColumnCount: 0,
      changed: true,
      convertedColumnCount: 1, parsedDateColumnCount: 0, removedRowCount: 0,
      calculatedColumnCount: 0, replacedCellCount: 0, droppedColumnCount: 0,
      splitColumnCount: 0, mergedColumnCount: 0, droppedSourceColumnCount: 0,
      adjustedOutlierCellCount: 0, outlierRemovedRowCount: 0, outlierColumnCount: 0,
      groupCount: 2, aggregatedColumnCount: 2, collapsedRowCount: 4,
      normalizedContactCellCount: 0, normalizedContactColumnCount: 0, extractedColumnCount: 0,
    });
    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("tab", { name: "Transformaciones" }));
    fireEvent.change(screen.getByLabelText("Columna para convertir 1"), { target: { value: "importe" } });
    fireEvent.change(screen.getByLabelText("Tipo destino 1"), { target: { value: "decimal" } });
    fireEvent.click(screen.getByRole("checkbox", { name: "Sustituir las filas por un resumen" }));
    const keys = screen.getByRole("group", { name: "Columnas para definir los grupos" });
    fireEvent.click(within(keys).getByRole("checkbox", { name: "region" }));
    fireEvent.click(screen.getByRole("button", { name: "+ Añadir cálculo" }));
    fireEvent.change(screen.getByLabelText("Columna para el cálculo 1"), { target: { value: "importe" } });
    expect(within(screen.getByLabelText("Cálculo 1")).getByRole("option", { name: "Suma" })).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Cálculo 1"), { target: { value: "sum" } });
    fireEvent.click(screen.getByRole("button", { name: "+ Añadir cálculo" }));
    fireEvent.change(screen.getByLabelText("Columna para el cálculo 2"), { target: { value: "nota" } });
    fireEvent.change(screen.getByLabelText("Cálculo 2"), { target: { value: "count_unique" } });
    fireEvent.click(screen.getByRole("button", { name: "Aplicar receta" }));
    const dialog = screen.getByRole("alertdialog", { name: "Confirmar cambios de alto impacto" });
    expect(within(dialog).getByText(/resumen de 1 claves y 2 agregaciones sobre 6 filas/)).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancelar" }));
    expect(applySpy).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Aplicar receta" }));
    fireEvent.click(within(screen.getByRole("alertdialog")).getByRole("button", { name: "Confirmar y aplicar" }));
    expect(applySpy).toHaveBeenCalledWith(expect.objectContaining({ groupSummary: { groupBy: ["region"], aggregations: [{ column: "importe", operation: "sum" }, { column: "nota", operation: "count_unique" }] } }), null);
    expect(await screen.findByText(/resumen de 2 grupos con 2 agregaciones/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Deshacer" })).toBeEnabled();
  });

  it("combina contacto y extracción textual con tipos efectivos", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.21.0", platform: "windows" });
    const original: DatasetPreview = { fileName: "clientes.csv", fileSizeBytes: 80, rowCount: 2, columnCount: 2, columns: [{ name: "correo", dataType: "String" }, { name: "codigo", dataType: "Int64" }], rows: [[" A@B.COM ", "12-34"]] };
    mockDatasetLoad(original);
    const applySpy = vi.spyOn(bridge, "applyTransformRecipe").mockResolvedValue({
      dataset: { ...original, columnCount: 3 }, renamedColumnCount: 0, convertedColumnCount: 1,
      changed: true,
      parsedDateColumnCount: 0, removedRowCount: 0, calculatedColumnCount: 0, replacedCellCount: 0,
      droppedColumnCount: 0, splitColumnCount: 0, mergedColumnCount: 0, droppedSourceColumnCount: 0,
      adjustedOutlierCellCount: 0, outlierRemovedRowCount: 0, outlierColumnCount: 0,
      groupCount: 0, aggregatedColumnCount: 0, collapsedRowCount: 0,
      normalizedContactCellCount: 1, normalizedContactColumnCount: 1, extractedColumnCount: 1,
    });
    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("tab", { name: "Transformaciones" }));
    fireEvent.change(screen.getByLabelText("Columna para convertir 1"), { target: { value: "codigo" } });
    fireEvent.click(screen.getByRole("button", { name: "+ Añadir contacto" }));
    fireEvent.change(screen.getByLabelText("Columna de contacto 1"), { target: { value: "correo" } });
    fireEvent.click(screen.getByRole("button", { name: "+ Añadir extracción" }));
    const source = screen.getByLabelText("Columna de extracción 1");
    expect(within(source).getByRole("option", { name: "codigo" })).toBeInTheDocument();
    fireEvent.change(source, { target: { value: "codigo" } });
    fireEvent.change(screen.getByLabelText("Texto que extraer 1"), { target: { value: "before" } });
    fireEvent.change(screen.getByLabelText("Nombre de extracción 1"), { target: { value: "prefijo" } });
    fireEvent.change(screen.getByLabelText("Separador para extraer texto 1"), { target: { value: "-" } });
    fireEvent.click(screen.getByRole("button", { name: "Aplicar receta" }));
    const dialog = screen.getByRole("alertdialog", { name: "Confirmar cambios de alto impacto" });
    expect(within(dialog).getByText(/normalizarán valores de contacto en 1 columnas/)).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancelar" }));
    expect(applySpy).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Aplicar receta" }));
    fireEvent.click(within(screen.getByRole("alertdialog")).getByRole("button", { name: "Confirmar y aplicar" }));
    expect(applySpy).toHaveBeenCalledWith(expect.objectContaining({ contactNormalizations: [{ column: "correo", kind: "email" }], textExtractions: [{ source: "codigo", kind: "before", name: "prefijo", delimiter: "-" }] }), null);
    expect(await screen.findByText(/1 contactos normalizados en 1 columnas, 1 columnas extraídas/)).toBeInTheDocument();
  });

  it("muestra el estado degradado del historial entregado por Rust", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.22.0", platform: "windows" });
    const dataset: DatasetPreview = { fileName: "sin-snapshots.csv", fileSizeBytes: 20, rowCount: 1, columnCount: 1, columns: [{ name: "valor", dataType: "String" }], rows: [["A"]] };
    mockDatasetLoad(dataset);
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(historyState({
      canUndo: false, canRedo: false, snapshotsEnabled: false,
      degradedReason: "No hay espacio disponible para snapshots.", currentIndex: 0,
      entryCount: 1, entries: [{ id: null, index: 0, label: "Dataset cargado", isCurrent: true }],
    }));
    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Preparar");
    expect(screen.getByText("No hay espacio disponible para snapshots.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Deshacer" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Rehacer" })).toBeDisabled();
    expect(screen.queryByText("Ver versiones (1)")).not.toBeInTheDocument();
  });

  it("mantiene la compuerta de exportación y presenta éxito, cancelación y errores", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.57.0", platform: "windows" });
    const dataset: DatasetPreview = {
      fileName: "entrega.csv", fileSizeBytes: 128, rowCount: 2, columnCount: 1,
      columns: [{ name: "valor", dataType: "String" }], rows: [["A"], ["B"]],
    };
    mockDatasetLoad(dataset);
    const exportSpy = vi.spyOn(bridge, "exportDataset").mockResolvedValue({
      fileName: "entrega.zip",
      fileSizeBytes: 2048,
      format: "Paquete Columnia",
      protectedColumnCount: 0,
      protectedColumns: [],
    });
    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Entregar");
    fireEvent.click(screen.getByRole("checkbox", {
      name: "Confirmo que quiero exportar sin validar la calidad",
    }));
    fireEvent.change(screen.getByRole("combobox", { name: "Formato de exportación" }), {
      target: { value: "bundle" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Exportar Paquete ZIP" }));
    await waitFor(() => expect(screen.getByRole("heading", { name: "Copia lista" })).toBeInTheDocument());
    expect(screen.getByText("entrega.zip")).toBeInTheDocument();
    expect(within(screen.getByRole("button", { name: "Entregar" })).getByText("Hecho")).toBeInTheDocument();
    expect(exportSpy).toHaveBeenCalledWith("bundle", [], true, expect.anything(), "none");

    exportSpy.mockResolvedValueOnce(null);
    fireEvent.click(screen.getByRole("button", { name: "Exportar Paquete ZIP" }));
    await waitFor(() => expect(screen.queryByRole("heading", { name: "Copia lista" })).not.toBeInTheDocument());
    expect(within(screen.getByRole("button", { name: "Entregar" })).queryByText("Hecho")).not.toBeInTheDocument();

    exportSpy.mockRejectedValueOnce(new Error("operación cancelada por el usuario"));
    fireEvent.click(screen.getByRole("button", { name: "Exportar Paquete ZIP" }));
    expect(await screen.findByText(/Exportación cancelada/)).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();

    exportSpy.mockRejectedValueOnce(new Error("disco lleno"));
    fireEvent.click(screen.getByRole("button", { name: "Exportar Paquete ZIP" }));
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("disco lleno"));
  });

  it("expone el estado web y la ficha local de licencia y privacidad", () => {
    renderAppWithHeaderConfirmation();
    expect(screen.getByText("Vista web · motor no conectado")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Licencia y privacidad"));
    const legalPanel = screen.getByRole("region", { name: "Licencia y privacidad de Columnia" });
    expect(within(legalPanel).getByText(/no inicia conexiones de red por sí sola/)).toBeInTheDocument();
    expect(within(legalPanel).getByText(/filas que el usuario elija enviar mediante una exportación ODBC explícita/)).toBeInTheDocument();
    expect(screen.getByText("Licencia", { selector: "h2" })).toBeInTheDocument();
  });

  it("mantiene exclusivos los paneles flotantes de la barra lateral", () => {
    renderAppWithHeaderConfirmation();
    const utilitiesSummary = screen.getByText("Preferencias y recursos");
    const legalSummary = screen.getByText("Licencia y privacidad");
    const utilitiesDetails = utilitiesSummary.closest("details");
    const legalDetails = legalSummary.closest("details");

    fireEvent.click(utilitiesSummary);
    expect(utilitiesDetails).toHaveAttribute("open");
    fireEvent.click(legalSummary);
    expect(utilitiesDetails).not.toHaveAttribute("open");
    expect(legalDetails).toHaveAttribute("open");
    fireEvent.click(utilitiesSummary);
    expect(utilitiesDetails).toHaveAttribute("open");
    expect(legalDetails).not.toHaveAttribute("open");
  });

  it.each([
    { mutation: "join", outcome: "cancelled" },
    { mutation: "join", outcome: "error" },
    { mutation: "consolidate", outcome: "cancelled" },
    { mutation: "consolidate", outcome: "error" },
  ] as const)("preserva la comparación al cancelar o fallar $mutation ($outcome)", async ({ mutation, outcome }) => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.57.0", platform: "windows" });
    const dataset: DatasetPreview = {
      fileName: "actual.csv", fileSizeBytes: 128, rowCount: 2, columnCount: 2,
      columns: [{ name: "id", dataType: "Int64" }, { name: "valor", dataType: "String" }],
      rows: [["1", "A"], ["2", "B"]],
    };
    mockDatasetLoad(dataset);
    const comparison = {
      currentFileName: "actual.csv",
      comparedFileName: "nuevo.csv",
      currentRowCount: 2,
      comparedRowCount: 2,
      commonRowCount: 1,
      currentOnlyRowCount: 1,
      comparedOnlyRowCount: 1,
      sharedColumns: ["id"],
      currentOnlyColumns: ["valor"],
      comparedOnlyColumns: [],
      schemaCompatible: true,
      keyColumns: ["id"],
      matchedKeyCount: 1,
      currentOnlyKeyCount: 1,
      comparedOnlyKeyCount: 0,
      conflictingKeyCount: 0,
      duplicateKeyCount: 0,
      conflicts: [],
      conflictOffset: 0,
      conflictsTruncated: false,
      canConsolidate: true,
    };
    vi.spyOn(bridge, "compareDataset").mockResolvedValue(comparison);
    vi.spyOn(bridge, "clearDatasetComparison").mockResolvedValue(undefined);
    let rejectMutation!: (reason: unknown) => void;
    const mutationPromise = new Promise<DatasetPreview>((_resolve, reject) => {
      rejectMutation = reject;
    });
    const consolidateSpy = vi.spyOn(bridge, "useConsolidatedDataset").mockReturnValue(
      mutation === "consolidate" ? mutationPromise : Promise.resolve(dataset),
    );
    const joinSpy = vi.spyOn(bridge, "joinDataset").mockReturnValue(
      mutation === "join" ? mutationPromise : Promise.resolve(dataset),
    );
    const cancelSpy = vi.spyOn(bridge, "cancelOperation").mockResolvedValue(undefined);

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    fireEvent.click(await screen.findByText("Comparar con otro dataset"));
    fireEvent.click(screen.getByRole("checkbox", { name: /id/ }));
    fireEvent.click(screen.getByRole("button", { name: "Elegir dataset para comparar" }));
    await screen.findByText("nuevo.csv");

    const startButton = mutation === "join"
      ? screen.getByRole("button", { name: "Elegir fuente y unir" })
      : screen.getByRole("button", { name: "Consolidar filas" });
    fireEvent.click(startButton);
    await waitFor(() => expect(mutation === "join" ? joinSpy : consolidateSpy).toHaveBeenCalledOnce());

    const cancelLabel = mutation === "join" ? "Cancelar unión" : "Cancelar consolidación";
    const waitingLabel = mutation === "join" ? "Esperando unión…" : "Esperando consolidación…";
    expect(screen.getByRole("button", { name: cancelLabel })).toBeEnabled();
    expect(screen.getByRole("button", { name: waitingLabel })).toBeDisabled();
    expect(screen.getByText("Filas compartidas")).toBeInTheDocument();

    if (outcome === "cancelled") {
      fireEvent.click(screen.getByRole("button", { name: cancelLabel }));
      expect(cancelSpy).toHaveBeenCalledWith("reviewMutation");
      expect(await screen.findByRole("button", {
        name: mutation === "join" ? "Cancelando unión…" : "Cancelando consolidación…",
      })).toBeDisabled();
    }

    await act(async () => {
      rejectMutation(outcome === "cancelled"
        ? "Operación cancelada por el usuario."
        : new Error("fallo de publicación"));
      await mutationPromise.catch(() => undefined);
    });

    expect(screen.getByText("nuevo.csv")).toBeInTheDocument();
    expect(screen.getByText("Filas compartidas")).toBeInTheDocument();
    if (outcome === "cancelled") {
      expect(await screen.findByRole("button", {
        name: mutation === "join" ? "Elegir fuente y unir" : "Consolidar filas",
      })).toBeEnabled();
    } else {
      expect(await screen.findByText(mutation === "join"
        ? "No se pudieron unir los datasets: fallo de publicación"
        : "No se pudieron consolidar las filas: fallo de publicación")).toBeInTheDocument();
    }
    if (outcome === "error") expect(cancelSpy).not.toHaveBeenCalled();
  });

  it("cancela la resolución de conflictos sin publicar cambios y permite reintentar", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.57.0", platform: "windows" });
    const dataset: DatasetPreview = {
      fileName: "actual.csv", fileSizeBytes: 128, rowCount: 2, columnCount: 2,
      columns: [{ name: "id", dataType: "Int64" }, { name: "valor", dataType: "String" }],
      rows: [["1", "A"], ["2", "B"]],
    };
    mockDatasetLoad(dataset);
    const comparison = {
      currentFileName: "actual.csv",
      comparedFileName: "nuevo.csv",
      currentRowCount: 2,
      comparedRowCount: 2,
      commonRowCount: 1,
      currentOnlyRowCount: 1,
      comparedOnlyRowCount: 0,
      sharedColumns: ["id", "valor"],
      currentOnlyColumns: [],
      comparedOnlyColumns: [],
      schemaCompatible: true,
      keyColumns: ["id"],
      matchedKeyCount: 1,
      currentOnlyKeyCount: 0,
      comparedOnlyKeyCount: 0,
      conflictingKeyCount: 1,
      duplicateKeyCount: 0,
      conflicts: [{
        key: ["1"],
        cells: [{ column: "valor", current: "A", compared: "Z" }],
      }],
      conflictOffset: 0,
      conflictsTruncated: false,
      canConsolidate: false,
    };
    vi.spyOn(bridge, "compareDataset").mockResolvedValue(comparison);
    vi.spyOn(bridge, "clearDatasetComparison").mockResolvedValue(undefined);
    let rejectResolution!: (reason: unknown) => void;
    const resolutionPromise = new Promise<DatasetPreview>((_resolve, reject) => {
      rejectResolution = reject;
    });
    const resolvedDataset = { ...dataset, fileName: "resuelto.csv" };
    const resolveSpy = vi.spyOn(bridge, "resolveDatasetConflicts")
      .mockReturnValueOnce(resolutionPromise)
      .mockResolvedValueOnce(resolvedDataset);
    const cancelSpy = vi.spyOn(bridge, "cancelOperation").mockResolvedValue(undefined);

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    fireEvent.click(await screen.findByText("Comparar con otro dataset"));
    fireEvent.click(screen.getByRole("checkbox", { name: /id/ }));
    fireEvent.click(screen.getByRole("button", { name: "Elegir dataset para comparar" }));
    await screen.findByText("nuevo.csv");
    fireEvent.click(screen.getByRole("radio", { name: "Usar comparado en valor" }));
    fireEvent.click(screen.getByRole("button", { name: "Resolver conflictos" }));
    await waitFor(() => expect(resolveSpy).toHaveBeenCalledWith([
      { action: "useSource", conflictIndex: 0, column: "valor", source: "compared" },
    ]));
    expect(screen.getByRole("button", { name: "Cancelar resolución" })).toBeEnabled();
    expect(screen.getByText("Resolviendo conflictos. Puedes cancelar mientras se calcula el resultado.", {
      selector: 'p[role="status"]',
    })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Cancelar resolución" }));
    expect(cancelSpy).toHaveBeenCalledWith("reviewMutation");
    expect(await screen.findByRole("button", { name: "Cancelando resolución…" })).toBeDisabled();
    await act(async () => {
      rejectResolution(new Error("Operación cancelada por el usuario."));
      await resolutionPromise.catch(() => undefined);
    });

    expect(await screen.findByRole("heading", { name: "actual.csv" })).toBeInTheDocument();
    expect(screen.getByText("nuevo.csv")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Usar comparado en valor" })).toBeChecked();
    expect(screen.getByRole("button", { name: "Resolver conflictos" })).toBeEnabled();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Resolver conflictos" }));
    await waitFor(() => expect(resolveSpy).toHaveBeenCalledTimes(2));
    expect(resolveSpy).toHaveBeenLastCalledWith([
      { action: "useSource", conflictIndex: 0, column: "valor", source: "compared" },
    ]);
    expect(await screen.findByRole("heading", { name: "resuelto.csv" })).toBeInTheDocument();
  });

  it("mantiene exclusión mutua hasta que termina una cancelación tardía", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.57.0", platform: "windows" });
    const dataset: DatasetPreview = {
      fileName: "actual.csv", fileSizeBytes: 128, rowCount: 2, columnCount: 2,
      columns: [{ name: "id", dataType: "Int64" }, { name: "valor", dataType: "String" }],
      rows: [["1", "A"], ["2", "B"]],
    };
    const joinedDataset: DatasetPreview = { ...dataset, fileName: "unido.csv" };
    const secondJoinedDataset: DatasetPreview = { ...dataset, fileName: "unido-segundo.csv" };
    mockDatasetLoad(dataset);
    const comparison = {
      currentFileName: "actual.csv",
      comparedFileName: "nuevo.csv",
      currentRowCount: 2,
      comparedRowCount: 2,
      commonRowCount: 1,
      currentOnlyRowCount: 1,
      comparedOnlyRowCount: 0,
      sharedColumns: ["id"],
      currentOnlyColumns: ["valor"],
      comparedOnlyColumns: [],
      schemaCompatible: true,
      keyColumns: ["id"],
      matchedKeyCount: 1,
      currentOnlyKeyCount: 1,
      comparedOnlyKeyCount: 0,
      conflictingKeyCount: 0,
      duplicateKeyCount: 0,
      conflicts: [],
      conflictOffset: 0,
      conflictsTruncated: false,
      canConsolidate: true,
    };
    const compareSpy = vi.spyOn(bridge, "compareDataset").mockResolvedValue(comparison);
    vi.spyOn(bridge, "clearDatasetComparison").mockResolvedValue(undefined);
    let resolveJoin!: (value: DatasetPreview) => void;
    const joinPromise = new Promise<DatasetPreview>((resolve) => {
      resolveJoin = resolve;
    });
    const joinSpy = vi.spyOn(bridge, "joinDataset")
      .mockReturnValueOnce(joinPromise)
      .mockResolvedValue(secondJoinedDataset);
    let resolveCancellation!: () => void;
    const cancellationPromise = new Promise<void>((resolve) => {
      resolveCancellation = resolve;
    });
    const cancelSpy = vi.spyOn(bridge, "cancelOperation").mockReturnValue(cancellationPromise);

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    fireEvent.click(await screen.findByText("Comparar con otro dataset"));
    fireEvent.click(screen.getByRole("checkbox", { name: /id/ }));
    fireEvent.click(screen.getByRole("button", { name: "Elegir dataset para comparar" }));
    await waitFor(() => expect(compareSpy).toHaveBeenCalledOnce());

    fireEvent.click(screen.getByRole("button", { name: "Elegir fuente y unir" }));
    await waitFor(() => expect(joinSpy).toHaveBeenCalledOnce());
    fireEvent.click(screen.getByRole("button", { name: "Cancelar unión" }));
    expect(cancelSpy).toHaveBeenCalledWith("reviewMutation");

    await act(async () => {
      resolveJoin(joinedDataset);
      await joinPromise;
    });
    expect(await screen.findByRole("heading", { name: "unido.csv" })).toBeInTheDocument();

    const compareButton = screen.getByRole("button", { name: "Elegir dataset para comparar" });
    expect(compareButton).toBeDisabled();
    fireEvent.click(compareButton);
    expect(compareSpy).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveCancellation();
      await cancellationPromise;
    });
    const compareButtonAfterCancellation = screen.getByRole("button", { name: "Elegir dataset para comparar" });
    expect(compareButtonAfterCancellation).toBeEnabled();
    fireEvent.click(compareButtonAfterCancellation);
    await waitFor(() => expect(compareSpy).toHaveBeenCalledTimes(2));
    expect(await screen.findByText("nuevo.csv")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("checkbox", { name: /id/ }));
    fireEvent.click(screen.getByRole("button", { name: "Elegir fuente y unir" }));
    await waitFor(() => expect(joinSpy).toHaveBeenCalledTimes(2));
    expect(await screen.findByRole("heading", { name: "unido-segundo.csv" })).toBeInTheDocument();
  });

  it("coordina comparación, descarte, consolidación y unión desde Revisar", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.57.0", platform: "windows" });
    const dataset: DatasetPreview = {
      fileName: "actual.csv", fileSizeBytes: 128, rowCount: 2, columnCount: 2,
      columns: [{ name: "id", dataType: "Int64" }, { name: "valor", dataType: "String" }],
      rows: [["1", "A"], ["2", "B"]],
    };
    mockDatasetLoad(dataset);
    const comparison = {
      currentFileName: "actual.csv",
      comparedFileName: "nuevo.csv",
      currentRowCount: 2,
      comparedRowCount: 2,
      commonRowCount: 1,
      currentOnlyRowCount: 1,
      comparedOnlyRowCount: 1,
      sharedColumns: ["id"],
      currentOnlyColumns: ["valor"],
      comparedOnlyColumns: [],
      schemaCompatible: true,
      keyColumns: ["id"],
      matchedKeyCount: 1,
      currentOnlyKeyCount: 1,
      comparedOnlyKeyCount: 0,
      conflictingKeyCount: 0,
      duplicateKeyCount: 0,
      conflicts: [],
      conflictOffset: 0,
      conflictsTruncated: false,
      canConsolidate: true,
    };
    const compareSpy = vi.spyOn(bridge, "compareDataset").mockResolvedValue(comparison);
    vi.spyOn(bridge, "clearDatasetComparison").mockResolvedValue(undefined);
    vi.spyOn(bridge, "useConsolidatedDataset").mockResolvedValue(dataset);
    const joinSpy = vi.spyOn(bridge, "joinDataset").mockResolvedValue(dataset);
    vi.spyOn(bridge, "exportDataset").mockResolvedValue({
      fileName: "actual.zip",
      fileSizeBytes: 2048,
      format: "Paquete Columnia",
      protectedColumnCount: 0,
      protectedColumns: [],
    });
    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await waitFor(() => expect(screen.getByText("Comparar con otro dataset")).toBeInTheDocument());
    await switchPhase("Entregar");
    fireEvent.click(screen.getByRole("checkbox", { name: "Confirmo que quiero exportar sin validar la calidad" }));
    fireEvent.change(screen.getByRole("combobox", { name: "Formato de exportación" }), { target: { value: "bundle" } });
    fireEvent.click(screen.getByRole("button", { name: "Exportar Paquete ZIP" }));
    await screen.findByRole("heading", { name: "Copia lista" });
    expect(within(screen.getByRole("button", { name: "Entregar" })).getByText("Hecho")).toBeInTheDocument();
    await switchPhase("Revisar");
    await waitFor(() => expect(screen.getByText("Comparar con otro dataset")).toBeInTheDocument());
    fireEvent.click(screen.getByText("Comparar con otro dataset"));
    fireEvent.click(screen.getByRole("checkbox", { name: /id/ }));
    fireEvent.click(screen.getByRole("button", { name: "Elegir dataset para comparar" }));
    await waitFor(() => expect(compareSpy).toHaveBeenCalledWith(["id"]));
    expect(screen.getByText("nuevo.csv")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Descartar comparación" }));
    await waitFor(() => expect(bridge.clearDatasetComparison).toHaveBeenCalled());

    fireEvent.click(screen.getByRole("button", { name: "Elegir dataset para comparar" }));
    await waitFor(() => expect(screen.getByText("nuevo.csv")).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: "Consolidar filas" }));
    await waitFor(() => expect(bridge.useConsolidatedDataset).toHaveBeenCalledOnce());
    expect(within(screen.getByRole("button", { name: "Entregar" })).queryByText("Hecho")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("checkbox", { name: /id/ }));
    fireEvent.click(screen.getByRole("radio", { name: /^Left/ }));
    fireEvent.click(screen.getByRole("button", { name: "Elegir fuente y unir" }));
    await waitFor(() => expect(joinSpy).toHaveBeenCalledWith(["id"], "left"));
  });

  it("protege el historial reciente, precarga etapas y permite reintentar el catálogo", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.57.0", platform: "windows" });
    const listProjects = vi.spyOn(bridge, "listProjects")
      .mockRejectedValueOnce(new Error("catálogo temporalmente no disponible"))
      .mockResolvedValue({ projects: [], recoveryCandidate: null });
    const pick = vi.spyOn(bridge, "pickDatasetSource").mockResolvedValue(null);
    localStorage.setItem("columnia.recent-datasets", JSON.stringify([
      { id: "recent-1", fileName: "C:\\datos\\ventas.csv", format: "csv", lastOpenedAt: 2 },
      { id: "recent-2", fileName: "clientes.json", format: "json", lastOpenedAt: 1 },
    ]));

    renderAppWithHeaderConfirmation();
    await waitFor(() => expect(
      screen.getAllByRole("alert").some((alert) => alert.textContent?.includes("catálogo temporalmente")),
    ).toBe(true));
    fireEvent.click(screen.getByRole("button", { name: "Reintentar" }));
    await waitFor(() => expect(listProjects).toHaveBeenCalledTimes(2));

    fireEvent.mouseEnter(screen.getByRole("button", { name: "Revisar" }));
    fireEvent.focus(screen.getByRole("button", { name: "Preparar" }));
    fireEvent.mouseEnter(screen.getByRole("button", { name: "Entregar" }));
    fireEvent.click(screen.getByText("Preferencias y recursos"));
    fireEvent.click(screen.getAllByRole("button", { name: "Elegir de nuevo" })[0]);
    await waitFor(() => expect(pick).toHaveBeenCalledOnce());
    fireEvent.click(screen.getByRole("button", { name: "Quitar ventas.csv del historial" }));
    fireEvent.click(screen.getByRole("button", { name: "Limpiar historial" }));
    expect(screen.queryByRole("heading", { name: "Archivos recientes" })).not.toBeInTheDocument();
    localStorage.removeItem("columnia.recent-datasets");
  });

  it("muestra un error del motor y limpia la conexión sin filtrar el mensaje", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockRejectedValue(new Error("motor no disponible"));
    vi.spyOn(bridge, "listProjects").mockResolvedValue({ projects: [], recoveryCandidate: null });
    vi.spyOn(bridge, "listSampleDatasets").mockResolvedValue([]);

    render(<App />);
    expect(await screen.findByRole("alert")).toHaveTextContent("Error del motor: motor no disponible");
  });

  it("normaliza un rechazo no tipado al inicializar el motor", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockRejectedValue("motor no disponible");
    vi.spyOn(bridge, "listProjects").mockResolvedValue({ projects: [], recoveryCandidate: null });
    vi.spyOn(bridge, "listSampleDatasets").mockResolvedValue([]);

    render(<App />);
    expect(await screen.findByRole("alert")).toHaveTextContent("Error del motor: motor no disponible");
  });

  it("tolera un selector cancelado y permite reintentar una muestra delimitada", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "1.25.0", platform: "windows" });
    vi.spyOn(bridge, "listProjects").mockResolvedValue({ projects: [], recoveryCandidate: null });
    vi.spyOn(bridge, "listSampleDatasets").mockResolvedValue([]);
    const pick = vi.spyOn(bridge, "pickDatasetSource").mockResolvedValue(null);
    const preview = vi.spyOn(bridge, "previewDelimitedHeaderReview")
      .mockRejectedValueOnce(new Error("muestra ilegible"))
      .mockResolvedValueOnce(defaultDelimitedHeaderReview());
    vi.spyOn(bridge, "loadDatasetSelection").mockResolvedValue({
      fileName: "muestra.csv", fileSizeBytes: 32, rowCount: 1, columnCount: 1,
      columns: [{ name: "value", dataType: "String" }], rows: [["ok"]],
    });

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await waitFor(() => expect(pick).toHaveBeenCalledOnce());
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    pick.mockResolvedValueOnce({
      selectionId: "retry-selection", fileName: "muestra.csv", fileSizeBytes: 32,
      format: "csv", sheets: [], defaultSheetId: null, isCompressedContainer: false,
      resourceEstimate: resourceEstimate(32),
    });
    fireEvent.click(screen.getByRole("button", { name: "Seleccionar dataset" }));
    const dialog = await screen.findByRole("dialog", { name: "Revisar encabezados de muestra.csv" });
    expect(await within(dialog).findByRole("alert")).toHaveTextContent("muestra ilegible");
    fireEvent.click(within(dialog).getByRole("button", { name: "Reintentar muestra" }));
    await waitFor(() => expect(preview).toHaveBeenCalledTimes(2));
    expect(await within(dialog).findByText("Con primera fila como encabezado")).toBeInTheDocument();
  });

  it("permite cancelar la inspección de un libro antes de cargarlo", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "1.25.0", platform: "windows" });
    vi.spyOn(bridge, "listProjects").mockResolvedValue({ projects: [], recoveryCandidate: null });
    vi.spyOn(bridge, "listSampleDatasets").mockResolvedValue([]);
    vi.spyOn(bridge, "pickDatasetSource").mockResolvedValue({
      selectionId: "workbook-cancel", fileName: "ventas.xlsx", fileSizeBytes: 1024,
      format: "excel", sheets: [{ id: "0", name: "Ventas" }], defaultSheetId: "0",
      isCompressedContainer: false, resourceEstimate: resourceEstimate(1024),
    });
    let resolveSheets!: (value: { id: string; name: string }[]) => void;
    vi.spyOn(bridge, "inspectWorkbookSheets").mockReturnValue(new Promise((resolve) => { resolveSheets = resolve; }));
    const cancel = vi.spyOn(bridge, "cancelOperation").mockResolvedValue(undefined);
    const discard = vi.spyOn(bridge, "discardDatasetSelection").mockResolvedValue(undefined);

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    const cancelButton = await screen.findByRole("button", { name: "Cancelar inspección" });
    fireEvent.click(cancelButton);
    await waitFor(() => expect(cancel).toHaveBeenCalledWith("load"));
    await waitFor(() => expect(discard).toHaveBeenCalledWith("workbook-cancel"));
    expect(screen.queryByRole("button", { name: "Cancelar inspección" })).not.toBeInTheDocument();
    resolveSheets([{ id: "0", name: "Ventas" }]);
  });

  it("confirma el preflight de una fuente source-backed antes de materializarla", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "1.25.0", platform: "windows" });
    vi.spyOn(bridge, "listProjects").mockResolvedValue({ projects: [], recoveryCandidate: null });
    vi.spyOn(bridge, "listSampleDatasets").mockResolvedValue([]);
    const source = {
      selectionId: "json-source", fileName: "ventas.json", fileSizeBytes: 512 * 1024 * 1024,
      format: "json" as const, sheets: [], defaultSheetId: null, isCompressedContainer: false,
      resourceEstimate: resourceEstimate(512 * 1024 * 1024, "sourceBacked"),
    };
    vi.spyOn(bridge, "pickDatasetSource").mockResolvedValue(source);
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(historyState());
    vi.spyOn(bridge, "getDatasetProfile").mockResolvedValue({
      rowCount: 1, duplicateRowCount: 0, nearDuplicateRowCount: 0, duplicatePercentage: 0, columns: [],
    });
    const load = vi.spyOn(bridge, "loadDatasetSelection").mockResolvedValue({
      fileName: "ventas.json", fileSizeBytes: source.fileSizeBytes, rowCount: 1, columnCount: 1,
      columns: [{ name: "id", dataType: "Int64" }], rows: [["1"]],
    });

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    const dialog = await screen.findByRole("dialog", { name: "Revisar importación de ventas.json" });
    expect(within(dialog).getByText(/Lectura source-backed/)).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "Revisar esquema" }));
    fireEvent.click(await within(dialog).findByRole("button", { name: "Cargar archivo" }));
    expect(await screen.findByRole("heading", { name: "ventas.json" })).toBeInTheDocument();
    expect(load).toHaveBeenCalledWith("json-source", null, null, expect.any(Function), null, null, null);
  });

  it("cancela una página de vista previa y muestra el fallo al reintentarlo", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "1.25.0", platform: "windows" });
    const dataset: DatasetPreview = {
      fileName: "paginas.csv", fileSizeBytes: 128, rowCount: 75, columnCount: 1,
      columns: [{ name: "city", dataType: "String" }], rows: [["Santo Domingo"]],
    };
    mockDatasetLoad(dataset);
    let resolvePage!: (value: { offset: number; rows: (string | null)[][] }) => void;
    const page = vi.spyOn(bridge, "getDatasetPage").mockReturnValue(new Promise((resolve) => { resolvePage = resolve; }));
    const cancel = vi.spyOn(bridge, "cancelOperation").mockResolvedValue(undefined);
    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    fireEvent.click(await screen.findByRole("tab", { name: "Vista previa" }));
    fireEvent.click(screen.getByRole("button", { name: "Siguiente" }));
    fireEvent.click(await screen.findByRole("button", { name: "Cancelar carga" }));
    expect(cancel).toHaveBeenCalledWith("datasetPage");
    resolvePage({ offset: 50, rows: [["Puerto Plata"]] });
    await waitFor(() => expect(screen.getByRole("button", { name: "Siguiente" })).toBeEnabled());

    page.mockRejectedValueOnce(new Error("página no disponible"));
    fireEvent.click(screen.getByRole("button", { name: "Siguiente" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("página no disponible");
  });

  it("exporta a una base remota después del preflight y conserva el contrato del destino", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "1.25.0", platform: "windows" });
    const dataset: DatasetPreview = {
      fileName: "remoto.csv", fileSizeBytes: 128, rowCount: 1, columnCount: 1,
      columns: [{ name: "id", dataType: "Int64" }], rows: [["1"]],
    };
    mockDatasetLoad(dataset);
    vi.spyOn(bridge, "preflightDatabaseExport").mockResolvedValue({
      kind: "postgresql", schema: "public", table: "dataset", tablePolicy: "create_only",
      tableExists: false, ready: true, issues: [],
    } satisfies RemoteExportPreflight);
    const exportSpy = vi.spyOn(bridge, "exportDatasetToDatabase").mockResolvedValue({
      fileName: "remoto", fileSizeBytes: 1, format: "PostgreSQL", protectedColumnCount: 0, protectedColumns: [],
    });
    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Entregar");
    fireEvent.click(screen.getByRole("checkbox", { name: "Confirmo que quiero exportar sin validar la calidad" }));
    fireEvent.change(screen.getByRole("combobox", { name: "Formato de exportación" }), { target: { value: "postgresql" } });
    fireEvent.change(screen.getByLabelText("Cadena de conexión ODBC"), { target: { value: "Driver={PostgreSQL};Server=localhost" } });
    fireEvent.click(screen.getByRole("button", { name: "Analizar compatibilidad" }));
    await screen.findByText(/Preflight completo/);
    fireEvent.click(screen.getByRole("button", { name: "Exportar PostgreSQL" }));
    await waitFor(() => expect(exportSpy).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "postgresql", schema: "public", table: "dataset" }),
      [], true, expect.any(Function), "none",
    ));
  });

  it("permite revisar un perfil reutilizable en el diálogo unificado y aplicar la tarea", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "1.25.0", platform: "windows" });
    const { task, summary } = reusableTaskFixture();
    const { headerMode: _headerMode, ...jsonProfileBase } = task.importProfile;
    const jsonTask: ReusableTask = {
      ...task,
      importProfile: { ...jsonProfileBase, format: "json" },
    };
    const source = {
      selectionId: "profile-json", fileName: "perfil.json", fileSizeBytes: 64,
      format: "json" as const, sheets: [], defaultSheetId: null, isCompressedContainer: false,
      resourceEstimate: resourceEstimate(64),
    };
    const dataset: DatasetPreview = {
      fileName: "perfil.json", fileSizeBytes: 64, rowCount: 1, columnCount: 1,
      columns: [{ name: "id", dataType: "Int64" }], rows: [["1"]],
    };
    vi.spyOn(bridge, "listProjects").mockResolvedValue({ projects: [], recoveryCandidate: null });
    vi.spyOn(bridge, "listReusableTasks").mockResolvedValue([summary]);
    vi.spyOn(bridge, "openReusableTask").mockResolvedValue(jsonTask);
    vi.spyOn(bridge, "pickDatasetSource").mockResolvedValue(source);
    vi.spyOn(bridge, "loadDatasetSelection").mockResolvedValue(dataset);
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(historyState());
    vi.spyOn(bridge, "getDatasetProfile").mockResolvedValue({
      rowCount: 1, duplicateRowCount: 0, nearDuplicateRowCount: 0, duplicatePercentage: 0, columns: [],
    });
    const discard = vi.spyOn(bridge, "discardDatasetSelection").mockResolvedValue(undefined);

    render(<App />);
    fireEvent.click(await screen.findByText("Reutilizar una tarea"));
    fireEvent.change(await screen.findByLabelText("Tarea guardada"), { target: { value: summary.id } });
    await screen.findByText("Configuración que se reutilizará");
    fireEvent.click(screen.getByRole("button", { name: "Preparar próxima importación" }));
    await screen.findByText(/Tarea “Cierre recurrente” preparada/);
    fireEvent.click(screen.getByRole("button", { name: "Seleccionar dataset" }));
    const profileDialog = await screen.findByRole("dialog", { name: "Revisar importación de perfil.json" });
    expect(within(profileDialog).getByRole("heading", { name: "Perfil reutilizable del proyecto" }).parentElement)
      .toHaveTextContent("Se usará el perfil de “Cierre recurrente”");
    fireEvent.click(within(profileDialog).getByRole("button", { name: "Revisar esquema" }));
    fireEvent.click(await within(profileDialog).findByRole("button", { name: "Cargar archivo" }));
    expect(await screen.findByRole("heading", { name: "Prepara datos consistentes" })).toBeInTheDocument();
    expect(bridge.loadDatasetSelection).toHaveBeenLastCalledWith(
      "profile-json", null, null, expect.any(Function), {
        ...jsonTask.importProfile,
        dateConvention: "unresolved",
        numberConvention: "unresolved",
      }, null, null,
    );

    fireEvent.click(screen.getByRole("button", { name: "Cargar" }));
    fireEvent.click(screen.getByRole("button", { name: "Seleccionar otro dataset" }));
    const secondProfileDialog = await screen.findByRole("dialog", { name: "Revisar importación de perfil.json" });
    fireEvent.click(within(secondProfileDialog).getByRole("button", { name: "Revisar esquema" }));
    fireEvent.click(await within(secondProfileDialog).findByRole("button", { name: "Cargar archivo" }));
    await waitFor(() => expect(bridge.loadDatasetSelection).toHaveBeenCalledTimes(2));
    await screen.findByRole("heading", { name: "Revisa antes de modificar" });
    fireEvent.click(screen.getByRole("button", { name: "Cargar" }));
    fireEvent.click(screen.getByRole("button", { name: "Seleccionar otro dataset" }));
    const cancelledProfileDialog = await screen.findByRole("dialog", { name: "Revisar importación de perfil.json" });
    fireEvent.click(within(cancelledProfileDialog).getByRole("button", { name: "Cancelar" }));
    await waitFor(() => expect(discard).toHaveBeenCalledWith("profile-json"));
  });

  it("cambia páginas de conflictos y presenta el error de la página remota", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "1.25.0", platform: "windows" });
    const dataset: DatasetPreview = {
      fileName: "conflictos.csv", fileSizeBytes: 128, rowCount: 2, columnCount: 2,
      columns: [{ name: "id", dataType: "Int64" }, { name: "valor", dataType: "String" }],
      rows: [["1", "A"], ["2", "B"]],
    };
    mockDatasetLoad(dataset);
    const comparison = {
      currentFileName: "conflictos.csv", comparedFileName: "nuevo.csv", currentRowCount: 2,
      comparedRowCount: 2, commonRowCount: 2, currentOnlyRowCount: 0, comparedOnlyRowCount: 0,
      sharedColumns: ["id", "valor"], currentOnlyColumns: [], comparedOnlyColumns: [],
      schemaCompatible: true, keyColumns: ["id"], matchedKeyCount: 2, currentOnlyKeyCount: 0,
      comparedOnlyKeyCount: 0, conflictingKeyCount: 3, duplicateKeyCount: 0,
      conflicts: [{ key: ["1"], cells: [{ column: "valor", current: "A", compared: "Z" }] }],
      conflictOffset: 0, conflictsTruncated: true, canConsolidate: false,
    };
    vi.spyOn(bridge, "compareDataset").mockResolvedValue(comparison);
    vi.spyOn(bridge, "clearDatasetComparison").mockResolvedValue(undefined);
    const nextPage = vi.spyOn(bridge, "getDatasetConflictPage")
      .mockResolvedValueOnce({ offset: 1, conflicts: [{ key: ["2"], cells: [{ column: "valor", current: "B", compared: "Y" }] }], hasNext: true })
      .mockRejectedValueOnce(new Error("página de conflictos no disponible"));

    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    fireEvent.click(await screen.findByText("Comparar con otro dataset"));
    fireEvent.click(screen.getByRole("checkbox", { name: /id/ }));
    fireEvent.click(screen.getByRole("button", { name: "Elegir dataset para comparar" }));
    await screen.findByText("nuevo.csv");
    fireEvent.click(screen.getByRole("radio", { name: "Usar comparado en valor" }));
    fireEvent.click(screen.getByRole("button", { name: "Siguientes conflictos" }));
    await waitFor(() => expect(nextPage).toHaveBeenCalledWith(1, 50));
    expect(await screen.findByText("Conflictos 2–2 de 3")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("radio", { name: "Usar comparado en valor" }));
    fireEvent.click(screen.getByRole("button", { name: "Siguientes conflictos" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("página de conflictos no disponible");
  });

  it("muestra el error al rechazar la cancelación del diagnóstico activo", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "1.25.0", platform: "windows" });
    const dataset: DatasetPreview = {
      fileName: "diagnostico.csv", fileSizeBytes: 64, rowCount: 1, columnCount: 1,
      columns: [{ name: "id", dataType: "Int64" }], rows: [["1"]],
    };
    mockDatasetLoad(dataset);
    let resolveProfile!: (value: DatasetProfile) => void;
    vi.spyOn(bridge, "getDatasetProfile").mockReturnValue(new Promise((resolve) => { resolveProfile = resolve; }));
    const analyzeProfile = bridge.getDatasetProfile;
    const cancel = vi.spyOn(bridge, "cancelOperation").mockRejectedValue(new Error("cancelación rechazada"));
    renderAppWithHeaderConfirmation();
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await waitFor(() => expect(analyzeProfile).toHaveBeenCalled());
    const cancelButton = await screen.findByRole("button", { name: "Cancelar" });
    fireEvent.click(cancelButton);
    await waitFor(() => expect(cancel).toHaveBeenCalledWith("profile"));
    expect(await screen.findByRole("alert")).toHaveTextContent("cancelación rechazada");
    resolveProfile({ rowCount: 1, duplicateRowCount: 0, nearDuplicateRowCount: 0, duplicatePercentage: 0, columns: [] });
  });
});
