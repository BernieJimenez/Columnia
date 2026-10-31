import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as bridge from "../../bridge";
import type { DatabaseImportResult, DatasetSourceInspection } from "../../bridge";
import { DatabaseImportPanel } from "./DatabaseImportPanel";

const connection = "Driver={ODBC Driver 18 for SQL Server};Server=localhost;Database=VentasRetail;Trusted_Connection=yes";

const inspection = {
  selectionId: "selection-1",
  fileName: "dbo.Ventas.parquet",
  fileSizeBytes: 2048,
  format: "parquet",
  sheets: [],
  defaultSheetId: null,
  isCompressedContainer: false,
  resourceEstimate: { processingPath: "inMemory", estimatedMaterializationRamBytes: 1, estimatedTemporaryDiskBytes: null },
} as DatasetSourceInspection;

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

type RunImport = () => Promise<DatasetSourceInspection | null>;

function renderPanel(onImport: (run: RunImport) => unknown = (run) => void run()) {
  render(<DatabaseImportPanel connected blocked={false} onImport={(run) => void onImport(run)} />);
  fireEvent.click(screen.getByText("Cargar desde SQL Server"));
  return onImport;
}

describe("DatabaseImportPanel", () => {
  it("fuera de la app de escritorio explica que no puede conectar", () => {
    render(<DatabaseImportPanel connected={false} blocked={false} onImport={vi.fn()} />);
    expect(screen.getByText(/solo es posible en la aplicación de escritorio/)).toBeInTheDocument();
    expect(screen.queryByLabelText("Cadena de conexión ODBC")).not.toBeInTheDocument();
  });

  it("no guarda la cadena: es un campo de contraseña y «Ver tablas» espera a que exista", () => {
    renderPanel();
    const input = screen.getByLabelText("Cadena de conexión ODBC");
    expect(input).toHaveAttribute("type", "password");
    expect(input).toHaveAttribute("autocomplete", "off");
    expect(screen.getByRole("button", { name: "Ver tablas" })).toBeDisabled();
    fireEvent.change(input, { target: { value: connection } });
    expect(screen.getByRole("button", { name: "Ver tablas" })).toBeEnabled();
  });

  it("lista las tablas por esquema y trae la elegida con el límite de filas", async () => {
    const list = vi.spyOn(bridge, "listDatabaseTables").mockResolvedValue([
      { schema: "dbo", name: "Clientes", isView: false },
      { schema: "dbo", name: "Ventas", isView: false },
      { schema: "reportes", name: "VentasPorMes", isView: true },
    ]);
    const result: DatabaseImportResult = { inspection, rowsImported: 1000, moreRowsAvailable: true, truncatedColumns: ["Notas"] };
    const importTable = vi.spyOn(bridge, "importDatabaseTable").mockResolvedValue(result);
    const onImport = vi.fn(async (run: () => Promise<DatasetSourceInspection | null>) => {
      await expect(run()).resolves.toEqual(inspection);
    });
    renderPanel(onImport);

    fireEvent.change(screen.getByLabelText("Cadena de conexión ODBC"), { target: { value: connection } });
    fireEvent.click(screen.getByRole("button", { name: "Ver tablas" }));
    expect(await screen.findByText("3 tablas y vistas disponibles.")).toBeInTheDocument();
    expect(list).toHaveBeenCalledWith({ kind: "sqlserver", connectionString: connection });
    expect(screen.getByRole("option", { name: "VentasPorMes (vista)" })).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Tabla o vista"), { target: { value: "1" } });
    fireEvent.change(screen.getByLabelText("Filas como máximo"), { target: { value: "10000" } });
    fireEvent.click(screen.getByRole("button", { name: "Traer dbo.Ventas" }));

    await waitFor(() => expect(importTable).toHaveBeenCalledWith(
      { kind: "sqlserver", connectionString: connection }, "dbo", "Ventas", 10_000, expect.any(Function),
    ));
    const done = await screen.findByText(/Se trajeron/);
    expect(done).toHaveTextContent(/Se trajeron 1[.,]?000 filas de dbo\.Ventas/);
    expect(done).toHaveTextContent("La tabla tiene más filas");
    expect(screen.getByText(/«Notas» tenían más de 8\.000 caracteres/)).toBeInTheDocument();
  });

  it("muestra el error del servidor cerca de la acción y no carga nada", async () => {
    vi.spyOn(bridge, "listDatabaseTables").mockResolvedValue([{ schema: "dbo", name: "Ventas", isView: false }]);
    vi.spyOn(bridge, "importDatabaseTable").mockRejectedValue("No se pudo leer la tabla para SQL Server: el servidor rechazó la sentencia.");
    const onImport = vi.fn(async (run: () => Promise<DatasetSourceInspection | null>) => {
      await expect(run()).resolves.toBeNull();
    });
    renderPanel(onImport);
    fireEvent.change(screen.getByLabelText("Cadena de conexión ODBC"), { target: { value: connection } });
    fireEvent.click(screen.getByRole("button", { name: "Ver tablas" }));
    fireEvent.click(await screen.findByRole("button", { name: "Traer dbo.Ventas" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("el servidor rechazó la sentencia");
  });

  it("dice cuándo el usuario no puede leer ninguna tabla", async () => {
    vi.spyOn(bridge, "listDatabaseTables").mockResolvedValue([]);
    renderPanel();
    fireEvent.change(screen.getByLabelText("Cadena de conexión ODBC"), { target: { value: connection } });
    fireEvent.click(screen.getByRole("button", { name: "Ver tablas" }));
    expect(await screen.findByText(/no muestra tablas ni vistas que este usuario pueda leer/)).toBeInTheDocument();
  });
});
