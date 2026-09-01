import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { App } from "./App";
import * as bridge from "./bridge";
import type { DatasetPreview, DatasetProfile, HistoryState, ProjectSummary, SavedRecipe } from "./bridge";

function historyState(overrides: Partial<HistoryState> = {}): HistoryState {
  return {
    canUndo: true, canRedo: false, currentIndex: 1, entryCount: 2,
    entries: [{ index: 0, label: "Dataset cargado", isCurrent: false }, { index: 1, label: "Cambio", isCurrent: true }],
    snapshotsEnabled: true, degradedReason: null, maxEntries: 12, diskBytes: 100,
    diskBudgetBytes: 1024, ...overrides,
  };
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});
async function openQualityAndAnalyze() {
  fireEvent.click(await screen.findByRole("button", { name: "Analizar calidad" }));
}

async function switchPhase(label: "Cargar" | "Revisar" | "Preparar" | "Entregar") {
  fireEvent.click(await screen.findByRole("button", { name: label }));
  await waitFor(() => expect(screen.queryByText("Cargando etapa…")).not.toBeInTheDocument());
}

function mockDatasetLoad(dataset: DatasetPreview) {
  vi.spyOn(bridge, "getHistoryState").mockResolvedValue(historyState());
  vi.spyOn(bridge, "pickDatasetSource").mockResolvedValue({
    selectionId: "selection-test",
    fileName: dataset.fileName,
    fileSizeBytes: dataset.fileSizeBytes,
    format: "csv",
    sheets: [],
    defaultSheetId: null,
    isCompressedContainer: false,
  });
  return vi.spyOn(bridge, "loadDatasetSelection").mockResolvedValue(dataset);
}

