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
    type StoredTask = {
      name: string;
      importProfile: { schema: Array<{ name: string; dataType: string }> };
      recipe: unknown;
      qualityRules: unknown[];
      outputFormat: string;
      [key: string]: unknown;
    };
    type StoredTaskSummary = {
      id: string;
      name: string;
      createdAt: string;
      updatedAt: string;
      inputColumnCount: number;
      hasRecipe: boolean;
      qualityRuleCount: number;
      outputFormat: string;
    };
    let reusableTask: { id: string; task: StoredTask; summary: StoredTaskSummary } | null = null;
    let callbackId = 0;

    const invoke = async (command: string, args: Record<string, unknown> = {}) => {
      calls.push({ command, args });
      switch (command) {
        case "get_app_info":
          return { name: "Columnia", version: "0.168.0", platform: "windows" };
        case "list_sample_datasets":
          return [];
        case "list_reusable_tasks":
          return reusableTask ? [reusableTask.summary] : [];
        case "list_projects":
          return { projects: [], recoveryCandidate: null };
        case "save_reusable_task": {
          const task = args.task as StoredTask;
          const id = typeof args.taskId === "string" ? args.taskId : "task-import-format-e2e";
          const now = "2026-09-21T00:00:00.000Z";
          const summary: StoredTaskSummary = {
            id,
            name: task.name,
            createdAt: reusableTask?.summary.createdAt ?? now,
            updatedAt: now,
            inputColumnCount: task.importProfile.schema.length,
            hasRecipe: task.recipe !== null,
            qualityRuleCount: task.qualityRules.length,
            outputFormat: task.outputFormat,
          };
          reusableTask = { id, task, summary };
          return summary;
        }
        case "open_reusable_task":
          if (!reusableTask || args.taskId !== reusableTask.id) throw new Error("Tarea no encontrada");
          return reusableTask.task;
        case "check_reusable_task_schema": {
          if (!reusableTask || args.taskId !== reusableTask.id) throw new Error("Tarea no encontrada");
          const actual = args.schema as StoredTask["importProfile"]["schema"];
          const expected = reusableTask.task.importProfile.schema;
          const actualByName = new Map(actual.map((column) => [column.name, column.dataType]));
          const expectedNames = new Set(expected.map((column) => column.name));
          const missingColumns = expected.filter((column) => !actualByName.has(column.name)).map((column) => column.name);
          const addedColumns = actual.filter((column) => !expectedNames.has(column.name)).map((column) => column.name);
          const changedTypes = expected.flatMap((column) => {
            const actualType = actualByName.get(column.name);
            return actualType && actualType !== column.dataType
              ? [{ column: column.name, expected: column.dataType, actual: actualType }]
              : [];
          });
          const orderChanged = expected.map((column) => column.name).join("\u0000")
            !== actual.map((column) => column.name).join("\u0000");
          return {
            status: missingColumns.length || addedColumns.length || changedTypes.length || orderChanged
              ? "review_required"
              : "ready",
            missingColumns,
            addedColumns,
            changedTypes,
            orderChanged,
          };
        }
        case "delete_reusable_task":
          reusableTask = null;
          return null;
        case "pick_dataset_source":
          return source;
        case "inspect_workbook_sheets":
          return source.sheets;
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

  const taskCalls = await page.evaluate(() => {
    const state = (window as Window & {
      __COLUMNIA_IMPORT_FORMAT_E2E__?: {
        calls: Array<{ command: string; args: Record<string, unknown> }>;
      };
    }).__COLUMNIA_IMPORT_FORMAT_E2E__;
    return state?.calls.filter((call) =>
      call.command === "save_reusable_task" || call.command === "check_reusable_task_schema") ?? [];
  });
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
      taskId: "task-import-format-e2e",
      schema: fixture.columns,
    },
  });
});
