import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { App } from "./App";
import * as bridge from "./bridge";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

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
        },
      ],
    });

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar CSV" }));
    fireEvent.click(await screen.findByRole("button", { name: "Analizar calidad" }));

    expect(await screen.findByRole("region", { name: "Perfil de calidad por columna" })).toBeInTheDocument();
    expect(screen.getByRole("rowheader", { name: /temperature/ })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "66.7%" })).toBeInTheDocument();
    expect(screen.getByText("1 (33.3%)")).toBeInTheDocument();
    expect(profileSpy).toHaveBeenCalledOnce();
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
        },
      ],
    });

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Seleccionar CSV" }));
    fireEvent.click(await screen.findByRole("button", { name: "Analizar calidad" }));

    expect(await screen.findByRole("region", { name: "Perfil de columnas de texto" })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "7.5" })).toBeInTheDocument();
  });
});
