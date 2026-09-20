import { expect, test, type Page } from "@playwright/test";

type SyntheticImport = {
  format: "csv" | "excel" | "parquet";
  fileName: string;
  columns: Array<{ name: string; dataType: string }>;
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
  await page.addInitScript((input) => {
    const source = {
      selectionId: `selection-${input.format}`,
      fileName: input.fileName,
      fileSizeBytes: 256,
      format: input.format,
      sheets: input.format === "excel" ? [{ id: "sheet-sales", name: "Ventas" }] : [],
      defaultSheetId: input.format === "excel" ? "sheet-sales" : null,
      isCompressedContainer: input.format === "excel",
      resourceEstimate: {
        processingPath: "inMemory",
        estimatedMaterializationRamBytes: 268_435_968,
        estimatedTemporaryDiskBytes: null,
      },
    };
    const dataset = {
      fileName: input.fileName,
      fileSizeBytes: 256,
      rowCount: input.rows.length,
      columnCount: input.columns.length,
      columns: input.columns,
      rows: input.rows,
    };
    const calls: Array<{ command: string; args: Record<string, unknown> }> = [];
    let callbackId = 0;

    const invoke = async (command: string, args: Record<string, unknown> = {}) => {
      calls.push({ command, args });
      switch (command) {
        case "get_app_info":
          return { name: "Columnia", version: "0.167.0", platform: "windows" };
        case "list_sample_datasets":
        case "list_reusable_tasks":
        case "list_projects":
          return [];
        case "get_recovery_candidate":
          return null;
        case "pick_dataset_source":
          return source;
        case "preview_delimited_header_review":
          return {
            delimiter: ",",
            firstRow: {
              headerMode: "firstRow",
              columns: input.columns,
              rows: input.rows,
              includesFirstRow: false,
              sampleTruncated: false,
            },
            generated: {
              headerMode: "generated",
              columns: input.columns.map((column, index) => ({
                ...column,
                name: `column_${index + 1}`,
              })),
              rows: input.rows,
              includesFirstRow: true,
              sampleTruncated: false,
            },
          };
        case "load_dataset_selection":
          return dataset;
        case "get_dataset_profile":
          return {
            rowCount: dataset.rowCount,
            duplicateRowCount: 0,
            nearDuplicateRowCount: 0,
            duplicatePercentage: 0,
            columns: [],
          };
        case "get_history_state":
          return {
            canUndo: false,
            canRedo: false,
            currentIndex: 0,
            entryCount: 0,
            entries: [],
            snapshotsEnabled: true,
            degradedReason: null,
            maxEntries: 50,
            diskBytes: 0,
            diskBudgetBytes: 536_870_912,
          };
        case "get_dataset_page":
          return { offset: args.offset ?? 0, rows: dataset.rows };
        case "clear_dataset_comparison":
          return null;
        default:
          throw new Error(`Comando Tauri no simulado: ${command}`);
      }
    };

    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {
        invoke,
        transformCallback: () => ++callbackId,
        unregisterCallback: () => undefined,
      },
    });
    Object.defineProperty(window, "__COLUMNIA_IMPORT_FORMAT_E2E__", {
      configurable: true,
      value: { calls },
    });
  }, fixture);
}

for (const fixture of importCases) {
  test(`activa ${fixture.format.toUpperCase()} con el esquema y los encabezados esperados`, async ({ page }) => {
    await installSyntheticImportBridge(page, fixture);
    await page.goto("/", { waitUntil: "commit" });

    await page.getByRole("button", { name: "Seleccionar dataset" }).click();

    if (fixture.format === "csv") {
      const dialog = page.getByRole("dialog", { name: `Revisar encabezados de ${fixture.fileName}` });
      await expect(dialog).toBeVisible();
      const importOptions = dialog.locator("details.sheet-import-options");
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
      await dialog.getByRole("button", { name: "Cargar hoja" }).click();
    }

    const workflow = page.getByRole("navigation", { name: "Flujo de preparación de datos" });
    await expect(workflow.getByRole("button", { name: "Revisar", exact: true })).toHaveAttribute("aria-current", "step");
    await expect(page.getByRole("heading", { name: fixture.fileName, level: 3 })).toBeVisible();

    const loadCall = await page.evaluate(() => {
      const state = (window as Window & {
        __COLUMNIA_IMPORT_FORMAT_E2E__?: {
          calls: Array<{ command: string; args: Record<string, unknown> }>;
        };
      }).__COLUMNIA_IMPORT_FORMAT_E2E__;
      return state?.calls.find((call) => call.command === "load_dataset_selection") ?? null;
    });
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
    expect(headers).toEqual(fixture.columns.map((column) => `${column.name} ${column.dataType}`));
    const firstRow = await preview.locator("tbody tr").first().locator("td").allTextContents();
    expect(firstRow.map((value) => value.trim())).toEqual(fixture.rows[0]);
  });
}
