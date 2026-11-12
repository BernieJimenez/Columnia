import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as bridge from "../../bridge";
import type { DatabaseConnection, DatabaseImportResult, DatasetSourceInspection } from "../../bridge";
import { DatabaseImportPanel } from "./DatabaseImportPanel";

/** What the form sends for localhost and VentasRetail with Windows sign-in: fields, never a string. */
const connection: DatabaseConnection = {
  driver: "ODBC Driver 18 for SQL Server",
  server: "localhost",
  port: "",
  database: "VentasRetail",
  authentication: "windows",
  user: "",
  password: "",
  trustServerCertificate: false,
  allowUnencrypted: false,
};

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
  vi.spyOn(bridge, "listDatabaseDrivers").mockImplementation(async (kind) => kind === "sqlserver"
    ? ["ODBC Driver 18 for SQL Server", "ODBC Driver 17 for SQL Server"]
    : kind === "postgresql" ? ["PostgreSQL Unicode(x64)"] : []);
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

  it("no ofrece escribir una cadena ODBC: solo campos, y el usuario hostil viaja como un valor más", async () => {
    const list = vi.spyOn(bridge, "listDatabaseTables").mockResolvedValue([]);
    renderPanel();
    await fillConnection();
    expect(screen.queryByRole("button", { name: "Escribir la cadena ODBC" })).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Cadena de conexión ODBC")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("radio", { name: "Usuario y contraseña de SQL Server" }));
    fireEvent.change(screen.getByLabelText("Usuario"), { target: { value: "sa}};Server=X};Server=Y" } });
    fireEvent.change(screen.getByLabelText("Contraseña"), { target: { value: "{a}}b;Encrypt=yes}" } });
    fireEvent.click(screen.getByRole("button", { name: "Ver tablas" }));
    await waitFor(() => expect(list).toHaveBeenCalledWith({
      kind: "sqlserver",
      connection: { ...connection, authentication: "password", user: "sa}};Server=X};Server=Y", password: "{a}}b;Encrypt=yes}" },
    }));
  });

  it("con Windows no envía el usuario ni la contraseña que se escribieron antes", async () => {
    const list = vi.spyOn(bridge, "listDatabaseTables").mockResolvedValue([]);
    renderPanel();
    await fillConnection();
    fireEvent.click(screen.getByRole("radio", { name: "Usuario y contraseña de SQL Server" }));
    fireEvent.change(screen.getByLabelText("Usuario"), { target: { value: "ana" } });
    fireEvent.change(screen.getByLabelText("Contraseña"), { target: { value: "secreta" } });
    fireEvent.click(screen.getByRole("radio", { name: "Windows (tu usuario actual)" }));
    fireEvent.click(screen.getByRole("button", { name: "Ver tablas" }));
    await waitFor(() => expect(list).toHaveBeenCalledWith({ kind: "sqlserver", connection }));
  });

  it("lista las tablas por esquema y trae la elegida con el límite de filas", async () => {
    const list = vi.spyOn(bridge, "listDatabaseTables").mockResolvedValue([
      { schema: "dbo", name: "Clientes", isView: false },
      { schema: "dbo", name: "Ventas", isView: false },
      { schema: "reportes", name: "VentasPorMes", isView: true },
    ]);
    const result: DatabaseImportResult = { inspection, rowsImported: 1000, moreRowsAvailable: true, truncatedColumns: ["Notas"], replacedColumns: ["Texto"] };
    const importTable = vi.spyOn(bridge, "importDatabaseTable").mockResolvedValue(result);
    const onImport = vi.fn(async (run: RunImport) => {
      await expect(run()).resolves.toEqual(inspection);
    });
    renderPanel(onImport);

    await fillConnection();
    fireEvent.click(screen.getByRole("button", { name: "Ver tablas" }));
    expect(await screen.findByText("3 tablas y vistas disponibles.")).toBeInTheDocument();
    expect(list).toHaveBeenCalledWith({ kind: "sqlserver", connection });
    expect(screen.getByRole("option", { name: "VentasPorMes (vista)" })).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Tabla o vista"), { target: { value: "1" } });
    fireEvent.change(screen.getByLabelText("Filas como máximo"), { target: { value: "10000" } });
    fireEvent.click(screen.getByRole("button", { name: "Traer dbo.Ventas" }));

    await waitFor(() => expect(importTable).toHaveBeenCalledWith(
      { kind: "sqlserver", connection }, "dbo", "Ventas", 10_000, expect.any(Function),
    ));
    const done = await screen.findByText(/Se trajeron/);
    expect(done).toHaveTextContent(/Se trajeron 1[.,]?000 filas de dbo\.Ventas/);
    expect(done).toHaveTextContent("La tabla tiene más filas");
    expect(screen.getByText(/«Notas» tenían más de 8\.000 caracteres/)).toBeInTheDocument();
    expect(screen.getByText(/«Texto» tenían caracteres no válidos y se sustituyeron por �/)).toBeInTheDocument();
  });

  it("avisa a la aplicación de lo que la importación dejó fuera, solo si dejó algo (AUD-061)", async () => {
    vi.spyOn(bridge, "listDatabaseTables").mockResolvedValue([{ schema: "dbo", name: "Ventas", isView: false }]);
    const importTable = vi.spyOn(bridge, "importDatabaseTable");
    const onImported = vi.fn();
    render(<DatabaseImportPanel connected blocked={false} onImport={(run) => void run()} onImported={onImported} />);
    await fillConnection();
    fireEvent.click(screen.getByRole("button", { name: "Ver tablas" }));
    await screen.findByRole("button", { name: "Traer dbo.Ventas" });

    importTable.mockResolvedValueOnce({ inspection, rowsImported: 10_000, moreRowsAvailable: true, truncatedColumns: ["Notas"], replacedColumns: [] });
    fireEvent.click(screen.getByRole("button", { name: "Traer dbo.Ventas" }));
    await waitFor(() => expect(onImported).toHaveBeenCalledWith("selection-1", {
      table: "dbo.Ventas", rowsImported: 10_000, partial: true, truncatedColumns: ["Notas"], replacedColumns: [],
    }));

    importTable.mockResolvedValueOnce({ inspection, rowsImported: 5, moreRowsAvailable: false, truncatedColumns: [], replacedColumns: [] });
    fireEvent.click(screen.getByRole("button", { name: "Traer dbo.Ventas" }));
    await waitFor(() => expect(onImported).toHaveBeenLastCalledWith("selection-1", null));
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

  it("tras «Ver tablas» el foco pasa a la lista de tablas y el resultado se anuncia en un estado persistente", async () => {
    let finish: (tables: Array<{ schema: string; name: string; isView: boolean }>) => void = () => undefined;
    const list = vi.spyOn(bridge, "listDatabaseTables").mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    renderPanel();
    await fillConnection();
    const button = screen.getByRole("button", { name: "Ver tablas" });
    button.focus();
    const status = screen.getAllByRole("status").find((node) => node.hasAttribute("tabindex") && node.textContent === "");
    expect(status).toBeDefined();
    fireEvent.click(button);

    expect(status).toHaveTextContent("Conectando…");
    expect(document.activeElement).not.toBe(document.body);
    expect(document.activeElement).toBe(button);
    fireEvent.click(button);
    expect(list).toHaveBeenCalledTimes(1);

    finish([{ schema: "dbo", name: "Ventas", isView: false }, { schema: "dbo", name: "Clientes", isView: false }]);
    await waitFor(() => expect(document.activeElement).toBe(screen.getByLabelText("Tabla o vista")));
    expect(status).toHaveTextContent("2 tablas y vistas disponibles.");
    expect(screen.getAllByRole("status").filter((node) => node.textContent?.includes("2 tablas"))).toHaveLength(1);
  });

  it("si «Ver tablas» falla, el foco pasa al mensaje de error", async () => {
    vi.spyOn(bridge, "listDatabaseTables").mockRejectedValue("No se pudo conectar con el servidor.");
    renderPanel();
    await fillConnection();
    screen.getByRole("button", { name: "Ver tablas" }).focus();
    fireEvent.click(screen.getByRole("button", { name: "Ver tablas" }));
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("No se pudo conectar con el servidor.");
    expect(document.activeElement).toBe(alert);
  });

  it("durante la importación el foco no cae a body y al terminar pasa al resultado o al error", async () => {
    vi.spyOn(bridge, "listDatabaseTables").mockResolvedValue([{ schema: "dbo", name: "Ventas", isView: false }]);
    let finish: (result: DatabaseImportResult) => void = () => undefined;
    let fail: (reason: string) => void = () => undefined;
    const importTable = vi.spyOn(bridge, "importDatabaseTable")
      .mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }))
      .mockReturnValueOnce(new Promise((_resolve, reject) => { fail = reject; }));
    renderPanel();
    await fillConnection();
    fireEvent.click(screen.getByRole("button", { name: "Ver tablas" }));
    const traer = await screen.findByRole("button", { name: "Traer dbo.Ventas" });
    traer.focus();
    fireEvent.click(traer);
    await waitFor(() => expect(importTable).toHaveBeenCalledTimes(1));
    expect(document.activeElement).toBe(traer);
    expect(traer).toHaveAttribute("aria-disabled", "true");
    fireEvent.click(traer);
    expect(importTable).toHaveBeenCalledTimes(1);

    finish({ inspection, rowsImported: 5, moreRowsAvailable: false, truncatedColumns: [], replacedColumns: [] });
    const done = await screen.findByText(/Se trajeron 5 filas/);
    await waitFor(() => expect(document.activeElement).toBe(done.closest("[role=status]")));

    traer.focus();
    fireEvent.click(traer);
    await waitFor(() => expect(importTable).toHaveBeenCalledTimes(2));
    fail("El servidor rechazó la sentencia.");
    const alert = await screen.findByRole("alert");
    await waitFor(() => expect(document.activeElement).toBe(alert));
  });

  it("sin controlador de SQL Server explica qué instalar", async () => {
    vi.spyOn(bridge, "listDatabaseDrivers").mockResolvedValue([]);
    renderPanel();
    expect(await screen.findByText(/Instala «ODBC Driver 18 for SQL Server»/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Ver tablas" })).toBeDisabled();
  });

  it("lee de PostgreSQL con usuario, contraseña y cifrado obligatorio", async () => {
    const list = vi.spyOn(bridge, "listDatabaseTables").mockResolvedValue([{ schema: "public", name: "pedidos", isView: false }]);
    renderPanel();
    fireEvent.click(screen.getByRole("radio", { name: "PostgreSQL" }));
    await waitFor(() => expect(screen.getByText("Controlador: PostgreSQL Unicode(x64).")).toBeInTheDocument());
    expect(screen.queryByRole("radio", { name: "Windows (tu usuario actual)" })).not.toBeInTheDocument();
    expect(screen.getByLabelText("Puerto")).toHaveAttribute("placeholder", "5432");
    fireEvent.change(screen.getByLabelText("Servidor"), { target: { value: "db.ejemplo.com" } });
    fireEvent.change(screen.getByLabelText("Puerto"), { target: { value: "70000" } });
    fireEvent.change(screen.getByLabelText("Usuario"), { target: { value: "ana" } });
    expect(screen.getByText("El puerto es un número entre 1 y 65535.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Ver tablas" })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Puerto"), { target: { value: "5433" } });
    fireEvent.change(screen.getByLabelText("Base de datos"), { target: { value: "tienda" } });
    fireEvent.change(screen.getByLabelText("Contraseña"), { target: { value: "a;b" } });
    fireEvent.click(screen.getByRole("button", { name: "Ver tablas" }));
    await waitFor(() => expect(list).toHaveBeenCalledWith({
      kind: "postgresql",
      connection: {
        driver: "PostgreSQL Unicode(x64)",
        server: "db.ejemplo.com",
        port: "5433",
        database: "tienda",
        authentication: "password",
        user: "ana",
        password: "a;b",
        trustServerCertificate: false,
        allowUnencrypted: false,
      },
    }));
    expect(await screen.findByRole("button", { name: "Traer public.pedidos" })).toBeEnabled();
  });

  it("al cambiar de motor vacía la conexión y dice qué controlador instalar", async () => {
    renderPanel();
    await fillConnection();
    expect(screen.getByRole("button", { name: "Ver tablas" })).toBeEnabled();
    fireEvent.click(screen.getByRole("radio", { name: "MySQL" }));
    expect(await screen.findByText(/No hay un controlador ODBC de MySQL instalado\. Instala MySQL Connector\/ODBC/)).toBeInTheDocument();
    expect(screen.getByLabelText("Servidor")).toHaveValue("");
    expect(screen.getByRole("button", { name: "Ver tablas" })).toBeDisabled();
  });

  it("asocia el motivo de ocupado al motor, la conexión y «Ver tablas» solo mientras está bloqueado (AUD-059)", () => {
    const { rerender } = render(<DatabaseImportPanel connected blocked onImport={vi.fn()} />);
    expect(screen.getByRole("group", { name: "Motor" })).toHaveAttribute("aria-describedby", "busy-reason");
    expect(screen.getByRole("group", { name: "Datos de conexión" })).toHaveAttribute("aria-describedby", "busy-reason");
    const show = screen.getByRole("button", { name: "Ver tablas" });
    expect(show).toHaveAttribute("aria-disabled", "true");
    expect(show).toHaveAttribute("aria-describedby", "busy-reason");

    rerender(<DatabaseImportPanel connected blocked={false} onImport={vi.fn()} />);
    expect(screen.getByRole("group", { name: "Motor" })).not.toHaveAttribute("aria-describedby");
    expect(screen.getByRole("group", { name: "Datos de conexión" })).not.toHaveAttribute("aria-describedby");
    expect(screen.getByRole("button", { name: "Ver tablas" })).not.toHaveAttribute("aria-describedby");
  });

  it("asocia el motivo de ocupado a la tabla y a «Traer» una vez listadas las tablas (AUD-059)", async () => {
    vi.spyOn(bridge, "listDatabaseTables").mockResolvedValue([{ schema: "dbo", name: "Ventas", isView: false }]);
    const { rerender } = render(<DatabaseImportPanel connected blocked={false} onImport={vi.fn()} />);
    await fillConnection();
    fireEvent.click(screen.getByRole("button", { name: "Ver tablas" }));
    const table = await screen.findByLabelText("Tabla o vista");
    expect(table).not.toHaveAttribute("aria-describedby");
    expect(screen.getByRole("button", { name: "Traer dbo.Ventas" })).not.toHaveAttribute("aria-describedby");

    rerender(<DatabaseImportPanel connected blocked onImport={vi.fn()} />);
    expect(screen.getByLabelText("Tabla o vista")).toHaveAttribute("aria-describedby", "busy-reason");
    expect(screen.getByLabelText("Filas como máximo")).toHaveAttribute("aria-describedby", "busy-reason");
    const bring = screen.getByRole("button", { name: "Traer dbo.Ventas" });
    expect(bring).toHaveAttribute("aria-disabled", "true");
    expect(bring).toHaveAttribute("aria-describedby", "busy-reason");
  });
});
