import { formatDataType } from "../src/format";
import { expect, test, type Page } from "@playwright/test";

import type { DatasetColumn } from "../src/bridge";
import { installTauriMock, recordedCalls } from "./support/tauri-mock";

type SyntheticImport = {
  format: "csv" | "excel" | "parquet";
  fileName: string;
  columns: DatasetColumn[];
  rows: Array<Array<string | null>>;
};

const sharedColumns = [
  { name: "id", dataType: "Int64" },
  { name: "nombre", dataType: "String" },
  { name: "importe", dataType: "Float64" },
];
const sharedRows = [
  ["1001", "Ana Torres", "1250.5"],
  ["1002", "Luis Pérez", "980"],
];

const importCases: SyntheticImport[] = [
  { format: "csv", fileName: "ventas-sinteticas.csv", columns: sharedColumns, rows: sharedRows },
  { format: "excel", fileName: "ventas-sinteticas.xlsx", columns: sharedColumns, rows: sharedRows },
  { format: "parquet", fileName: "ventas-sinteticas.parquet", columns: sharedColumns, rows: sharedRows },
];

async function installSyntheticImportBridge(page: Page, fixture: SyntheticImport) {
  await installTauriMock(page, {
    dataset: {
      fileName: fixture.fileName,
      fileSizeBytes: 256,
      rowCount: fixture.rows.length,
      columnCount: fixture.columns.length,
      columns: fixture.columns,
      rows: fixture.rows,
    },
    format: fixture.format,
    sheets: fixture.format === "excel" ? [{ id: "sheet-sales", name: "Ventas", hidden: false, mergedCellCount: 0 }] : [],
    // The engine's verdict for an unchanged schema; computing it here would
    // test the mock instead of the app (QA-06).
    taskSchema: { status: "ready", missingColumns: [], addedColumns: [], changedTypes: [], orderChanged: false },
  });
}

for (const fixture of importCases) {
  test(`activa ${fixture.format.toUpperCase()} con el esquema y los encabezados esperados`, async ({ page }) => {
    await installSyntheticImportBridge(page, fixture);
    await page.goto("/", { waitUntil: "commit" });

    await page.getByRole("button", { name: "Seleccionar dataset" }).click();

    if (fixture.format === "csv") {
      const dialog = page.getByRole("dialog", { name: `Revisar encabezados de ${fixture.fileName}` });
      await expect(dialog).toBeVisible();
      const importOptions = dialog.locator("details.sheet-import-options").filter({ hasText: "Interpretación de fechas y números" });
      await expect(importOptions).not.toHaveAttribute("open", "");
      await importOptions.locator("summary").focus();
      await page.keyboard.press("Enter");
      await expect(importOptions).toHaveAttribute("open", "");
      await expect(dialog.getByRole("combobox", { name: "Fechas" })).toBeVisible();
      await expect(dialog.getByRole("combobox", { name: "Números" })).toBeVisible();
      await expect(dialog.getByRole("columnheader").first()).toContainText("id");
      await expect(dialog.getByRole("radio", { name: /primera fila como encabezados/ })).toBeChecked();
      await dialog.getByRole("button", { name: "Cargar archivo" }).click();
    } else if (fixture.format === "excel") {
      const dialog = page.getByRole("dialog", { name: `Elegir hoja de ${fixture.fileName}` });
      await expect(dialog).toBeVisible();
      await expect(dialog.getByLabel("Hoja")).toHaveValue("sheet-sales");
      await expect(dialog.getByRole("radio", { name: /primera fila como encabezados/ })).toBeChecked();
      await dialog.getByRole("button", { name: "Revisar esquema" }).click();
      await dialog.getByRole("button", { name: "Cargar hoja" }).click();
    } else {
      const dialog = page.getByRole("dialog", { name: `Revisar importación de ${fixture.fileName}` });
      await expect(dialog).toBeVisible();
      // UX-08: a Parquet reviews its schema as soon as it is selected.
      await expect(dialog.getByRole("region", { name: "Esquema detectado antes de importar" })).toBeVisible();
      await dialog.getByRole("button", { name: "Cargar archivo" }).click();
    }

    const workflow = page.getByRole("navigation", { name: "Flujo de preparación de datos" });
    await expect(workflow.getByRole("button", { name: "Revisar", exact: true })).toHaveAttribute("aria-current", "step");
    await expect(page.getByRole("heading", { name: fixture.fileName, level: 3 })).toBeVisible();

    const loadCall = (await recordedCalls(page)).find((call) => call.command === "load_dataset_selection") ?? null;
    expect(loadCall).not.toBeNull();
    expect(loadCall?.args).toMatchObject({
      selectionId: `selection-${fixture.format}`,
      sheetId: fixture.format === "excel" ? "sheet-sales" : null,
      headerMode: fixture.format === "parquet" ? null : "firstRow",
    });

    await page.getByRole("tab", { name: "Vista previa" }).click();
    const preview = page.locator('.table-region[aria-label="Vista previa del dataset"]');
    await expect(preview).toBeVisible();
    const headers = await preview.locator("thead th").evaluateAll((cells) =>
      cells.map((cell) => `${cell.querySelector("span")?.textContent ?? ""} ${cell.querySelector("small")?.textContent ?? ""}`.trim()),
    );
    expect(headers).toEqual(fixture.columns.map((column) => `${column.name} ${formatDataType(column.dataType)}`));
    const firstRow = await preview.locator("tbody tr").first().locator("td").allTextContents();
    expect(firstRow.map((value) => value.trim())).toEqual(fixture.rows[0]);
  });
}

