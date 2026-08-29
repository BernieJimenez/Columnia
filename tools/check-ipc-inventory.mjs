import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const inventoryPath = resolve(projectRoot, "docs/reference/ipc-inventory.json");
const rustPath = resolve(projectRoot, "src-tauri/src/lib.rs");
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
  ["DataprepSessionMigrationPlan", "DataprepSessionMigrationPlan"],
  ["QualityRulesDocument", "QualityRulesDocument"],
  ["DatasetColumn", "DatasetColumn"],
  ["DatasetPreview", "DatasetPreview"],
  ["WorkbookSheet", "WorkbookSheet"],
  ["DatasetSourceInspection", "DatasetSourceInspection"],
  ["DatasetPage", "DatasetPage"],
  ["DatasetQueryResult", "DatasetQueryResult"],
  ["NumericCorrelation", "NumericCorrelation"],
  ["NumericCorrelationMatrix", "NumericCorrelationMatrix"],
  ["CategoricalGroup", "CategoricalGroup"],
  ["CategoricalGroupSummary", "CategoricalGroupSummary"],
  ["ColumnProfile", "ColumnProfile"],
  ["DatasetProfile", "DatasetProfile"],
  ["DatasetMutation", "DatasetMutation"],
  ["ColumnRename", "ColumnRename"],
  ["ColumnNormalizationResult", "ColumnNormalizationResult"],
  ["ChangedTextColumn", "ChangedTextColumn"],
  ["TextCleaningResult", "TextCleaningResult"],
  ["HistoryResult", "HistoryResult"],
  ["HistoryEntryState", "HistoryEntryState"],
  ["HistoryState", "HistoryState"],
  ["SafeCorrectionsResult", "SafeCorrectionsResult"],
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
  ["ProjectSummary", "ProjectSummary"],
  ["ProjectOpenResult", "ProjectOpenResult"],
  ["SqlQueryHistoryEntry", "SqlQueryHistoryEntry"],
  ["ProjectWorkspace", "ProjectWorkspace"],
];

function handlerEntries(source) {
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
  return source.slice(opening + 1, closing).split(",").map((raw) => {
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
    sourceFiles: ["src-tauri/src/lib.rs", "src-tauri/src/dataset.rs", "src-tauri/src/projects.rs", "src-tauri/src/resource.rs", "src-tauri/src/updater.rs", "src/bridge.ts"],
    productionCommands: entries.filter(({ name, debug }) => !debug && !debugOnly.has(name)),
    debugCommands: entries.filter(({ name, debug }) => debug || debugOnly.has(name)),
    sharedStructures: sharedStructures.map(([rust, typescript]) => ({ rust, typescript })),
  };
}

function comparable(value) {
  return JSON.stringify(value);
}

try {
  const expected = expectedInventory(await readFile(rustPath, "utf8"));
  if (process.argv.includes("--write")) {
    await writeFile(inventoryPath, `${JSON.stringify(expected, null, 2)}\n`, "utf8");
    console.log(`Inventario IPC generado: ${inventoryPath}`);
    process.exit(0);
  }
  const current = JSON.parse(await readFile(inventoryPath, "utf8"));
  if (comparable(current.productionCommands) !== comparable(expected.productionCommands)
    || comparable(current.debugCommands) !== comparable(expected.debugCommands)) {
    throw new Error("El inventario IPC no coincide con generate_handler; regénéralo y clasifica cualquier novedad.");
  }
  if (comparable(current.sharedStructures) !== comparable(expected.sharedStructures)) {
    throw new Error("La lista de estructuras compartidas IPC cambió; actualiza el inventario y sus contratos.");
  }
  if (current.productionCommands.length !== 60 || current.debugCommands.length !== 4) {
    throw new Error(`Conteo IPC inesperado: ${current.productionCommands.length} producción, ${current.debugCommands.length} debug.`);
  }
  console.log(`Inventario IPC aprobado: ${current.productionCommands.length} comandos producción, ${current.debugCommands.length} debug, ${current.sharedStructures.length} estructuras.`);
} catch (error) {
  console.error(`Gate de inventario IPC falló: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
}
