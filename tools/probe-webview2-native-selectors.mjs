import { chromium } from "@playwright/test";
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const portArgumentIndex = process.argv.indexOf("--port");
const port = portArgumentIndex >= 0 ? Number(process.argv[portArgumentIndex + 1]) : 9222;
const requestFileArgumentIndex = process.argv.indexOf("--request-file");
const requestFile = requestFileArgumentIndex >= 0 ? resolve(process.argv[requestFileArgumentIndex + 1]) : null;
const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sourcePath = resolve(projectRoot, "fixtures", "automation", "input.csv");
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
          throw new Error(`${mode}_${response.errorCode ?? "native_dialog_driver_failed"}`);
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
    if (currentCommand === "export_dataset" && invokeArgs.onProgress === null) {
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
  return result;
}

function forbiddenFields(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) return [];
  return Object.keys(value).filter((key) => /path|filepath|sourcepath/i.test(key));
}

function validSource(source) {
  return Boolean(source)
    && source.format === "csv"
    && source.fileName === "input.csv"
    && Number.isInteger(source.fileSizeBytes)
    && source.fileSizeBytes > 0
    && Array.isArray(source.sheets)
    && source.sheets.length === 0
    && forbiddenFields(source).length === 0;
}

function validRecipe(recipe) {
  return Boolean(recipe)
    && recipe.version === 1
    && recipe.name === "Native selector probe"
    && typeof recipe.savedAt === "string"
    && recipe.recipe
    && Array.isArray(recipe.recipe.renames)
    && recipe.recipe.renames.length === 0
    && forbiddenFields(recipe).length === 0;
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

async function run() {
  temporaryDirectory = mkdtempSync(join(tmpdir(), "columnia-native-selectors-"));
  const recipePath = join(temporaryDirectory, "native-selector-probe.json");
  const exportPath = join(temporaryDirectory, "native-selector-probe.csv");
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 5_000 });
  const page = await findPage();

  const source = await invokeWithNativeDialog(page, "pick_dataset_source", {}, "open", sourcePath);
  if (!validSource(source)) throw new Error("dataset_picker_invalid");
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
    { recipe: emptyRecipe, name: "Native selector probe" },
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

  return {
    status: "passed",
    phase: "native_file_selectors",
    dialogs: ["open_dataset", "save_recipe", "open_recipe", "save_export"],
    sourcePickerVerified: true,
    recipeSaveVerified: true,
    recipePickerVerified: true,
    exportPickerVerified: true,
    outputsVerified: true,
    forbiddenPathFields: false,
    interactions: [
      "pick_dataset_source",
      "discard_dataset_selection",
      "save_transform_recipe",
      "pick_transform_recipe",
      "probe_seed_dataset",
      "export_dataset",
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
    interactions: [],
  }));
  process.exitCode = 1;
} finally {
  if (browser) await browser.close().catch(() => {});
  if (temporaryDirectory) rmSync(temporaryDirectory, { recursive: true, force: true });
}