test("revisa y permite aplicar una tarea recién guardada al esquema activo", async ({ page }) => {
  const fixture = importCases.find((item) => item.format === "csv")!;
  await installSyntheticImportBridge(page, fixture);
  await page.goto("/", { waitUntil: "commit" });

  await page.getByRole("button", { name: "Seleccionar dataset" }).click();
  const dialog = page.getByRole("dialog", { name: `Revisar encabezados de ${fixture.fileName}` });
  await expect(dialog.getByRole("button", { name: "Cargar archivo" })).toBeEnabled();
  await dialog.getByRole("button", { name: "Cargar archivo" }).click();

  const workflow = page.getByRole("navigation", { name: "Flujo de preparación de datos" });
  await expect(workflow.getByRole("button", { name: "Revisar", exact: true })).toHaveAttribute("aria-current", "step");
  await workflow.getByRole("button", { name: "Cargar", exact: true }).click();
  await page.getByText("Reutilizar una tarea").click();
  await page.getByLabel("Guardar configuración actual").fill("Ventas semanales");
  await page.getByRole("button", { name: "Guardar", exact: true }).click();

  await expect(page.getByText("Tarea “Ventas semanales” guardada en este equipo.")).toBeVisible();
  await expect(page.getByText("El esquema es compatible. Puedes aplicar la configuración guardada."))
    .toBeVisible();
  const apply = page.getByRole("button", { name: "Aplicar al dataset actual" });
  await expect(apply).toBeEnabled();
  await apply.click();
  await expect(workflow.getByRole("button", { name: "Revisar", exact: true })).toHaveAttribute("aria-current", "step");

  const taskCalls = (await recordedCalls(page)).filter((call) =>
    call.command === "save_reusable_task" || call.command === "check_reusable_task_schema");
  expect(taskCalls).toHaveLength(2);
  expect(taskCalls[0]).toMatchObject({
    command: "save_reusable_task",
    args: {
      taskId: null,
      task: {
        name: "Ventas semanales",
        importProfile: {
          format: "csv",
          schema: fixture.columns,
        },
      },
    },
  });
  expect(taskCalls[1]).toMatchObject({
    command: "check_reusable_task_schema",
    args: {
      taskId: "task-e2e",
      schema: fixture.columns,
    },
  });
});
