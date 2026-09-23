import { chromium } from "@playwright/test";
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const portArgumentIndex = process.argv.indexOf("--port");
const port = portArgumentIndex >= 0 ? Number(process.argv[portArgumentIndex + 1]) : 9222;
const requestFileArgumentIndex = process.argv.indexOf("--request-file");
const requestFile = requestFileArgumentIndex >= 0 ? resolve(process.argv[requestFileArgumentIndex + 1]) : null;
const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const datasetPathArgumentIndex = process.argv.indexOf("--dataset-path");
const requestedDatasetPath = datasetPathArgumentIndex >= 0
  ? process.argv[datasetPathArgumentIndex + 1]
  : null;
const expectedRowCountArgumentIndex = process.argv.indexOf("--expected-row-count");
const expectedRowCount = expectedRowCountArgumentIndex >= 0
  ? Number(process.argv[expectedRowCountArgumentIndex + 1])
  : null;
const sourcePath = requestedDatasetPath
  ? resolve(requestedDatasetPath)
  : resolve(projectRoot, "fixtures", "automation", "input.csv");
const sourceFileName = basename(sourcePath);
const helperTimeoutMs = 100_000;
const probeTimeoutMs = 150_000;

if (!Number.isInteger(port) || port < 1024 || port > 65535) {
  console.log(JSON.stringify({ status: "failed", phase: "native_selectors_invalid_port" }));
  process.exit(1);
}

let browser;
let temporaryDirectory;

function sleep(milliseconds) {
  return new Promise((resolvePromise) => setTimeout(resolvePromise, milliseconds));
}

