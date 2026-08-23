import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const debugOnlyCommands = new Set([
  "probe_seed_dataset",
  "probe_save_transform_recipe",
  "probe_export_dataset",
  "probe_reopen_project",
]);

function registeredTauriCommands(source: string): string[] {
  const marker = source.indexOf("tauri::generate_handler!");
  const openingIndex = marker < 0 ? -1 : source.indexOf("[", marker);
  let closingIndex = -1;
  if (openingIndex >= 0) {
    let depth = 0;
    for (let index = openingIndex; index < source.length; index += 1) {
      if (source[index] === "[") depth += 1;
      if (source[index] === "]") depth -= 1;
      if (depth === 0) {
        closingIndex = index;
        break;
      }
    }
  }
  const handler = openingIndex >= 0 && closingIndex > openingIndex
    ? source.slice(openingIndex + 1, closingIndex)
    : undefined;

  if (!handler) {
    throw new Error("No se encontró tauri::generate_handler! en src-tauri/src/lib.rs.");
  }

  return handler
    .split(",")
    .map((entry) => entry.replace(/#\[[^\]]+\]\s*/g, "").trim().split("::").at(-1) ?? "")
    .filter((command) => command && !debugOnlyCommands.has(command));
}

function invokedBridgeCommands(source: string): string[] {
  return [...source.matchAll(/\binvoke(?:<[^>]+>)?\(\s*"([^"]+)"/g)].map(
    (match) => match[1],
  );
}

function splitTopLevel(value: string, delimiter = ","): string[] {
  const entries: string[] = [];
  let start = 0;
  let angleDepth = 0;
  let parenthesisDepth = 0;
  let bracketDepth = 0;
  let braceDepth = 0;

  for (let index = 0; index < value.length; index += 1) {
    const character = value[index];
    if (character === "<") angleDepth += 1;
    if (character === ">") angleDepth -= 1;
    if (character === "(") parenthesisDepth += 1;
    if (character === ")") parenthesisDepth -= 1;
    if (character === "[") bracketDepth += 1;
    if (character === "]") bracketDepth -= 1;
    if (character === "{") braceDepth += 1;
    if (character === "}") braceDepth -= 1;

    if (
      character === delimiter &&
      angleDepth === 0 &&
      parenthesisDepth === 0 &&
      bracketDepth === 0 &&
      braceDepth === 0
    ) {
      entries.push(value.slice(start, index).trim());
      start = index + 1;
    }
  }

  const last = value.slice(start).trim();
  if (last) entries.push(last);
  return entries;
}

function balancedBraces(source: string, openingIndex: number): string {
  let depth = 0;

  for (let index = openingIndex; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") depth -= 1;
    if (depth === 0) return source.slice(openingIndex + 1, index);
  }

  throw new Error("Se encontró una estructura con llaves incompletas.");
}

function balancedParentheses(source: string, openingIndex: number): string {
  let depth = 0;

  for (let index = openingIndex; index < source.length; index += 1) {
    if (source[index] === "(") depth += 1;
    if (source[index] === ")") depth -= 1;
    if (depth === 0) return source.slice(openingIndex + 1, index);
  }

  throw new Error("Se encontró una firma Rust con paréntesis incompletos.");
}

function camelCase(value: string): string {
  return value.replace(/_([a-z])/g, (_, letter: string) => letter.toUpperCase());
}

function rustCommandArguments(
  source: string,
  commands: string[],
): Record<string, string[]> {
  return Object.fromEntries(
    commands.map((command) => {
      const signature = new RegExp(`(?:pub\\s+)?(?:async\\s+)?fn\\s+${command}\\s*\\(`).exec(
        source,
      );
      if (!signature) throw new Error(`No se encontró la firma Rust de ${command}.`);

      const openingIndex = signature.index + signature[0].lastIndexOf("(");
      const argumentsSource = balancedParentheses(source, openingIndex);
      const argumentsList = splitTopLevel(argumentsSource)
        .map((argument) => {
          const separator = argument.indexOf(":");
          if (separator === -1) return null;
          return {
            name: argument.slice(0, separator).trim(),
            type: argument.slice(separator + 1).trim(),
          };
        })
        .filter((argument): argument is { name: string; type: string } => argument !== null)
        .filter(({ type }) => !type.includes("AppHandle") && !type.startsWith("State<"))
        .map(({ name }) => camelCase(name))
        .sort();

      return [command, argumentsList];
    }),
  );
}

function bridgeCommandArguments(source: string): Record<string, string[]> {
  const calls = source.matchAll(
    /\binvoke(?:<[^>]+>)?\(\s*"([^"]+)"(?:\s*,\s*\{([\s\S]*?)\})?\s*\)/g,
  );

  return Object.fromEntries(
    [...calls].map((match) => {
      const argumentsList = splitTopLevel(match[2] ?? "")
        .map((property) => property.split(":", 1)[0].trim())
        .filter(Boolean)
        .sort();
      return [match[1], argumentsList];
    }),
  );
}

function rustCommandReturnTypes(
  source: string,
  commands: string[],
): Record<string, string> {
  return Object.fromEntries(
    commands.map((command) => {
      const signature = new RegExp(`(?:pub\\s+)?(?:async\\s+)?fn\\s+${command}\\s*\\(`).exec(
        source,
      );
      if (!signature) throw new Error(`No se encontró la firma Rust de ${command}.`);

      const openingIndex = signature.index + signature[0].lastIndexOf("(");
      balancedParentheses(source, openingIndex);
      let depth = 0;
      let closingIndex = -1;
      for (let index = openingIndex; index < source.length; index += 1) {
        if (source[index] === "(") depth += 1;
        if (source[index] === ")") depth -= 1;
        if (depth === 0) {
          closingIndex = index;
          break;
        }
      }

      const returnMatch = source.slice(closingIndex + 1).match(/^\s*->\s*([^\{]+)\{/);
      if (!returnMatch) throw new Error(`No se encontró el retorno Rust de ${command}.`);
      return [command, normalizeRustReturnType(returnMatch[1].trim())];
    }),
  );
}

function genericContents(type: string, generic: string): string | null {
  const prefix = `${generic}<`;
  return type.startsWith(prefix) && type.endsWith(">")
    ? type.slice(prefix.length, -1).trim()
    : null;
}

function normalizeRustReturnType(type: string): string {
  const compact = type.replace(/\s+/g, " ").trim();
  const result = genericContents(compact, "Result");
  if (result !== null) return normalizeRustReturnType(splitTopLevel(result)[0]);

  const option = genericContents(compact, "Option");
  if (option !== null) return `${normalizeRustReturnType(option)} | null`;

  const vector = genericContents(compact, "Vec");
  if (vector !== null) return `${normalizeRustReturnType(vector)}[]`;

  if (compact === "()") return "void";
  if (compact === "StoredTransformRecipe") return "SavedRecipe";
  return compact;
}

function bridgeCommandReturnTypes(source: string): Record<string, string> {
  const calls = source.matchAll(/\binvoke<([^>]+)>\(\s*"([^"]+)"/g);

  return Object.fromEntries(
    [...calls].map((match) => {
      const normalized = match[1]
        .replace(/\bLoadedRecipe\b/g, "SavedRecipe")
        .split("|")
        .map((part) => part.trim())
        .join(" | ");
      return [match[2], normalized];
    }),
  );
}

function rustStructFields(source: string, structName: string): string[] {
  const declaration = new RegExp(`(?:pub\\s+)?struct\\s+${structName}\\s*\\{`).exec(source);
  if (!declaration) throw new Error(`No se encontró la estructura Rust ${structName}.`);

  const openingIndex = declaration.index + declaration[0].lastIndexOf("{");
  const body = balancedBraces(source, openingIndex)
    .replace(/#\[[^\]]*\]\s*/g, "")
    .replace(/\/\/.*$/gm, "");

  return splitTopLevel(body)
    .map((field) => field.match(/^(?:pub(?:\([^)]*\))?\s+)?([a-z][a-z0-9_]*)\s*:/)?.[1])
    .filter((field): field is string => Boolean(field))
    .map(camelCase)
    .sort();
}

function rustStructFieldTypes(
  source: string,
  structName: string,
): Record<string, string> {
  const declaration = new RegExp(`(?:pub\\s+)?struct\\s+${structName}\\s*\\{`).exec(source);
  if (!declaration) throw new Error(`No se encontró la estructura Rust ${structName}.`);

  const openingIndex = declaration.index + declaration[0].lastIndexOf("{");
  const body = balancedBraces(source, openingIndex)
    .replace(/#\[[^\]]*\]\s*/g, "")
    .replace(/\/\/.*$/gm, "");

  return Object.fromEntries(
    splitTopLevel(body).flatMap((field) => {
      const match = field.match(
        /^(?:pub(?:\([^)]*\))?\s+)?([a-z][a-z0-9_]*)\s*:\s*([\s\S]+)$/,
      );
      return match ? [[camelCase(match[1]), normalizeRustFieldType(match[2])]] : [];
    }),
  );
}

function normalizeRustFieldType(type: string): string {
  const compact = type.replace(/\s+/g, " ").trim();
  const withoutReference = compact.replace(/^&(?:'\w+\s+)?/, "");
  const option = genericContents(withoutReference, "Option");
  if (option !== null) return `optional<${normalizeRustFieldType(option)}>`;

  const vector = genericContents(withoutReference, "Vec");
  if (vector !== null) return `array<${normalizeRustFieldType(vector)}>`;

  if (withoutReference === "String" || withoutReference === "str") return "string";
  if (withoutReference === "bool") return "boolean";
  if (/^(?:[iu](?:8|16|32|64|128|size)|f(?:32|64))$/.test(withoutReference)) {
    return "number";
  }

  if (withoutReference === "StoredTransformRecipe") return "SavedRecipe";
  if (
    [
      "QualityRuleKind",
      "RecipeCastTarget",
      "RecipeDateFormat",
      "RecipeDateTarget",
      "RecipeFilterOperator",
      "CalculatedOperation",
      "CalculatedOperandKind",
      "FindReplaceScope",
      "OutlierAction",
      "SummaryOperation",
      "ContactKind",
      "ExtractionKind",
    ].includes(withoutReference)
  ) return "string";
  return withoutReference;
}

function typescriptInterfaceFields(
  source: string,
  interfaceName: string,
  visited = new Set<string>(),
): string[] {
  if (visited.has(interfaceName)) return [];
  visited.add(interfaceName);

  const declaration = new RegExp(
    `export\\s+interface\\s+${interfaceName}(?:\\s+extends\\s+([^\\{]+))?\\s*\\{`,
  ).exec(source);
  if (!declaration) throw new Error(`No se encontró la interfaz TypeScript ${interfaceName}.`);

  const openingIndex = declaration.index + declaration[0].lastIndexOf("{");
  const ownFields = splitTopLevel(balancedBraces(source, openingIndex), ";")
    .map((field) => field.trim().match(/^([A-Za-z][A-Za-z0-9_]*)\??\s*:/)?.[1])
    .filter((field): field is string => Boolean(field));
  const inheritedFields = (declaration[1] ?? "")
    .split(",")
    .map((parent) => parent.trim())
    .filter(Boolean)
    .flatMap((parent) => typescriptInterfaceFields(source, parent, visited));

  return [...new Set([...ownFields, ...inheritedFields])].sort();
}

function typescriptTypeAliases(source: string): Record<string, string> {
  return Object.fromEntries(
    [...source.matchAll(/export\s+type\s+(\w+)\s*=\s*([\s\S]*?);/g)].map((match) => [
      match[1],
      match[2].trim(),
    ]),
  );
}

function normalizeTypescriptFieldType(
  type: string,
  aliases: Record<string, string>,
  visited = new Set<string>(),
): string {
  const compact = type.replace(/\s+/g, " ").trim();
  if (aliases[compact] && !visited.has(compact)) {
    return normalizeTypescriptFieldType(
      aliases[compact],
      aliases,
      new Set([...visited, compact]),
    );
  }

  if (compact.endsWith("[]")) {
    return `array<${normalizeTypescriptFieldType(compact.slice(0, -2), aliases, visited)}>`;
  }

  const array = genericContents(compact, "Array");
  if (array !== null) return `array<${normalizeTypescriptFieldType(array, aliases, visited)}>`;

  const union = splitTopLevel(compact, "|").filter(Boolean);
  if (union.length > 1) {
    const nullable = union.some((part) => part === "null" || part === "undefined");
    const members = union.filter((part) => part !== "null" && part !== "undefined");
    const normalizedMembers = [
      ...new Set(members.map((member) => normalizeTypescriptFieldType(member, aliases, visited))),
    ].sort();
    const normalized = normalizedMembers.length === 1
      ? normalizedMembers[0]
      : normalizedMembers.join(" | ");
    return nullable ? `optional<${normalized}>` : normalized;
  }

  if (/^(["']).*\1$/.test(compact)) return "string";
  if (/^-?(?:\d+(?:\.\d+)?|\.\d+)$/.test(compact)) return "number";
  return compact;
}

function typescriptInterfaceFieldTypes(
  source: string,
  interfaceName: string,
  aliases: Record<string, string>,
  visited = new Set<string>(),
): Record<string, string> {
  if (visited.has(interfaceName)) return {};
  visited.add(interfaceName);

  const declaration = new RegExp(
    `export\\s+interface\\s+${interfaceName}(?:\\s+extends\\s+([^\\{]+))?\\s*\\{`,
  ).exec(source);
  if (!declaration) throw new Error(`No se encontró la interfaz TypeScript ${interfaceName}.`);

  const openingIndex = declaration.index + declaration[0].lastIndexOf("{");
  const ownFields = Object.fromEntries(
    splitTopLevel(balancedBraces(source, openingIndex), ";").flatMap((field) => {
      const match = field.trim().match(/^([A-Za-z][A-Za-z0-9_]*)(\?)?\s*:\s*([\s\S]+)$/);
      if (!match || match[3].includes("{")) return [];
      const normalized = normalizeTypescriptFieldType(match[3], aliases);
      return [[match[1], match[2] ? `optional<${normalized}>` : normalized]];
    }),
  );
  const inheritedFields = Object.assign(
    {},
    ...(declaration[1] ?? "")
      .split(",")
      .map((parent) => parent.trim())
      .filter(Boolean)
      .map((parent) => typescriptInterfaceFieldTypes(source, parent, aliases, visited)),
  );

  return { ...inheritedFields, ...ownFields };
}

function duplicates(values: string[]): string[] {
  return values.filter((value, index) => values.indexOf(value) !== index);
}

describe("contrato IPC", () => {
  it("mantiene en paridad los comandos Tauri registrados y la fachada TypeScript", () => {
    const rustSource = readFileSync(resolve("src-tauri/src/lib.rs"), "utf8");
    const bridgeSource = readFileSync(resolve("src/bridge.ts"), "utf8");

    const registered = registeredTauriCommands(rustSource);
    const invoked = invokedBridgeCommands(bridgeSource);

    expect(duplicates(registered), "comandos Rust registrados más de una vez").toEqual([]);
    expect(duplicates(invoked), "comandos invocados más de una vez desde el bridge").toEqual([]);
    expect([...invoked].sort()).toEqual([...registered].sort());
  });

  it("mantiene en paridad los argumentos serializados de cada comando", () => {
    const rustSource = [
      readFileSync(resolve("src-tauri/src/lib.rs"), "utf8"),
      readFileSync(resolve("src-tauri/src/dataset.rs"), "utf8"),
      readFileSync(resolve("src-tauri/src/projects.rs"), "utf8"),
    ].join("\n");
    const bridgeSource = readFileSync(resolve("src/bridge.ts"), "utf8");
    const registered = registeredTauriCommands(rustSource);

    expect(bridgeCommandArguments(bridgeSource)).toEqual(
      rustCommandArguments(rustSource, registered),
    );
  });

  it("mantiene en paridad los tipos de retorno declarados por cada comando", () => {
    const rustSource = [
      readFileSync(resolve("src-tauri/src/lib.rs"), "utf8"),
      readFileSync(resolve("src-tauri/src/dataset.rs"), "utf8"),
      readFileSync(resolve("src-tauri/src/projects.rs"), "utf8"),
    ].join("\n");
    const bridgeSource = readFileSync(resolve("src/bridge.ts"), "utf8");
    const registered = registeredTauriCommands(rustSource);

    expect(bridgeCommandReturnTypes(bridgeSource)).toEqual(
      rustCommandReturnTypes(rustSource, registered),
    );
  });

  it("mantiene en paridad los campos de las estructuras compartidas", () => {
    const rustSource = [
      readFileSync(resolve("src-tauri/src/lib.rs"), "utf8"),
      readFileSync(resolve("src-tauri/src/dataset.rs"), "utf8"),
      readFileSync(resolve("src-tauri/src/projects.rs"), "utf8"),
    ].join("\n");
    const bridgeSource = readFileSync(resolve("src/bridge.ts"), "utf8");
    const sharedStructures: Array<[rust: string, typescript: string]> = [
      ["AppInfo", "AppInfo"],
      ["OperationProgress", "OperationProgress"],
      ["ExportResult", "ExportResult"],
      ["QualityRule", "QualityRule"],
      ["QualityRuleResult", "QualityRuleResult"],
      ["QualityValidationResult", "QualityValidationResult"],
      ["DatasetColumn", "DatasetColumn"],
      ["DatasetPreview", "DatasetPreview"],
      ["WorkbookSheet", "WorkbookSheet"],
      ["DatasetSourceInspection", "DatasetSourceInspection"],
      ["DatasetPage", "DatasetPage"],
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
      ["ProjectWorkspace", "ProjectWorkspace"],
    ];

    const contracts = Object.fromEntries(
      sharedStructures.map(([rustName, typescriptName]) => [
        typescriptName,
        {
          rust: rustStructFields(rustSource, rustName),
          typescript: typescriptInterfaceFields(bridgeSource, typescriptName),
        },
      ]),
    );

    for (const [name, fields] of Object.entries(contracts)) {
      expect(fields.typescript, `campos incompatibles en ${name}`).toEqual(fields.rust);
    }
  });

  it("mantiene en paridad los tipos concretos de los campos compartidos", () => {
    const rustSource = [
      readFileSync(resolve("src-tauri/src/lib.rs"), "utf8"),
      readFileSync(resolve("src-tauri/src/dataset.rs"), "utf8"),
      readFileSync(resolve("src-tauri/src/projects.rs"), "utf8"),
    ].join("\n");
    const bridgeSource = readFileSync(resolve("src/bridge.ts"), "utf8");
    const aliases = typescriptTypeAliases(bridgeSource);
    const sharedStructures: Array<[rust: string, typescript: string]> = [
      ["AppInfo", "AppInfo"],
      ["OperationProgress", "OperationProgress"],
      ["ExportResult", "ExportResult"],
      ["QualityRule", "QualityRule"],
      ["QualityRuleResult", "QualityRuleResult"],
      ["QualityValidationResult", "QualityValidationResult"],
      ["DatasetColumn", "DatasetColumn"],
      ["DatasetPreview", "DatasetPreview"],
      ["WorkbookSheet", "WorkbookSheet"],
      ["DatasetSourceInspection", "DatasetSourceInspection"],
      ["DatasetPage", "DatasetPage"],
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
      ["ProjectWorkspace", "ProjectWorkspace"],
    ];

    for (const [rustName, typescriptName] of sharedStructures) {
      expect(
        typescriptInterfaceFieldTypes(bridgeSource, typescriptName, aliases),
        `tipos de campo incompatibles en ${typescriptName}`,
      ).toEqual(rustStructFieldTypes(rustSource, rustName));
    }
  });
});
