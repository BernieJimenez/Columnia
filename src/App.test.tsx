import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { App } from "./App";
import * as bridge from "./bridge";
import type { DatasetPreview, DatasetProfile } from "./bridge";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});
async function openQualityAndAnalyze() {
  fireEvent.click(await screen.findByRole("button", { name: "Analizar calidad" }));
}

function mockDatasetLoad(dataset: DatasetPreview) {
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
      canUndo: false,
      canRedo: true,
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
      canUndo: true,
      canRedo: false,
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
});