async function requestNativeDialog(mode, targetPath) {
  if (!requestFile) throw new Error("native_dialog_driver_missing");
  const requestId = `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  writeFileSync(requestFile, JSON.stringify({ requestId, mode, targetPath, status: "pending" }), "utf8");
  const deadline = Date.now() + helperTimeoutMs;
  try {
    while (Date.now() < deadline) {
      try {
        const response = JSON.parse(readFileSync(requestFile, "utf8"));
        if (response.requestId === requestId && response.status === "passed") return response;
        if (response.requestId === requestId && response.status === "failed") {
          const error = new Error(`${mode}_${response.errorCode ?? "native_dialog_driver_failed"}`);
          error.diagnostics = Array.isArray(response.diagnostics) ? response.diagnostics : [];
          throw error;
        }
      } catch (error) {
        if (error instanceof SyntaxError || error?.code === "ENOENT") {
          await sleep(150);
          continue;
        }
        throw error;
      }
      await sleep(150);
    }
    throw new Error("native_dialog_driver_timeout");
  } finally {
    try { unlinkSync(requestFile); } catch {}
  }
}

function withTimeout(promise, timeoutMs, errorCode) {
  let timeout;
  const timeoutPromise = new Promise((_, reject) => {
    timeout = setTimeout(() => reject(new Error(errorCode)), timeoutMs);
  });
  return Promise.race([promise, timeoutPromise]).finally(() => clearTimeout(timeout));
}

async function invoke(page, command, args = {}) {
  return page.evaluate(async ({ command: currentCommand, args: currentArgs }) => {
    const internals = window.__TAURI_INTERNALS__;
    if (!internals || typeof internals.invoke !== "function") {
      throw new Error("tauri_ipc_unavailable");
    }
    const invokeArgs = { ...currentArgs };
    if ((currentCommand === "export_dataset" || currentCommand === "load_dataset_selection")
      && invokeArgs.onProgress === null) {
      const callbackId = internals.transformCallback(() => {}, false);
      invokeArgs.onProgress = `__CHANNEL__:${callbackId}`;
    }
    return internals.invoke(currentCommand, invokeArgs);
  }, { command, args });
}

async function invokeWithNativeDialog(page, command, args, mode, targetPath) {
  await page.bringToFront();
  const driver = requestNativeDialog(mode, targetPath);
  const invocation = invoke(page, command, args);
  const [result] = await withTimeout(Promise.all([invocation, driver]), helperTimeoutMs, "native_dialog_timeout");
  // rfd closes the Win32 modal asynchronously after the command resolves.
  // Give that window a bounded teardown interval before the next native
  // command, otherwise the following dialog can be created disabled.
  await sleep(750);
  return result;
}

function forbiddenFields(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) return [];
  return Object.keys(value).filter((key) => /path|filepath|sourcepath/i.test(key));
}

function validSource(source, expectedFileName = sourceFileName) {
  return Boolean(source)
    && source.format === "csv"
    && source.fileName === expectedFileName
    && Number.isInteger(source.fileSizeBytes)
    && source.fileSizeBytes > 0
    && Array.isArray(source.sheets)
    && source.sheets.length === 0
    && forbiddenFields(source).length === 0;
}

function validDatasetPreview(preview, source) {
  return Boolean(preview)
    && preview.fileName === source.fileName
    && preview.fileSizeBytes === source.fileSizeBytes
    && Number.isInteger(preview.rowCount)
    && preview.rowCount > 0
    && (expectedRowCount === null || preview.rowCount === expectedRowCount)
    && Number.isInteger(preview.columnCount)
    && preview.columnCount === 4
    && Array.isArray(preview.columns)
    && preview.columns.length === preview.columnCount
    && Array.isArray(preview.rows)
    && preview.rows.length > 0
    && forbiddenFields(preview).length === 0;
}

function validRecipe(recipe) {
  return Boolean(recipe)
    && (recipe.version === 1 || recipe.version === 2)
    && recipe.name === "Native selector probe"
    && typeof recipe.savedAt === "string"
    && recipe.recipe
    && Array.isArray(recipe.recipe.renames)
    && recipe.recipe.renames.length === 0
    && forbiddenFields(recipe).length === 0;
}

function validRoundTripSource(source, format, fileName) {
  if (!source
    || source.format !== format
    || source.fileName !== fileName
    || !Number.isInteger(source.fileSizeBytes)
    || source.fileSizeBytes <= 0
    || !Array.isArray(source.sheets)
    || forbiddenFields(source).length > 0) {
    return false;
  }
  if (format === "excel") {
    return source.sheets.length > 0
      && typeof source.defaultSheetId === "string"
      && source.sheets.some((sheet) => sheet.id === source.defaultSheetId);
  }
  return source.sheets.length === 0 && source.defaultSheetId === null;
}

function validRoundTripDataset(dataset, fileName) {
  const expectedRows = [
    ["1", "probe-a"],
    ["2", "probe-b"],
  ];
  return Boolean(dataset)
    && dataset.fileName === fileName
    && dataset.rowCount === 2
    && dataset.columnCount === 2
    && Array.isArray(dataset.columns)
    && dataset.columns.map((column) => column.name).join(",") === "id,value"
    && Array.isArray(dataset.rows)
    && expectedRows.every((expectedRow, rowIndex) => {
      const row = dataset.rows[rowIndex];
      return Array.isArray(row) && expectedRow.every((expectedCell, columnIndex) => {
        const actualCell = row[columnIndex];
        return actualCell === expectedCell
          || (/^\d+$/.test(expectedCell)
            && typeof actualCell === "string"
            && actualCell.replace(/\.0+$/, "") === expectedCell);
      });
    })
    && forbiddenFields(dataset).length === 0;
}

async function exportRoundTripFormat(page, format, targetPath) {
  const exported = await invokeWithNativeDialog(
    page,
    "export_dataset",
    {
      format,
      qualityRules: [],
      allowUnvalidated: true,
      privacyMode: "none",
      onProgress: null,
    },
    "save",
    targetPath,
  );
  if (!exported
    || !Number.isInteger(exported.fileSizeBytes)
    || exported.fileSizeBytes <= 0
    || !existsSync(targetPath)
    || statSync(targetPath).size !== exported.fileSizeBytes
    || forbiddenFields(exported).length > 0) {
    throw new Error(`${format}_export_invalid`);
  }

  const fileName = basename(targetPath);
  let source = await invokeWithNativeDialog(
    page,
    "pick_dataset_source",
    {},
    "open",
    targetPath,
  );
  if (format === "excel" && source) {
    const sheets = await invoke(page, "inspect_workbook_sheets", {
      selectionId: source.selectionId,
    });
    source = {
      ...source,
      sheets: Array.isArray(sheets) ? sheets : [],
      defaultSheetId: source.defaultSheetId ?? sheets?.[0]?.id ?? null,
    };
  }
  if (!validRoundTripSource(source, format, fileName)) {
    throw new Error(`${format}_source_invalid`);
  }

  const dataset = await invoke(page, "load_dataset_selection", {
    selectionId: source.selectionId,
    sheetId: format === "excel" ? source.defaultSheetId : null,
    headerMode: format === "parquet" ? null : "firstRow",
    onProgress: null,
  });
  if (!validRoundTripDataset(dataset, fileName)) {
    throw new Error(`${format}_load_invalid:${JSON.stringify({
      fileName: dataset?.fileName ?? null,
      rowCount: dataset?.rowCount ?? null,
      columnCount: dataset?.columnCount ?? null,
      columns: Array.isArray(dataset?.columns) ? dataset.columns.map((column) => column.name) : null,
      rows: Array.isArray(dataset?.rows) ? dataset.rows.slice(0, 2) : null,
    })}`);
  }

  return {
    format,
    fileName,
    sizeBytes: source.fileSizeBytes,
    sheetCount: source.sheets.length,
    rowCount: dataset.rowCount,
    columnCount: dataset.columnCount,
    schema: dataset.columns.map((column) => column.name),
    valuesVerified: true,
  };
}

async function findPage() {
  const deadline = Date.now() + probeTimeoutMs;
  while (Date.now() < deadline) {
    const pages = browser.contexts().flatMap((context) => context.pages());
    for (const page of pages) {
      try {
        const ready = await page.evaluate(() => Boolean(
          window.__TAURI_INTERNALS__ && document.querySelector("#app-title"),
        ));
        if (ready) return page;
      } catch {
        // The WebView2 target may still be navigating.
      }
    }
    await sleep(250);
  }
  throw new Error("native_selectors_page_timeout");
}

async function runLargeDatasetBenchmark(page, source) {
  const recipe = {
    renames: [{ from: "amount", to: "total" }],
    casts: [],
    dateParses: [],
    filters: [],
    calculatedColumn: null,
    findReplace: null,
    keepColumns: null,
    splitColumn: null,
    mergeColumns: null,
    outlierTreatments: [],
    groupSummary: null,
    contactNormalizations: [],
    textExtractions: [],
  };
  const timedInvoke = async (command, args) => {
    const startedAt = performance.now();
    const result = await invoke(page, command, args);
    return {
      result,
      durationMs: Number((performance.now() - startedAt).toFixed(2)),
    };
  };

  const loaded = await timedInvoke("load_dataset_selection", {
    selectionId: source.selectionId,
    sheetId: null,
    headerMode: null,
    onProgress: null,
  });
  if (!validDatasetPreview(loaded.result, source)) throw new Error("large_dataset_load_invalid");

  const paged = await timedInvoke("get_dataset_page", { offset: 0, limit: 50 });
  if (!paged.result
    || paged.result.offset !== 0
    || !Array.isArray(paged.result.rows)
    || paged.result.rows.length === 0
    || paged.result.rows.length > 50
    || paged.result.rows.some((row) => !Array.isArray(row) || row.length !== loaded.result.columnCount)
    || forbiddenFields(paged.result).length > 0) {
    throw new Error("large_dataset_page_invalid");
  }

  const transformed = await timedInvoke("apply_transform_recipe", { recipe });
  if (!transformed.result
    || transformed.result.changed !== true
    || transformed.result.dataset?.rowCount !== loaded.result.rowCount
    || transformed.result.dataset?.columnCount !== loaded.result.columnCount
    || transformed.result.dataset?.columns?.some((column) => column.name === "amount")
    || !transformed.result.dataset?.columns?.some((column) => column.name === "total")
    || forbiddenFields(transformed.result).length > 0) {
    throw new Error("large_dataset_transform_invalid");
  }

  const exported = await timedInvoke("probe_export_dataset", {
    format: "csv",
    qualityRules: [],
    allowUnvalidated: true,
  });
  if (!exported.result
    || exported.result.format !== "CSV"
    || !Number.isInteger(exported.result.fileSizeBytes)
    || exported.result.fileSizeBytes <= 0
    || forbiddenFields(exported.result).length > 0) {
    throw new Error("large_dataset_export_invalid");
  }

  return {
    requested: true,
    fileName: source.fileName,
    sizeBytes: source.fileSizeBytes,
    rowCount: loaded.result.rowCount,
    columnCount: loaded.result.columnCount,
    loadDurationMs: loaded.durationMs,
    pageDurationMs: paged.durationMs,
    transformDurationMs: transformed.durationMs,
    exportDurationMs: exported.durationMs,
    exportedSizeBytes: exported.result.fileSizeBytes,
    interactions: [
      "pick_dataset_source",
      "load_dataset_selection",
      "get_dataset_page",
      "apply_transform_recipe",
      "probe_export_dataset",
    ],
  };
}

async function selectDatasetFromApp(page, targetPath) {
  const button = page.getByRole("button", { name: "Seleccionar dataset" });
  await button.waitFor({ state: "visible", timeout: probeTimeoutMs });
  await withTimeout(Promise.all([
    requestNativeDialog("open", targetPath),
    button.click(),
  ]), helperTimeoutMs, "native_dialog_timeout");
  await sleep(750);
}

function importDialogFor(page, targetPath) {
  const fileName = basename(targetPath).replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return page.getByRole("dialog", { name: new RegExp(`de ${fileName}$`) });
}

async function prepareReusableTaskInApp(page, taskId) {
  await page.locator("details.reusable-task-panel > summary").click();
  const taskSelect = page.locator("#reusable-task-select");
  await taskSelect.waitFor({ state: "visible", timeout: probeTimeoutMs });
  await taskSelect.locator('option[value="' + taskId + '"]').waitFor({ state: "attached", timeout: probeTimeoutMs });
  await taskSelect.selectOption(taskId);
  await page.getByRole("heading", { name: "Configuración que se reutilizará" })
    .waitFor({ state: "visible", timeout: probeTimeoutMs });
  await page.getByRole("button", { name: "Preparar próxima importación" }).click();
  await page.getByRole("button", { name: "Tarea lista para importar" })
    .waitFor({ state: "visible", timeout: probeTimeoutMs });
}

async function runReusableTaskFlow(page, emptyRecipe) {
  const compatiblePath = join(temporaryDirectory, "native-reusable-compatible.csv");
  const mismatchedPath = join(temporaryDirectory, "native-reusable-mismatched.csv");
  writeFileSync(compatiblePath, "id,value\n1,probe-a\n2,probe-b\n", "utf8");
  writeFileSync(mismatchedPath, "id,value,extra\n1,probe-a,x\n2,probe-b,y\n", "utf8");
  let taskId = null;
  let outcome;
  try {
    const source = await invokeWithNativeDialog(page, "pick_dataset_source", {}, "open", compatiblePath);
    if (!validSource(source, basename(compatiblePath))) throw new Error("reusable_task_profile_source_invalid");
    const dataset = await invoke(page, "load_dataset_selection", {
      selectionId: source.selectionId, sheetId: null, headerMode: "firstRow", onProgress: null,
    });
    if (!validRoundTripDataset(dataset, source.fileName)) throw new Error("reusable_task_profile_dataset_invalid");
    const importProfile = {
      version: 1, format: "csv", headerMode: "firstRow",
      dateConvention: "unresolved", numberConvention: "unresolved",
      schema: dataset.columns.map(({ name, dataType }) => ({ name, dataType })),
    };
    const taskName = "Native reusable task " + Date.now();
    const recipe = {
      version: 2, name: taskName, savedAt: new Date().toISOString(),
      recipe: { ...emptyRecipe, renames: [{ from: "value", to: "label" }] },
      sourceSchema: importProfile.schema,
    };
    const savedTask = await invoke(page, "save_reusable_task", {
      taskId: null,
      task: { version: 1, name: taskName, importProfile, recipe, qualityRules: [], outputFormat: "csv", privacyMode: "none" },
    });
    if (!savedTask || typeof savedTask.id !== "string" || savedTask.name !== taskName) {
      throw new Error("reusable_task_save_invalid");
    }
    taskId = savedTask.id;

    await page.reload({ waitUntil: "domcontentloaded", timeout: probeTimeoutMs });
    await page.locator("#app-title").waitFor({ state: "visible", timeout: probeTimeoutMs });
    await prepareReusableTaskInApp(page, taskId);
    // The unified preflight reports a saved-profile mismatch inside the import
    // dialog after "Revisar esquema", before any dataset is activated.
    await selectDatasetFromApp(page, mismatchedPath);
    const mismatchImport = importDialogFor(page, mismatchedPath);
    await mismatchImport.getByRole("button", { name: "Revisar esquema" }).click({ timeout: probeTimeoutMs });
    const mismatchAlert = mismatchImport.getByRole("alert")
      .filter({ hasText: "El esquema no coincide con el perfil guardado." });
    await mismatchAlert.waitFor({ state: "visible", timeout: probeTimeoutMs });
    const mismatchAlertText = await mismatchAlert.innerText();
    if (!mismatchAlertText.includes("extra")) {
      const error = new Error("reusable_task_mismatch_details_invalid");
      error.diagnostics = [mismatchAlertText.slice(0, 400)];
      throw error;
    }
    await mismatchImport.getByRole("button", { name: "Importar con esquema nuevo" })
      .waitFor({ state: "visible", timeout: probeTimeoutMs });
    await mismatchImport.getByRole("button", { name: "Cancelar", exact: true }).click();
    await mismatchImport.waitFor({ state: "hidden", timeout: probeTimeoutMs });

    await page.reload({ waitUntil: "domcontentloaded", timeout: probeTimeoutMs });
    await page.locator("#app-title").waitFor({ state: "visible", timeout: probeTimeoutMs });
    await prepareReusableTaskInApp(page, taskId);
    await selectDatasetFromApp(page, compatiblePath);
    const compatibleImport = importDialogFor(page, compatiblePath);
    await compatibleImport.getByRole("button", { name: "Revisar esquema" }).click({ timeout: probeTimeoutMs });
    await compatibleImport.getByRole("button", { name: "Cargar archivo" }).click({ timeout: probeTimeoutMs });
    await page.locator(".workspace--prepare").waitFor({ state: "visible", timeout: probeTimeoutMs });
    outcome = {
      taskSavedAndReopened: true,
      mismatchRequiredConfirmation: true,
      mismatchDetailsVerified: true,
      compatibleTaskAutoApplied: true,
      compatibleImportFile: basename(compatiblePath),
      recipeDraftLoaded: true,
    };
  } finally {
    if (taskId) await invoke(page, "delete_reusable_task", { taskId });
  }
  return { ...outcome, syntheticTaskRemoved: true };
}

async function run() {
  temporaryDirectory = mkdtempSync(join(tmpdir(), "columnia-native-selectors-"));
  const recipePath = join(temporaryDirectory, "native-selector-probe.json");
  const exportPath = join(temporaryDirectory, "native-selector-probe.csv");
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 5_000 });
  const page = await findPage();

  const source = await invokeWithNativeDialog(page, "pick_dataset_source", {}, "open", sourcePath);
  if (!validSource(source)) throw new Error("dataset_picker_invalid");

  if (requestedDatasetPath) {
    if (extname(sourcePath).toLowerCase() !== ".csv") throw new Error("large_dataset_format_invalid");
    const benchmark = await runLargeDatasetBenchmark(page, source);
    return {
      status: "passed",
      phase: "native_large_dataset",
      dialogs: ["open_dataset"],
      sourcePickerVerified: true,
      outputsVerified: true,
      forbiddenPathFields: false,
      datasetBenchmark: benchmark,
    };
  }

  await invoke(page, "discard_dataset_selection", { selectionId: source.selectionId });

  const emptyRecipe = {
    renames: [],
    casts: [],
    dateParses: [],
    filters: [],
    calculatedColumn: null,
    findReplace: null,
    keepColumns: null,
    splitColumn: null,
    mergeColumns: null,
    outlierTreatments: [],
    groupSummary: null,
    contactNormalizations: [],
    textExtractions: [],
  };
  const savedRecipe = await invokeWithNativeDialog(
    page,
    "save_transform_recipe",
    {
      recipe: emptyRecipe,
      name: "Native selector probe",
      sourceSchema: [
        { name: "id", dataType: "Int64" },
        { name: "value", dataType: "String" },
      ],
    },
    "save",
    recipePath,
  );
  if (!validRecipe(savedRecipe) || !existsSync(recipePath)) throw new Error("recipe_save_invalid");

  const pickedRecipe = await invokeWithNativeDialog(
    page,
    "pick_transform_recipe",
    {},
    "open",
    recipePath,
  );
  if (!validRecipe(pickedRecipe)) throw new Error("recipe_picker_invalid");

  const seeded = await invoke(page, "probe_seed_dataset");
  if (!seeded || seeded.fileName !== "native-probe.csv" || forbiddenFields(seeded).length > 0) {
    throw new Error("dataset_seed_invalid");
  }
  const exported = await invokeWithNativeDialog(
    page,
    "export_dataset",
    { format: "csv", qualityRules: [], allowUnvalidated: true, privacyMode: "none", onProgress: null },
    "save",
    exportPath,
  );
  if (
    !exported
    || exported.format !== "CSV"
    || typeof exported.fileName !== "string"
    || !exported.fileName.endsWith(".csv")
    || !Number.isInteger(exported.fileSizeBytes)
    || exported.fileSizeBytes <= 0
    || !existsSync(exportPath)
    || statSync(exportPath).size <= 0
    || forbiddenFields(exported).length > 0
  ) {
    throw new Error("export_selector_invalid");
  }

  const roundTrips = [];
  roundTrips.push(await exportRoundTripFormat(
    page,
    "csv",
    join(temporaryDirectory, "native-round-trip.csv"),
  ));
  roundTrips.push(await exportRoundTripFormat(
    page,
    "excel",
    join(temporaryDirectory, "native-round-trip.xlsx"),
  ));
  roundTrips.push(await exportRoundTripFormat(
    page,
    "parquet",
    join(temporaryDirectory, "native-round-trip.parquet"),
  ));

  const reusableTaskFlow = await runReusableTaskFlow(page, emptyRecipe);

  return {
    status: "passed",
    phase: "native_file_selectors",
    dialogs: ["open_dataset", "save_recipe", "open_recipe", "save_export"],
    sourcePickerVerified: true,
    recipeSaveVerified: true,
    recipePickerVerified: true,
    exportPickerVerified: true,
    outputsVerified: true,
    realImportRoundTrips: roundTrips,
    reusableTaskFlow,
    csvBytesLoaded: true,
    excelAndParquetBytesLoaded: true,
    forbiddenPathFields: false,
    interactions: [
      "pick_dataset_source",
      "discard_dataset_selection",
      "save_transform_recipe",
      "pick_transform_recipe",
      "probe_seed_dataset",
      "export_dataset:csv",
      "pick_dataset_source:csv",
      "load_dataset_selection:csv",
      "export_dataset:excel",
      "pick_dataset_source:excel",
      "inspect_workbook_sheets:excel",
      "load_dataset_selection:excel",
      "export_dataset:parquet",
      "pick_dataset_source:parquet",
      "load_dataset_selection:parquet",
      "save_reusable_task",
      "reusable_task_schema_mismatch_requires_confirmation",
      "reusable_task_compatible_import_auto_applies",
      "delete_reusable_task",
    ],
  };
}

try {
  const result = await run();
  console.log(JSON.stringify(result));
  process.exitCode = 0;
} catch (error) {
  console.log(JSON.stringify({
    status: "failed",
    phase: "native_file_selectors_failed",
    errorCode: error instanceof Error ? error.message : "native_selectors_failed",
    diagnostics: error instanceof Error && Array.isArray(error.diagnostics) ? error.diagnostics : [],
    interactions: [],
  }));
  process.exitCode = 1;
} finally {
  if (browser) await browser.close().catch(() => {});
  if (temporaryDirectory) rmSync(temporaryDirectory, { recursive: true, force: true });
}
