import { expect, test } from "@playwright/test";

import type { DatasetPreview } from "../src/bridge";
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
