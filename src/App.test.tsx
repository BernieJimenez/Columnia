import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { App } from "./App";
import * as bridge from "./bridge";
import type { DatasetPreview, DatasetProfile, HistoryState } from "./bridge";

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
  it("explica cómo conectar el motor cuando se abre en navegador", async () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "Columnia" })).toBeInTheDocument();
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
    expect(screen.queryByRole("button", { name: "Seleccionar dataset" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Exportar CSV" })).not.toBeInTheDocument();
    expect(screen.getByText("2.0 KB")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("tab", { name: "Vista previa" }));
    expect(screen.getByRole("cell", { name: "Santo Domingo" })).toBeInTheDocument();
    expect(screen.getByText("null")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Cargar" }));
    expect(screen.getByRole("button", { name: "Seleccionar otro dataset" })).toBeInTheDocument();
    expect(screen.getByText(/Se admiten CSV, TSV, TXT delimitado, JSON, Parquet, Excel y ODS de hasta 500 MB/)).toBeInTheDocument();
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
      async (format, onProgress) => {
        onProgress?.({ operation: "export", stage: "Escribiendo dataset", percent: 25 });
        expect(format).toBe("parquet");
        return {
          fileName: "ventas-columnia.parquet",
          fileSizeBytes: 2048,
          format: "Parquet",
        };
      },
    );

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    await screen.findByRole("heading", { name: "ventas.csv" });
    expect(screen.queryByRole("button", { name: "Exportar Parquet" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Entregar" }));
    fireEvent.click(screen.getByRole("button", { name: "Exportar Parquet" }));

    expect(
      await screen.findByText(/Parquet exportado como ventas-columnia\.parquet/),
    ).toBeInTheDocument();
    expect(screen.getByText(/2\.0 KB/)).toBeInTheDocument();
    expect(exportSpy).toHaveBeenCalledOnce();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
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
    fireEvent.click(await screen.findByRole("button", { name: "Preparar" }));
    fireEvent.click(screen.getByRole("button", { name: "Normalizar columnas" }));

    expect(await screen.findByText("Se normalizó 1 nombre de columna.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Deshacer" })).toBeInTheDocument();
    expect(normalizeSpy).toHaveBeenCalledOnce();

    fireEvent.click(screen.getByRole("button", { name: "Revisar" }));
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
    fireEvent.click(await screen.findByRole("button", { name: "Preparar" }));
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
    fireEvent.click(await screen.findByRole("button", { name: "Preparar" }));
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
          standardDeviation: 1.414,
          firstQuartile: 28.5,
          median: 29,
          thirdQuartile: 29.5,
          outlierCount: 0,
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

    fireEvent.click(screen.getByRole("button", { name: "Preparar" }));
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
          standardDeviation: null,
          firstQuartile: null,
          median: null,
          thirdQuartile: null,
          outlierCount: null,
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
          standardDeviation: null,
          firstQuartile: null,
          median: null,
          thirdQuartile: null,
          outlierCount: null,
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
          standardDeviation: 39.592,
          firstQuartile: 11,
          median: 12,
          thirdQuartile: 13,
          outlierCount: 1,
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
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    const dialog = await screen.findByRole("dialog", { name: /Elegir hoja de ventas.xlsx/ });
    expect(within(dialog).getByRole("option", { name: "Ventas 2026" })).toBeInTheDocument();
    expect(within(dialog).getByRole("note")).toHaveTextContent(/ocupar bastante más memoria/);
    expect(loadSpy).not.toHaveBeenCalled();
    fireEvent.change(within(dialog).getByLabelText("Hoja"), { target: { value: "1" } });
    fireEvent.click(within(dialog).getByRole("radio", { name: /Generar encabezados/ }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Cargar hoja" }));

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
    fireEvent.click(await screen.findByRole("button", { name: "Preparar" }));

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
    fireEvent.click(await screen.findByRole("button", { name: "Preparar" }));
    fireEvent.click(screen.getByRole("tab", { name: "Transformaciones" }));
    expect(screen.getByRole("button", { name: "Guardar receta" })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Columna para renombrar 1"), { target: { value: "estado" } });
    fireEvent.change(screen.getByLabelText("Nuevo nombre 1"), { target: { value: "situacion" } });
    fireEvent.change(screen.getByLabelText("Nombre de la receta"), { target: { value: "Limpieza ventas" } });
    fireEvent.click(screen.getByRole("button", { name: "Guardar receta" }));

    expect(await screen.findByText(/Receta guardada: Limpieza ventas/)).toBeInTheDocument();
    expect(saveSpy).toHaveBeenCalledWith(expect.objectContaining({ renames: [{ from: "estado", to: "situacion" }] }), "Limpieza ventas");
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
      findReplace: { scope: "column", column: "estado", find: "P", replace: "Pendiente" }, keepColumns: ["total", "estado", "correo", "fecha"],
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
    fireEvent.click(await screen.findByRole("button", { name: "Preparar" }));
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
    fireEvent.click(await screen.findByRole("button", { name: "Preparar" }));
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
    fireEvent.click(await screen.findByRole("button", { name: "Preparar" }));
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
    fireEvent.click(screen.getByRole("button", { name: "Aplicar receta" }));
    let dialog = screen.getByRole("alertdialog", { name: "Confirmar cambios de alto impacto" });
    expect(within(dialog).getByText(/1 filtros unidos por AND sobre 20 filas/)).toBeInTheDocument();
    expect(within(dialog).getByText(/En total se eliminarán 2 columnas originales/)).toBeInTheDocument();
    expect(applySpy).not.toHaveBeenCalled();
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancelar" }));
    expect(applySpy).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Aplicar receta" }));
    dialog = screen.getByRole("alertdialog", { name: "Confirmar cambios de alto impacto" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Confirmar y aplicar" }));
    expect(applySpy).toHaveBeenCalledOnce();
    expect(applySpy).toHaveBeenCalledWith(expect.objectContaining({
      filters: [{ column: "estado", operator: "not_null", value: null }],
      calculatedColumn: { name: "doble", source: "total", operation: "multiply", operand: { kind: "literal", value: "2" } },
      findReplace: { scope: "column", column: "estado", find: " ", replace: "" },
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
    fireEvent.click(await screen.findByRole("button", { name: "Preparar" }));
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
    fireEvent.click(await screen.findByRole("button", { name: "Preparar" }));
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
    fireEvent.click(await screen.findByRole("button", { name: "Preparar" }));
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
      convertedColumnCount: 1, parsedDateColumnCount: 0, removedRowCount: 0,
      calculatedColumnCount: 0, replacedCellCount: 0, droppedColumnCount: 0,
      splitColumnCount: 0, mergedColumnCount: 0, droppedSourceColumnCount: 0,
      adjustedOutlierCellCount: 0, outlierRemovedRowCount: 0, outlierColumnCount: 0,
      groupCount: 2, aggregatedColumnCount: 2, collapsedRowCount: 4,
      normalizedContactCellCount: 0, normalizedContactColumnCount: 0, extractedColumnCount: 0,
    });
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    fireEvent.click(await screen.findByRole("button", { name: "Preparar" }));
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
      parsedDateColumnCount: 0, removedRowCount: 0, calculatedColumnCount: 0, replacedCellCount: 0,
      droppedColumnCount: 0, splitColumnCount: 0, mergedColumnCount: 0, droppedSourceColumnCount: 0,
      adjustedOutlierCellCount: 0, outlierRemovedRowCount: 0, outlierColumnCount: 0,
      groupCount: 0, aggregatedColumnCount: 0, collapsedRowCount: 0,
      normalizedContactCellCount: 1, normalizedContactColumnCount: 1, extractedColumnCount: 1,
    });
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar dataset" }));
    fireEvent.click(await screen.findByRole("button", { name: "Preparar" }));
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
    fireEvent.click(await screen.findByRole("button", { name: "Preparar" }));
    expect(screen.getByText("No hay espacio disponible para snapshots.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Deshacer" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Rehacer" })).toBeDisabled();
    expect(screen.getByText("Ver etapas (1)")).toBeInTheDocument();
  });
});
