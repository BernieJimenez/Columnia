import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as bridge from "../../bridge";
import type { DatabaseImportResult, DatasetSourceInspection } from "../../bridge";
import { DatabaseImportPanel } from "./DatabaseImportPanel";

/** What the form builds for localhost and VentasRetail with Windows sign-in. */
const connection = "Driver={ODBC Driver 18 for SQL Server};Server=localhost;Database=VentasRetail;Trusted_Connection=yes;Encrypt=yes;";

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

beforeEach(() => {
  vi.spyOn(bridge, "listSqlServerDrivers").mockResolvedValue(["ODBC Driver 18 for SQL Server", "ODBC Driver 17 for SQL Server"]);
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

type RunImport = () => Promise<DatasetSourceInspection | null>;

function renderPanel(onImport: (run: RunImport) => unknown = (run) => void run()) {
  render(<DatabaseImportPanel connected blocked={false} onImport={(run) => void onImport(run)} />);
  return onImport;
}

async function fillConnection() {
  await waitFor(() => expect(screen.getByLabelText("Controlador ODBC")).toHaveValue("ODBC Driver 18 for SQL Server"));
  fireEvent.change(screen.getByLabelText("Servidor"), { target: { value: "localhost" } });
  fireEvent.change(screen.getByLabelText("Base de datos"), { target: { value: "VentasRetail" } });
}

describe("DatabaseImportPanel", () => {
  it("fuera de la app de escritorio explica que no puede conectar", () => {
    render(<DatabaseImportPanel connected={false} blocked={false} onImport={vi.fn()} />);
    expect(screen.getByText(/solo es posible en la aplicación de escritorio/)).toBeInTheDocument();
    expect(screen.queryByLabelText("Servidor")).not.toBeInTheDocument();
  });

  it("pide servidor y autenticación en lugar de una cadena ODBC, con el controlador más nuevo", async () => {
    renderPanel();
    expect(screen.getByRole("button", { name: "Ver tablas" })).toBeDisabled();
    await fillConnection();
    expect(screen.getByRole("radio", { name: "Windows (tu usuario actual)" })).toBeChecked();
    expect(screen.getByRole("button", { name: "Ver tablas" })).toBeEnabled();

    fireEvent.click(screen.getByRole("radio", { name: "Usuario y contraseña de SQL Server" }));
    expect(screen.getByLabelText("Contraseña")).toHaveAttribute("type", "password");
    expect(screen.getByRole("button", { name: "Ver tablas" })).toBeDisabled();
    expect(screen.getByText("Indica el usuario de SQL Server.")).toBeInTheDocument();
  });

  it("deja escribir la cadena ODBC a mano, en un campo de contraseña", async () => {
    const list = vi.spyOn(bridge, "listDatabaseTables").mockResolvedValue([]);
    renderPanel();
    fireEvent.click(screen.getByRole("button", { name: "Escribir la cadena ODBC" }));
    const manual = screen.getByLabelText("Cadena de conexión ODBC");
    expect(manual).toHaveAttribute("type", "password");
    fireEvent.change(manual, { target: { value: "DSN=Ventas" } });
    fireEvent.click(screen.getByRole("button", { name: "Ver tablas" }));
    await waitFor(() => expect(list).toHaveBeenCalledWith({ kind: "sqlserver", connectionString: "DSN=Ventas" }));
  });

  it("lista las tablas por esquema y trae la elegida con el límite de filas", async () => {
    const list = vi.spyOn(bridge, "listDatabaseTables").mockResolvedValue([
      { schema: "dbo", name: "Clientes", isView: false },
      { schema: "dbo", name: "Ventas", isView: false },
      { schema: "reportes", name: "VentasPorMes", isView: true },
    ]);
    const result: DatabaseImportResult = { inspection, rowsImported: 1000, moreRowsAvailable: true, truncatedColumns: ["Notas"] };
    const importTable = vi.spyOn(bridge, "importDatabaseTable").mockResolvedValue(result);
    const onImport = vi.fn(async (run: RunImport) => {
      await expect(run()).resolves.toEqual(inspection);
    });
    renderPanel(onImport);

    await fillConnection();
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
    const onImport = vi.fn(async (run: RunImport) => {
      await expect(run()).resolves.toBeNull();
    });
    renderPanel(onImport);
    await fillConnection();
    fireEvent.click(screen.getByRole("button", { name: "Ver tablas" }));
    fireEvent.click(await screen.findByRole("button", { name: "Traer dbo.Ventas" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("el servidor rechazó la sentencia");
  });

  it("dice cuándo el usuario no puede leer ninguna tabla", async () => {
    vi.spyOn(bridge, "listDatabaseTables").mockResolvedValue([]);
    renderPanel();
    await fillConnection();
    fireEvent.click(screen.getByRole("button", { name: "Ver tablas" }));
    expect(await screen.findByText(/no muestra tablas ni vistas que este usuario pueda leer/)).toBeInTheDocument();
  });

  it("sin controlador de SQL Server explica qué instalar", async () => {
    vi.spyOn(bridge, "listSqlServerDrivers").mockResolvedValue([]);
    renderPanel();
    expect(await screen.findByText(/Instala «ODBC Driver 18 for SQL Server»/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Ver tablas" })).toBeDisabled();
  });
});