describe("App", () => {
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
    vi.spyOn(bridge, "listProjects").mockResolvedValue([project]);
    vi.spyOn(bridge, "getRecoveryCandidate").mockResolvedValue(null);
    vi.spyOn(bridge, "openProject").mockResolvedValue({
      project,
      dataset,
      workspace: {
        qualityRules: [{ column: "total", kind: "not_null", maxInvalid: 0 }],
        recipeDraft: draft,
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
    });
    vi.spyOn(bridge, "loadDatasetSelection").mockResolvedValue({
      fileName: "externo.csv", fileSizeBytes: 64, rowCount: 1, columnCount: 1,
      columns: [{ name: "otro", dataType: "String" }], rows: [["dato"]],
    });
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(historyState({
      canUndo: false, entryCount: 1, currentIndex: 0,
      entries: [{ index: 0, label: "Dataset cargado", isCurrent: true }],
    }));
    const profileSpy = vi.spyOn(bridge, "getDatasetProfile");

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Abrir" }));
    expect(await screen.findByRole("heading", { name: "ventas.csv" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Analizar de nuevo" })).toBeInTheDocument();
    expect(screen.getByText("Filas analizadas").parentElement).toHaveTextContent("Filas analizadas1");

    await switchPhase("Entregar");
    expect(screen.getByRole("radio", { name: /^Validar calidad/ })).toBeChecked();
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
    expect(screen.getByRole("button", { name: "Analizar calidad" })).toBeInTheDocument();
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
    const listSpy = vi.spyOn(bridge, "listProjects").mockImplementation(async () => catalog);
    vi.spyOn(bridge, "getRecoveryCandidate").mockResolvedValue(null);
    const saveSpy = vi.spyOn(bridge, "saveProject").mockImplementation(async () => {
      catalog = [project];
      return project;
    });
    const deleteSpy = vi.spyOn(bridge, "deleteProject").mockImplementation(async () => {
      catalog = [];
    });
    mockDatasetLoad(dataset);

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    expect(await screen.findByRole("heading", { name: "ventas.csv" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Cargar" }));

    const projectName = await screen.findByRole("textbox", { name: "Nombre del proyecto" });
    fireEvent.change(projectName, { target: { value: project.name } });
    fireEvent.click(screen.getByRole("button", { name: "Guardar proyecto nuevo" }));
    await waitFor(() => expect(saveSpy).toHaveBeenCalledWith(
      null,
      project.name,
      { qualityRules: [], recipeDraft: null, reviewTab: "diagnosis", previewOffset: 0, activePhase: "load", analysisSampleRows: 100_000 },
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
    const listSpy = vi.spyOn(bridge, "listProjects").mockImplementation(async () => catalog);
    vi.spyOn(bridge, "getRecoveryCandidate").mockResolvedValue(null);
    const saveSpy = vi.spyOn(bridge, "saveProject").mockImplementation(async () => {
      catalog = [project];
      return project;
    });
    const openSpy = vi.spyOn(bridge, "openProject").mockResolvedValue({
      project,
      dataset,
      workspace: { qualityRules: [{ column: "email", kind: "not_null", maxInvalid: 0 }], recipeDraft: null, reviewTab: "preview", previewOffset: 50, activePhase: "prepare", analysisSampleRows: 50_000 },
      profile: { rowCount: 1, duplicateRowCount: 0, nearDuplicateRowCount: 0, duplicatePercentage: 0, columns: [] },
    });
    const pageSpy = vi.spyOn(bridge, "getDatasetPage").mockResolvedValue({
      offset: 50,
      rows: [["lucia@example.com"]],
    });
    mockDatasetLoad(dataset);

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    fireEvent.click(await screen.findByRole("button", { name: "Cargar" }));

    const projectName = await screen.findByRole("textbox", { name: "Nombre del proyecto" });
    fireEvent.change(projectName, { target: { value: project.name } });
    fireEvent.click(screen.getByRole("button", { name: "Guardar proyecto nuevo" }));
    await waitFor(() => expect(saveSpy).toHaveBeenCalledWith(
      null,
      project.name,
      { qualityRules: [], recipeDraft: null, reviewTab: "diagnosis", previewOffset: 0, activePhase: "load", analysisSampleRows: 100_000 },
    ));
    await waitFor(() => expect(listSpy.mock.calls.length).toBeGreaterThanOrEqual(2));

    fireEvent.click(await screen.findByRole("button", { name: "Abrir" }));
    await waitFor(() => expect(openSpy).toHaveBeenCalledWith(project.id));
    expect(await screen.findByRole("heading", { name: "clientes.csv" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Preparar" })).toHaveAttribute("aria-current", "step");
    fireEvent.click(screen.getByRole("button", { name: "Revisar" }));
    expect(screen.getByRole("tab", { name: "Vista previa" })).toHaveAttribute("aria-selected", "true");
    fireEvent.click(screen.getByRole("tab", { name: "Diagnóstico" }));
    expect(screen.getByRole("combobox", { name: "Filas de muestra para correlaciones" })).toHaveValue("50000");
    fireEvent.click(screen.getByRole("tab", { name: "Vista previa" }));
    expect(await screen.findByRole("cell", { name: "lucia@example.com" })).toBeInTheDocument();
    expect(screen.getByText(/Filas 51–51 de 75/)).toBeInTheDocument();
    expect(pageSpy).toHaveBeenCalledWith(50, 50);

    await switchPhase("Entregar");
    expect(screen.getByRole("combobox", { name: "Columna regla 1" })).toHaveValue("email");
  });

  it("mantiene el perfil en idle cuando el proyecto no incluye uno durable", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.26.0", platform: "windows" });
    const project: ProjectSummary = {
      id: "project-without-profile", name: "Sin perfil", datasetFileName: "simple.csv",
      rowCount: 1, columnCount: 1, createdAt: "2026-08-20T00:00:00Z", updatedAt: "2026-08-21T00:00:00Z",
    };
    vi.spyOn(bridge, "listProjects").mockResolvedValue([project]);
    vi.spyOn(bridge, "getRecoveryCandidate").mockResolvedValue(null);
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

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Abrir" }));

    expect(await screen.findByRole("button", { name: "Analizar calidad" })).toBeInTheDocument();
    expect(screen.queryByText("Filas analizadas")).not.toBeInTheDocument();
  });

  it("explica cómo conectar el motor cuando se abre en navegador", async () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "Columnia" })).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Flujo de preparación de datos" })).toBeInTheDocument();
    expect(screen.getByRole("progressbar", { name: "Progreso del flujo" })).toHaveAttribute("aria-valuenow", "1");
    expect(screen.getByRole("button", { name: "Continuar a Revisar" })).toBeDisabled();
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
    mockDatasetLoad({
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

    expect(await screen.findByRole("heading", { name: "temperaturas.csv" })).toBeInTheDocument();
    expect(screen.getByRole("progressbar", { name: "Progreso del flujo" })).toHaveAttribute("aria-valuetext", "Paso 2 de 4: Revisar");
    expect(screen.getByRole("button", { name: "Continuar a Preparar" })).toBeEnabled();
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

    render(<App />);
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

    render(<App />);
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
    fireEvent.click(screen.getByRole("button", { name: "Analizar calidad" }));

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
    expect(await screen.findByRole("button", { name: "Analizar de nuevo" })).toBeInTheDocument();
  });

  it("conserva el dataset activo cuando se cancela una sustitución", async () => {
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
      .mockResolvedValueOnce({ selectionId: "selection-active", fileName: "activo.csv", fileSizeBytes: 128, format: "csv", sheets: [], defaultSheetId: null, isCompressedContainer: false })
      .mockResolvedValueOnce({ selectionId: "selection-replacement", fileName: "nuevo.csv", fileSizeBytes: 128, format: "csv", sheets: [], defaultSheetId: null, isCompressedContainer: false });
    vi.spyOn(bridge, "loadDatasetSelection")
      .mockResolvedValueOnce(activeDataset)
      .mockImplementationOnce((_selectionId, _sheetId, _headerMode, onProgress) => {
        onProgress?.({ operation: "load", stage: "Leyendo y detectando columnas", percent: 25 });
        return replacementPromise;
      });
    const cancelSpy = vi.spyOn(bridge, "cancelOperation").mockResolvedValue(undefined);

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await screen.findByRole("heading", { name: "activo.csv" });
    fireEvent.click(screen.getByRole("button", { name: "Cargar" }));
    fireEvent.click(screen.getByRole("button", { name: "Seleccionar otro dataset" }));
    fireEvent.click(await screen.findByRole("button", { name: "Cancelar" }));

    expect(cancelSpy).toHaveBeenCalledWith("load");
    expect(screen.getByRole("button", { name: "Cancelando…" })).toBeDisabled();

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

    render(<App />);
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

    expect(
      await screen.findByText(/Parquet exportado como ventas-columnia\.parquet/),
    ).toBeInTheDocument();
    expect(screen.getByText(/2\.0 KB/)).toBeInTheDocument();
    expect(exportSpy).toHaveBeenCalledOnce();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
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

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Entregar");
    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    fireEvent.click(screen.getByRole("button", { name: "Validar contrato" }));

    expect(await screen.findByText("Contrato aprobado")).toBeInTheDocument();
    expect(validationSpy).toHaveBeenCalledWith([{ column: "total", kind: "not_null", maxInvalid: 0 }]);
    fireEvent.click(screen.getByRole("button", { name: "Exportar CSV" }));
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

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Entregar");
    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    fireEvent.click(screen.getByRole("button", { name: "Validar contrato" }));
    expect(await screen.findByText("Contrato fallido")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Exportar CSV" })).toBeDisabled();

    fireEvent.click(screen.getByRole("button", { name: "Validar contrato" }));
    expect(await screen.findByText("Contrato aprobado")).toBeInTheDocument();
    fireEvent.change(screen.getByRole("spinbutton", { name: "Inválidos regla 1" }), { target: { value: "1" } });
    expect(screen.getByText("Resultado desactualizado")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Exportar CSV" })).toBeDisabled();
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
    const normalizeSpy = vi.spyOn(bridge, "normalizeColumnNames").mockResolvedValue({
      dataset: {
        ...original,
        columns: [{ name: "ano_venta", dataType: "Int64" }],
      },
      renamedColumnCount: 1,
      renames: [{ from: "Año Venta", to: "ano_venta" }],
    });

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("button", { name: "Normalizar columnas" }));

    expect(await screen.findByText("Se normalizó 1 nombre de columna.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Deshacer" })).toBeInTheDocument();
    expect(normalizeSpy).toHaveBeenCalledOnce();

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
    const trimSpy = vi.spyOn(bridge, "trimTextValues").mockResolvedValue({
      dataset: { ...original, rows: [["Bogotá", "A1"]] },
      affectedRowCount: 1,
      changedCellCount: 1,
      changedColumns: [{ name: "city", changedCellCount: 1 }],
    });
    const normalizeSpy = vi.spyOn(bridge, "normalizeTextValues").mockResolvedValue({
      dataset: { ...original, rows: [["bogota", "A1"]] },
      affectedRowCount: 1,
      changedCellCount: 1,
      changedColumns: [{ name: "city", changedCellCount: 1 }],
    });

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("button", { name: "Recortar espacios" }));

    expect(await screen.findByText("Se recortaron espacios en 1 celda en 1 fila.")).toBeInTheDocument();
    expect(trimSpy).toHaveBeenCalledOnce();

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
      renamedColumnCount: 1,
      renames: [{ from: "Ciudad Nombre", to: "ciudad_nombre" }],
    });

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("button", { name: "Aplicar recomendadas" }));

    expect(
      await screen.findByText(/Correcciones recomendadas aplicadas: 1 celda recortada y 1 columna renombrada/),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Deshacer" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Rehacer" })).toBeDisabled();
    expect(applySpy).toHaveBeenCalledOnce();
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
    const removeSpy = vi.spyOn(bridge, "removeDuplicates").mockResolvedValue({
      affectedRowCount: 1,
      dataset: {
        fileName: "calidad.csv",
        fileSizeBytes: 1024,
        rowCount: 2,
        columnCount: 1,
        columns: [{ name: "temperature", dataType: "Int64" }],
        rows: [["30"], ["28"]],
      },
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

    render(<App />);
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
    fireEvent.click(screen.getByRole("button", { name: "Eliminar duplicados" }));
    expect(
      await screen.findByText("Se eliminaron 1 filas duplicadas adicionales."),
    ).toBeInTheDocument();
    expect(removeSpy).toHaveBeenCalledOnce();

    fireEvent.click(screen.getByRole("button", { name: "Deshacer" }));
    expect(await screen.findByRole("button", { name: "Analizar antes de preparar" })).toBeInTheDocument();
    expect(undoSpy).toHaveBeenCalledOnce();

    fireEvent.click(screen.getByRole("button", { name: "Rehacer" }));
    expect(await screen.findByText("Se rehízo el último cambio.")).toBeInTheDocument();
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

    render(<App />);
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

    render(<App />);
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

    render(<App />);
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
    });
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
    expect(within(dialog).getByRole("note")).toHaveTextContent(/ocupar bastante más memoria/);
    const sheetSelect = within(dialog).getByLabelText("Hoja");
    const loadSheet = within(dialog).getByRole("button", { name: "Cargar hoja" });
    expect(sheetSelect).toHaveFocus();
    loadSheet.focus();
    fireEvent.keyDown(loadSheet, { key: "Tab" });
    expect(sheetSelect).toHaveFocus();
    fireEvent.keyDown(sheetSelect, { key: "Tab", shiftKey: true });
    expect(loadSheet).toHaveFocus();
    expect(loadSpy).not.toHaveBeenCalled();
    fireEvent.change(sheetSelect, { target: { value: "1" } });
    fireEvent.click(within(dialog).getByRole("radio", { name: /Generar encabezados/ }));
    fireEvent.click(loadSheet);

    expect(await screen.findByRole("heading", { name: "ventas.xlsx" })).toBeInTheDocument();
    expect(loadSpy).toHaveBeenCalledWith("opaque-workbook-1", "1", "generated", expect.any(Function));
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

    render(<App />);
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
    });
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
      version: 1, name: "Limpieza ventas", savedAt: "2026-08-14T12:00:00Z",
      recipe: { renames: [{ from: "estado", to: "situacion" }], casts: [], dateParses: [], filters: [], calculatedColumn: null, findReplace: null, keepColumns: null, splitColumn: null, mergeColumns: null, outlierTreatments: [], groupSummary: null, contactNormalizations: [], textExtractions: [] },
    });

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("tab", { name: "Transformaciones" }));
    expect(screen.getByRole("button", { name: "Guardar receta" })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Columna para renombrar 1"), { target: { value: "estado" } });
    fireEvent.change(screen.getByLabelText("Nuevo nombre 1"), { target: { value: "situacion" } });
    fireEvent.change(screen.getByLabelText("Nombre de la receta"), { target: { value: "Limpieza ventas" } });
    fireEvent.click(screen.getByRole("button", { name: "Guardar receta" }));

    expect(await screen.findByText(/Receta guardada: Limpieza ventas/)).toBeInTheDocument();
    expect(saveSpy).toHaveBeenCalledWith(expect.objectContaining({ renames: [{ from: "estado", to: "situacion" }] }), "Limpieza ventas", null);
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

    render(<App />);
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
    render(<App />);
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
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("tab", { name: "Transformaciones" }));
    expect(screen.getByText(/comparaciones numéricas estrictas/)).toHaveTextContent(/extrae primero año, mes o día/);
    fireEvent.click(screen.getByRole("button", { name: "+ Añadir filtro AND" }));
    fireEvent.change(screen.getByLabelText("Columna del filtro 1"), { target: { value: "estado" } });
    fireEvent.change(screen.getByLabelText("Operador del filtro 1"), { target: { value: "not_null" } });
    expect(screen.getByLabelText("Valor del filtro 1")).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox", { name: "Crear una columna en esta receta" }));
    fireEvent.change(screen.getByLabelText("Nombre de la columna calculada"), { target: { value: "doble" } });
    fireEvent.change(screen.getByLabelText("Columna origen del cálculo"), { target: { value: "total" } });
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
    fireEvent.click(screen.getByRole("checkbox", { name: "Eliminar columnas origen" }));
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
    }));
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
    render(<App />);
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
    fireEvent.click(screen.getByRole("checkbox", { name: "Eliminar columna origen" }));
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
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("tab", { name: "Transformaciones" }));
    fireEvent.change(screen.getByLabelText("Columna para convertir 1"), { target: { value: "importe" } });
    fireEvent.change(screen.getByLabelText("Tipo destino 1"), { target: { value: "decimal" } });
    fireEvent.click(screen.getByRole("button", { name: "+ Añadir conversión" }));
    fireEvent.change(screen.getByLabelText("Columna para convertir 2"), { target: { value: "cantidad" } });
    fireEvent.change(screen.getByLabelText("Tipo destino 2"), { target: { value: "string" } });
    fireEvent.click(screen.getByRole("button", { name: "+ Añadir tratamiento" }));
    const target = screen.getByLabelText("Columna de outliers 1");
    expect(within(target).getByRole("option", { name: "importe" })).toBeInTheDocument();
    expect(within(target).queryByRole("option", { name: "cantidad" })).not.toBeInTheDocument();
    fireEvent.change(target, { target: { value: "importe" } });
    fireEvent.click(screen.getByRole("button", { name: "+ Añadir tratamiento" }));
    fireEvent.change(screen.getByLabelText("Columna de outliers 2"), { target: { value: "nota" } });
    fireEvent.change(screen.getByLabelText("Acción de outliers 2"), { target: { value: "drop" } });
    fireEvent.click(screen.getByRole("button", { name: "Aplicar receta" }));
    let dialog = screen.getByRole("alertdialog", { name: "Confirmar cambios de alto impacto" });
    expect(within(dialog).getByText(/Se limitarán valores atípicos en 1 columnas/)).toBeInTheDocument();
    expect(within(dialog).getByText(/eliminar filas atípicas detectadas en 1 columnas/)).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancelar" }));
    expect(applySpy).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Aplicar receta" }));
    dialog = screen.getByRole("alertdialog", { name: "Confirmar cambios de alto impacto" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Confirmar y aplicar" }));
    expect(applySpy).toHaveBeenCalledWith(expect.objectContaining({ outlierTreatments: [{ column: "importe", action: "cap" }, { column: "nota", action: "drop" }] }));
  });

  it("no crea historial cuando el tratamiento IQR no encuentra outliers", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.spyOn(bridge, "getAppInfo").mockResolvedValue({ name: "Columnia", version: "0.19.0", platform: "windows" });
    const original: DatasetPreview = {
      fileName: "estable.csv", fileSizeBytes: 100, rowCount: 4, columnCount: 1,
      columns: [{ name: "valor", dataType: "Float64" }], rows: [["1"], ["2"], ["3"], ["4"]],
    };
    mockDatasetLoad(original);
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(historyState({ canUndo: false, canRedo: false, currentIndex: 0, entryCount: 1, entries: [{ index: 0, label: "Dataset cargado", isCurrent: true }] }));
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
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("tab", { name: "Transformaciones" }));
    fireEvent.click(screen.getByRole("button", { name: "+ Añadir tratamiento" }));
    fireEvent.change(screen.getByLabelText("Columna de outliers 1"), { target: { value: "valor" } });
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
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Preparar");
    fireEvent.click(screen.getByRole("tab", { name: "Transformaciones" }));
    fireEvent.change(screen.getByLabelText("Columna para convertir 1"), { target: { value: "importe" } });
    fireEvent.change(screen.getByLabelText("Tipo destino 1"), { target: { value: "decimal" } });
    fireEvent.click(screen.getByRole("checkbox", { name: "Reemplazar el dataset por un resumen" }));
    const keys = screen.getByRole("group", { name: "Columnas para agrupar" });
    fireEvent.click(within(keys).getByRole("checkbox", { name: "region" }));
    fireEvent.click(screen.getByRole("button", { name: "+ Añadir agregación" }));
    fireEvent.change(screen.getByLabelText("Columna de agregación 1"), { target: { value: "importe" } });
    expect(within(screen.getByLabelText("Operación de agregación 1")).getByRole("option", { name: "Suma" })).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Operación de agregación 1"), { target: { value: "sum" } });
    fireEvent.click(screen.getByRole("button", { name: "+ Añadir agregación" }));
    fireEvent.change(screen.getByLabelText("Columna de agregación 2"), { target: { value: "nota" } });
    fireEvent.change(screen.getByLabelText("Operación de agregación 2"), { target: { value: "count_unique" } });
    fireEvent.click(screen.getByRole("button", { name: "Aplicar receta" }));
    const dialog = screen.getByRole("alertdialog", { name: "Confirmar cambios de alto impacto" });
    expect(within(dialog).getByText(/resumen de 1 claves y 2 agregaciones sobre 6 filas/)).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancelar" }));
    expect(applySpy).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Aplicar receta" }));
    fireEvent.click(within(screen.getByRole("alertdialog")).getByRole("button", { name: "Confirmar y aplicar" }));
    expect(applySpy).toHaveBeenCalledWith(expect.objectContaining({ groupSummary: { groupBy: ["region"], aggregations: [{ column: "importe", operation: "sum" }, { column: "nota", operation: "count_unique" }] } }));
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
    render(<App />);
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
    fireEvent.change(screen.getByLabelText("Regla de extracción 1"), { target: { value: "before" } });
    fireEvent.change(screen.getByLabelText("Nombre de extracción 1"), { target: { value: "prefijo" } });
    fireEvent.change(screen.getByLabelText("Delimitador de extracción 1"), { target: { value: "-" } });
    fireEvent.click(screen.getByRole("button", { name: "Aplicar receta" }));
    const dialog = screen.getByRole("alertdialog", { name: "Confirmar cambios de alto impacto" });
    expect(within(dialog).getByText(/normalizarán valores de contacto en 1 columnas/)).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancelar" }));
    expect(applySpy).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Aplicar receta" }));
    fireEvent.click(within(screen.getByRole("alertdialog")).getByRole("button", { name: "Confirmar y aplicar" }));
    expect(applySpy).toHaveBeenCalledWith(expect.objectContaining({ contactNormalizations: [{ column: "correo", kind: "email" }], textExtractions: [{ source: "codigo", kind: "before", name: "prefijo", delimiter: "-" }] }));
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
      entryCount: 1, entries: [{ index: 0, label: "Dataset cargado", isCurrent: true }],
    }));
    render(<App />);
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
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await switchPhase("Entregar");
    fireEvent.click(screen.getByRole("checkbox", {
      name: "Confirmo que quiero exportar sin validar la calidad",
    }));
    fireEvent.change(screen.getByRole("combobox", { name: "Formato de exportación" }), {
      target: { value: "bundle" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Exportar Paquete ZIP" }));
    await waitFor(() => expect(screen.getByText(/Paquete Columnia exportado como entrega\.zip/)).toBeInTheDocument());
    expect(exportSpy).toHaveBeenCalledWith("bundle", [], true, expect.anything(), "none");

    exportSpy.mockResolvedValueOnce(null);
    fireEvent.click(screen.getByRole("button", { name: "Exportar Paquete ZIP" }));
    await waitFor(() => expect(screen.queryByText(/Paquete Columnia exportado/)).not.toBeInTheDocument());

    exportSpy.mockRejectedValueOnce(new Error("operación cancelada por el usuario"));
    fireEvent.click(screen.getByRole("button", { name: "Exportar Paquete ZIP" }));
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());

    exportSpy.mockRejectedValueOnce(new Error("disco lleno"));
    fireEvent.click(screen.getByRole("button", { name: "Exportar Paquete ZIP" }));
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("disco lleno"));
  });

  it("expone el estado web y la ficha local de licencia y privacidad", () => {
    render(<App />);
    expect(screen.getByText("Vista web · motor no conectado")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Licencia y privacidad"));
    expect(screen.getByText("funciona localmente y no envía datasets a servicios externos.")).toBeInTheDocument();
    expect(screen.getByText("Licencia", { selector: "h2" })).toBeInTheDocument();
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
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
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
      .mockResolvedValue([]);
    vi.spyOn(bridge, "getRecoveryCandidate").mockResolvedValue(null);
    const pick = vi.spyOn(bridge, "pickDatasetSource").mockResolvedValue(null);
    localStorage.setItem("columnia.recent-datasets", JSON.stringify([
      { id: "recent-1", fileName: "C:\\datos\\ventas.csv", format: "csv", lastOpenedAt: 2 },
      { id: "recent-2", fileName: "clientes.json", format: "json", lastOpenedAt: 1 },
    ]));

    render(<App />);
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("catálogo temporalmente"));
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
});
