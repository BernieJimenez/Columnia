import { access, readdir, readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const inventoryPath = resolve(projectRoot, "docs/reference/ipc-inventory.json");
const rustPath = resolve(projectRoot, "src-tauri/src/lib.rs");
const sourceFiles = [
  "src-tauri/src/lib.rs",
  "src-tauri/src/dataset.rs",
  "src-tauri/src/dataset/comparison_io.rs",
  "src-tauri/src/dataset/explore.rs",
  "src-tauri/src/dataset/export_io.rs",
  "src-tauri/src/dataset/history.rs",
  "src-tauri/src/dataset/json_reader.rs",
  "src-tauri/src/dataset/operation_cancellation.rs",
  "src-tauri/src/dataset/operation_state.rs",
  "src-tauri/src/dataset/quality_contracts.rs",
  "src-tauri/src/dataset/recipe_documents.rs",
  "src-tauri/src/dataset/source_loading.rs",
  "src-tauri/src/dataset/spreadsheet_io.rs",
  "src-tauri/src/diagnostics.rs",
  "src-tauri/src/dataset/samples.rs",
  "src-tauri/src/projects.rs",
  "src-tauri/src/resource.rs",
  "src-tauri/src/reusable_tasks.rs",
  "src-tauri/src/updater.rs",
  "src-tauri/src/remote_databases.rs",
  "src-tauri/src/delivery_presets.rs",
  "src-tauri/src/session_guard.rs",
  "src/bridge.ts",
  "src/bridge/client.ts",
  "src/bridge/contracts.ts",
  "src/bridge/dataset-contracts.ts",
  "src/bridge/datasets.ts",
  "src/bridge/delivery-contracts.ts",
  "src/bridge/delivery.ts",
  "src/bridge/prepare.ts",
  "src/bridge/progress.ts",
  "src/bridge/project-contracts.ts",
  "src/bridge/reusable-task-contracts.ts",
  "src/bridge/diagnostics-contracts.ts",
  "src/bridge/projects.ts",
  "src/bridge/reusable-tasks.ts",
  "src/bridge/diagnostics.ts",
  "src/bridge/recipe-contracts.ts",
  "src/bridge/system-contracts.ts",
  "src/bridge/system.ts",
];
const debugOnly = new Set([
  "probe_seed_dataset",
  "probe_save_transform_recipe",
  "probe_export_dataset",
  "probe_reopen_project",
]);

const sharedStructures = [
  ["AppInfo", "AppInfo"],
  ["UpdateInfo", "UpdateInfo"],
  ["UpdaterProgress", "UpdaterProgress"],
  ["ResourceUsage", "ResourceUsage"],
  ["PerformanceSettings", "PerformanceSettings"],
  ["OperationProgress", "OperationProgress"],
  ["ExportResult", "ExportResult"],
  ["QualityCondition", "QualityCondition"],
  ["QualityRule", "QualityRule"],
  ["QualityRuleResult", "QualityRuleResult"],
  ["QualityValidationResult", "QualityValidationResult"],
  ["QualityMigrationWarning", "QualityMigrationWarning"],
  ["QualityMigrationResult", "QualityMigrationResult"],
  ["QualityRulesDocument", "QualityRulesDocument"],
  ["DatasetColumn", "DatasetColumn"],
  ["DatasetPreview", "DatasetPreview"],
  ["DatasetImportSchemaPreview", "DatasetImportSchemaPreview"],
  ["WorkbookSheet", "WorkbookSheet"],
  ["DatasetResourceEstimate", "DatasetResourceEstimate"],
  ["DatasetSourceInspection", "DatasetSourceInspection"],
  ["SampleDatasetDescriptor", "SampleDatasetDescriptor"],
  ["DatasetPage", "DatasetPage"],
  ["DatasetQueryResult", "DatasetQueryResult"],
  ["NumericCorrelation", "NumericCorrelation"],
  ["NumericCorrelationMatrix", "NumericCorrelationMatrix"],
  ["CategoricalGroup", "CategoricalGroup"],
  ["CategoricalGroupSummary", "CategoricalGroupSummary"],
  ["ColumnProfile", "ColumnProfile"],
  ["DatasetProfile", "DatasetProfile"],
  ["TemporalAggregationPeriod", "TemporalAggregationPeriod"],
  ["TemporalAggregationSeries", "TemporalAggregationSeries"],
  ["DatasetMutation", "DatasetMutation"],
  ["ColumnRename", "ColumnRename"],
  ["ColumnNormalizationResult", "ColumnNormalizationResult"],
  ["ChangedTextColumn", "ChangedTextColumn"],
  ["TextCleaningResult", "TextCleaningResult"],
  ["PersonalDataMaskResult", "PersonalDataMaskResult"],
  ["HistoryResult", "HistoryResult"],
  ["HistoryEntryState", "HistoryEntryState"],
  ["HistoryState", "HistoryState"],
  ["SafeCorrectionsResult", "SafeCorrectionsResult"],
  ["ImputationPreview", "ImputationPreview"],
  ["SafeCorrectionsPreview", "SafeCorrectionsPreview"],
  ["DateColumnPlan", "DateColumnPlan"],
  ["RecipeSourceColumn", "DatasetColumn"],
  ["RecipeRename", "RecipeRename"],
  ["RecipeCast", "RecipeCast"],
  ["RecipeDateParse", "RecipeDateParse"],
  ["RecipeFilter", "RecipeFilter"],
  ["CalculatedOperand", "CalculatedOperand"],
  ["CalculatedColumnRecipe", "CalculatedColumnRecipe"],
  ["FindReplaceRecipe", "FindReplaceRecipe"],
  ["SplitColumnRecipe", "SplitColumnRecipe"],
  ["MergeColumnsRecipe", "MergeColumnsRecipe"],
  ["OutlierTreatment", "OutlierTreatment"],
  ["SummaryAggregation", "SummaryAggregation"],
  ["GroupSummaryRecipe", "GroupSummaryRecipe"],
  ["ContactNormalization", "ContactNormalization"],
  ["TextExtraction", "TextExtraction"],
  ["TransformRecipe", "TransformRecipe"],
  ["StoredTransformRecipe", "SavedRecipe"],
  ["TransformRecipeResult", "TransformRecipeResult"],
  ["ReusableTask", "ReusableTask"],
  ["ReusableTaskSummary", "ReusableTaskSummary"],
  ["ReusableTaskSchemaCompatibility", "ReusableTaskSchemaCompatibility"],
  ["ProjectSummary", "ProjectSummary"],
  ["ProjectCatalogSnapshot", "ProjectCatalogSnapshot"],
  ["ProjectVersionSummary", "ProjectVersionSummary"],
  ["ProjectOpenResult", "ProjectOpenResult"],
  ["SqlQueryHistoryEntry", "SqlQueryHistoryEntry"],
  ["ProjectWorkspace", "ProjectWorkspace"],
  ["DiagnosticReport", "DiagnosticReport"],
  ["DiagnosticMetrics", "DiagnosticMetrics"],
  ["SessionStatus", "SessionStatus"],
  ["ExploreFilter", "ExploreFilter"],
  ["ExploreRange", "ExploreRange"],
  ["ExplorePanel", "ExplorePanel"],
  ["ExploreKpi", "ExploreKpi"],
  ["ExploreCategoryChart", "ExploreCategoryChart"],
  ["ExploreBar", "ExploreBar"],
  ["ExploreHistogram", "ExploreHistogram"],
  ["ExploreBin", "ExploreBin"],
  ["ExploreTrend", "ExploreTrend"],
  ["ExplorePoint", "ExplorePoint"],
];

/** Commands the TypeScript bridge invokes: `invoke("name")` or `invoke<T>("name")`. */
export function invokedCommands(sources) {
  const names = new Set();
  for (const contents of sources) {
    for (const match of contents.matchAll(/\binvoke(?:<[^>]*>)?\(\s*["'`]([a-z0-9_]+)["'`]/g)) {
      names.add(match[1]);
    }
  }
  return names;
}

/** QA-25: a command registered in Rust without a TS caller, or the reverse. */
export function commandParityProblems(productionNames, invoked) {
  const production = new Set(productionNames);
  const problems = [];
  for (const name of production) {
    if (!invoked.has(name)) problems.push(`El comando ${name} está registrado en Rust pero ningún invoke de TypeScript lo usa.`);
  }
  for (const name of invoked) {
    if (!production.has(name)) problems.push(`TypeScript invoca ${name}, que no es un comando de producción registrado.`);
  }
  return problems;
}

async function bridgeSources(directory) {
  const sources = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      sources.push(...await bridgeSources(path));
    } else if (/\.(ts|tsx)$/.test(entry.name) && !/\.test\.(ts|tsx)$/.test(entry.name)) {
      sources.push(await readFile(path, "utf8"));
    }
  }
  return sources;
}

export function handlerEntries(source) {
  const marker = source.indexOf("tauri::generate_handler!");
  const opening = source.indexOf("[", marker);
  if (marker < 0 || opening < 0) throw new Error("No se encontró generate_handler en lib.rs.");
  let depth = 0;
  let closing = -1;
  for (let index = opening; index < source.length; index += 1) {
    if (source[index] === "[") depth += 1;
    if (source[index] === "]") depth -= 1;
    if (depth === 0) {
      closing = index;
      break;
    }
  }
  if (closing < 0) throw new Error("La lista generate_handler está incompleta.");
  // Comments inside the list (`// …`) are not commands.
  const list = source.slice(opening + 1, closing).replace(/\/\/[^\n]*/g, "");
  return list.split(",").map((raw) => {
    const debug = raw.includes("cfg(debug_assertions)");
    const entry = raw.replace(/#\[[^\]]+\]\s*/g, "").trim();
    const parts = entry.split("::");
    return { name: parts.at(-1), module: parts.length > 1 ? parts.at(-2) : "lib", debug };
  }).filter(({ name }) => name);
}

function expectedInventory(source) {
  const entries = handlerEntries(source);
  return {
    schemaVersion: 1,
    sourceFiles,
    productionCommands: entries.filter(({ name, debug }) => !debug && !debugOnly.has(name)),
    debugCommands: entries.filter(({ name, debug }) => debug || debugOnly.has(name)),
    sharedStructures: sharedStructures.map(([rust, typescript]) => ({ rust, typescript })),
  };
}

function comparable(value) {
  return JSON.stringify(value);
}

async function main() {
  const expected = expectedInventory(await readFile(rustPath, "utf8"));
  if (process.argv.includes("--write")) {
    await writeFile(inventoryPath, `${JSON.stringify(expected, null, 2)}\n`, "utf8");
    console.log(`Inventario IPC generado: ${inventoryPath}`);
    process.exit(0);
  }
  const current = JSON.parse(await readFile(inventoryPath, "utf8"));
  if (comparable(current.sourceFiles) !== comparable(expected.sourceFiles)) {
    throw new Error("Los archivos fuente del inventario IPC no coinciden con los módulos de paridad.");
  }
  for (const file of expected.sourceFiles) {
    try {
      await access(resolve(projectRoot, file));
    } catch {
      throw new Error(`Falta un archivo fuente declarado por el inventario IPC: ${file}`);
    }
  }
  if (comparable(current.productionCommands) !== comparable(expected.productionCommands)
    || comparable(current.debugCommands) !== comparable(expected.debugCommands)) {
    throw new Error("El inventario IPC no coincide con generate_handler; regénéralo y clasifica cualquier novedad.");
  }
  if (comparable(current.sharedStructures) !== comparable(expected.sharedStructures)) {
    throw new Error("La lista de estructuras compartidas IPC cambió; actualiza el inventario y sus contratos.");
  }
  const parity = commandParityProblems(
    current.productionCommands.map(({ name }) => name),
    invokedCommands(await bridgeSources(resolve(projectRoot, "src"))),
  );
  if (parity.length > 0) throw new Error(parity.join(" "));
  console.log(`Inventario IPC aprobado: ${current.productionCommands.length} comandos producción con su invoke de TypeScript, ${current.debugCommands.length} debug, ${current.sharedStructures.length} estructuras.`);
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  try {
    await main();
  } catch (error) {
    console.error(`Gate de inventario IPC falló: ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
  }
}
