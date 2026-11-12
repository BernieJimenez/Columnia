import { expect, test } from "@playwright/test";

import type { DatasetPreview, ExplorePanel, ExportResult } from "../src/bridge";
import { installTauriMock, recordedCalls } from "./support/tauri-mock";

const dataset = {
  fileName: "dbo.Ventas.parquet",
  fileSizeBytes: 2048,
  rowCount: 3,
  columnCount: 3,
  columns: [{ name: "ID", dataType: "String" }, { name: "Producto", dataType: "String" }, { name: "Region", dataType: "String" }],
  rows: [["1", "Lentejas", "San Cristóbal"], ["2", "Pan", "Monseñor Nouel"], ["3", "Café", "Santiago"]],
} satisfies DatasetPreview;

// The interface sends fields; Rust builds the ODBC connection string.
const connection = {
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

test.describe("cargar desde SQL Server", () => {
  test("lista las tablas, trae la elegida y la abre por el flujo normal de Cargar", async ({ page }) => {
    await installTauriMock(page, { dataset, format: "parquet" });
    await page.goto("/", { waitUntil: "commit" });

    await page.getByRole("radio", { name: /^Base de datos/ }).check();
    await expect(page.getByRole("radio", { name: "SQL Server", exact: true })).toBeChecked();
    // A form instead of an ODBC string; the driver is the installed one.
    await expect(page.getByText("Controlador: ODBC Driver 18 for SQL Server.")).toBeVisible();
    await page.getByLabel("Servidor", { exact: true }).fill("localhost");
    await page.getByLabel("Base de datos", { exact: true }).fill("VentasRetail");
    await page.getByRole("button", { name: "Ver tablas" }).click();
    await expect(page.getByText("2 tablas y vistas disponibles.")).toBeVisible();

    await page.getByLabel("Tabla o vista", { exact: true }).selectOption({ label: "Ventas" });
    await page.getByRole("button", { name: "Traer dbo.Ventas" }).click();
    await expect(page.getByText(/Se trajeron 3 filas de dbo\.Ventas/)).toBeVisible();

    const calls = await recordedCalls(page);
    const importCall = calls.find((call) => call.command === "import_database_table");
    expect(importCall?.args).toMatchObject({
      source: { kind: "sqlserver", connection },
      schema: "dbo",
      table: "Ventas",
      rowLimit: 100_000,
    });

    await page.screenshot({ path: test.info().outputPath("tras-importar.png"), fullPage: true });
    const dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();
    await dialog.getByRole("button", { name: /Cargar/ }).first().click();
    await expect(page.locator(".dataset-bar__name")).toHaveText("dbo.Ventas.parquet");
  });
});

const salesDataset = {
  fileName: "ventas-e2e.csv",
  fileSizeBytes: 128,
  rowCount: 2,
  columnCount: 3,
  columns: [{ name: "id", dataType: "String" }, { name: "cliente", dataType: "String" }, { name: "total", dataType: "Float64" }],
  rows: [["1", "Ana", "10"], ["2", "Luis", "20"]],
} satisfies DatasetPreview;

const salesExplore = {
  rowCount: salesDataset.rowCount,
  totalRowCount: salesDataset.rowCount,
  kpis: [{ kind: "count", column: null, value: salesDataset.rowCount }],
  categories: [],
  histogram: null,
  trend: null,
  options: { categories: [], measures: [], dates: [], textDates: [] },
} satisfies ExplorePanel;

const scriptResult = {
  fileName: "Pedidos.sql",
  fileSizeBytes: 512,
  format: "SQL",
  protectedColumnCount: 0,
  protectedColumns: [],
  replacedControlCellCount: 0,
} satisfies ExportResult;

test.describe("entregar como script para SQL Server", () => {
  test("manda a export_dataset el modo y las claves de la tabla elegida", async ({ page }) => {
    await installTauriMock(page, { dataset: salesDataset, explore: salesExplore, exportResult: scriptResult });
    await page.goto("/", { waitUntil: "commit" });

    await page.getByRole("button", { name: "Seleccionar dataset" }).click();
    await page.getByRole("dialog").getByRole("button", { name: "Cargar archivo" }).click();
    await page.getByRole("button", { name: "Ver cambios propuestos" }).click();
    await page.getByRole("button", { name: "Continuar a Explorar" }).click();
    await page.getByRole("button", { name: "Continuar a Entregar" }).click();
    await expect(page.getByRole("heading", { name: "Exportar dataset activo" })).toBeVisible();

    await page.getByRole("radio", { name: /^SQL Server/ }).check();
    await expect(page.getByRole("combobox", { name: "Formato de exportación" })).toHaveValue("sqlserver_script");
    const target = page.getByRole("group", { name: "Tabla en SQL Server" });
    await target.getByRole("textbox", { name: "Esquema" }).fill("ventas");
    await target.getByRole("textbox", { name: "Tabla" }).fill("Pedidos");
    await target.getByRole("radio", { name: /^Actualizar por clave/ }).check();
    await page.getByRole("checkbox", { name: "Confirmo que quiero exportar sin validar la calidad" }).check();
    // Without a key column the script cannot run: the button stays blocked.
    const exportButton = page.getByRole("button", { name: "Exportar script T-SQL" });
    await expect(exportButton).toHaveAttribute("aria-disabled", "true");
    await target.getByRole("checkbox", { name: "id" }).check();
    await expect(exportButton).not.toHaveAttribute("aria-disabled", "true");

    await exportButton.click();
    await expect(page.getByRole("region", { name: "Etapa Entregar" })).toContainText("Pedidos.sql");

    const exportCall = (await recordedCalls(page)).find((call) => call.command === "export_dataset");
    expect(exportCall?.args).toMatchObject({
      format: "sqlserver_script",
      sqlServer: { schema: "ventas", table: "Pedidos", mode: "upsert", keyColumns: ["id"], textSize: "fitted" },
    });
  });
});
