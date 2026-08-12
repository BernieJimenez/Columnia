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
  fireEvent.click(await screen.findByRole("button", { name: "Calidad" }));
  fireEvent.click(await screen.findByRole("button", { name: "Analizar calidad" }));
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
    vi.spyOn(bridge, "pickAndLoadCsv").mockResolvedValue({
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
    const button = await screen.findByRole("button", { name: "Seleccionar CSV" });
    fireEvent.click(button);

    expect(await screen.findByRole("heading", { name: "temperaturas.csv" })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "Santo Domingo" })).toBeInTheDocument();
    expect(screen.getByText("2.0 KB")).toBeInTheDocument();
    expect(screen.getByText("null")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Calidad" }));

    expect(screen.queryByRole("button", { name: "Seleccionar CSV" })).not.toBeInTheDocument();
    expect(screen.queryByText(/Se admiten CSV de hasta 500 MB/)).not.toBeInTheDocument();
    expect(screen.getByText("temperaturas.csv")).toBeInTheDocument();
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
    vi.spyOn(bridge, "pickAndLoadCsv").mockResolvedValue({
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
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar CSV" }));
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

    let resolveLoad!: (dataset: DatasetPreview | null) => void;
    const loadPromise = new Promise<DatasetPreview | null>((resolve) => {
      resolveLoad = resolve;
    });
    vi.spyOn(bridge, "pickAndLoadCsv").mockImplementation((onProgress) => {
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
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar CSV" }));

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
    fireEvent.click(screen.getByRole("button", { name: "Calidad" }));
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
    expect(await screen.findByRole("button", { name: "Perfil listo" })).toBeInTheDocument();
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
    const replacementPromise = new Promise<DatasetPreview | null>((_resolve, reject) => {
      rejectReplacement = reject;
    });
    vi.spyOn(bridge, "pickAndLoadCsv")
      .mockResolvedValueOnce(activeDataset)
      .mockImplementationOnce((onProgress) => {
        onProgress?.({ operation: "load", stage: "Leyendo y detectando columnas", percent: 25 });
        return replacementPromise;
      });
    const cancelSpy = vi.spyOn(bridge, "cancelOperation").mockResolvedValue(undefined);

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar CSV" }));
    await screen.findByRole("heading", { name: "activo.csv" });
    fireEvent.click(screen.getByRole("button", { name: "Seleccionar CSV" }));
    fireEvent.click(await screen.findByRole("button", { name: "Cancelar" }));

    expect(cancelSpy).toHaveBeenCalledWith("load");
    expect(screen.getByRole("button", { name: "Cancelando…" })).toBeDisabled();

    rejectReplacement("Operación cancelada por el usuario.");
    expect(await screen.findByRole("heading", { name: "activo.csv" })).toBeInTheDocument();
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
    vi.spyOn(bridge, "pickAndLoadCsv").mockResolvedValue({
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
      fileName: "calidad.csv",
      fileSizeBytes: 1024,
      rowCount: 3,
      columnCount: 1,
      columns: [{ name: "temperature", dataType: "Int64" }],
      rows: [["30"], [null], ["28"]],
    });

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar CSV" }));
    await openQualityAndAnalyze();

    const generalProfile = await screen.findByRole("region", {
      name: "Perfil de calidad por columna",
    });
    expect(within(generalProfile).getByRole("rowheader", { name: /temperature/ })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "66.7%" })).toBeInTheDocument();
    expect(screen.getByText("1 (33.3%)")).toBeInTheDocument();
    expect(profileSpy).toHaveBeenCalledOnce();

    fireEvent.click(screen.getByRole("button", { name: "Eliminar duplicados" }));
    expect(
      await screen.findByText("Se eliminaron 1 filas duplicadas adicionales."),
    ).toBeInTheDocument();
    expect(removeSpy).toHaveBeenCalledOnce();

    fireEvent.click(screen.getByRole("button", { name: "Deshacer" }));
    expect(await screen.findByRole("button", { name: "Analizar calidad" })).toBeInTheDocument();
    expect(undoSpy).toHaveBeenCalledOnce();
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
    vi.spyOn(bridge, "pickAndLoadCsv").mockResolvedValue({
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
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar CSV" }));
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
    vi.spyOn(bridge, "pickAndLoadCsv").mockResolvedValue({
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
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar CSV" }));
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
    vi.spyOn(bridge, "pickAndLoadCsv").mockResolvedValue({
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
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar CSV" }));
    await openQualityAndAnalyze();

    expect(await screen.findByRole("region", { name: "Perfil de columnas numéricas" })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "39.592" })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "1" })).toBeInTheDocument();
  });
});
