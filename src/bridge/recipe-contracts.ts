import type {
  DatasetPreview,
} from "./dataset-contracts";
import type {
  LocalExportFormat,
  PrivacyMode,
} from "./delivery-contracts";

export type RecipeCastTarget = "string" | "integer" | "decimal" | "boolean";
export type RecipeDateFormat = "iso8601" | "ymd" | "dmy" | "mdy";
export type RecipeDateTarget = "date" | "datetime";
export type RecipeFilterOperator =
  | "eq" | "neq" | "gt" | "lt" | "gte" | "lte"
  | "contains" | "not_contains" | "is_null" | "not_null";
export type CalculatedOperation =
  | "add" | "subtract" | "multiply" | "divide" | "concat"
  | "year" | "month" | "day";
export type CalculatedOperandKind = "literal" | "column";
export type FindReplaceScope = "column" | "all_text_columns";
export type OutlierAction = "cap" | "drop" | "impute";
export type SummaryOperation = "sum" | "mean" | "min" | "max" | "count" | "count_unique";
export type ContactKind = "email" | "phone" | "address";
export type ExtractionKind =
  | "first_token" | "last_token" | "digits" | "letters" | "before" | "after";

// Alias públicos históricos. Mantenerlos evita romper consumidores existentes.

interface RecipeRename {
  from: string;
  to: string;
}

/** PROD-11: how the text writes decimals; omitted reads a dot. */
type RecipeDecimalSeparator = "dot" | "comma";

interface RecipeCast {
  column: string;
  target: RecipeCastTarget;
  decimalSeparator?: RecipeDecimalSeparator;
}

interface RecipeDateParse {
  column: string;
  format: RecipeDateFormat;
  target: RecipeDateTarget;
}

export interface RecipeFilter {
  column: string;
  operator: RecipeFilterOperator;
  value: string | null;
}

interface CalculatedOperand {
  kind: CalculatedOperandKind;
  value: string;
}


interface CalculatedColumnRecipe {
  name: string;
  source: string;
  operation: CalculatedOperation;
  operand: CalculatedOperand | null;
}

interface FindReplaceRecipe {
  scope: FindReplaceScope;
  column: string | null;
  find: string;
  replace: string;
  regex: boolean;
}

interface SplitColumnRecipe {
  source: string;
  delimiter: string;
  names: string[];
  dropSource: boolean;
}

interface MergeColumnsRecipe {
  sources: string[];
  name: string;
  separator: string;
  dropSources: boolean;
}

interface OutlierTreatment {
  column: string;
  action: OutlierAction;
}

interface SummaryAggregation {
  column: string;
  operation: SummaryOperation;
}

interface GroupSummaryRecipe {
  groupBy: string[];
  aggregations: SummaryAggregation[];
}

interface ContactNormalization {
  column: string;
  kind: ContactKind;
}

interface TextExtraction {
  source: string;
  kind: ExtractionKind;
  name: string;
  delimiter: string | null;
}

export interface TransformRecipe {
  renames: RecipeRename[];
  casts: RecipeCast[];
  dateParses: RecipeDateParse[];
  filters: RecipeFilter[];
  calculatedColumn: CalculatedColumnRecipe | null;
  findReplace: FindReplaceRecipe | null;
  keepColumns: string[] | null;
  splitColumn: SplitColumnRecipe | null;
  mergeColumns: MergeColumnsRecipe | null;
  outlierTreatments: OutlierTreatment[];
  groupSummary: GroupSummaryRecipe | null;
  contactNormalizations: ContactNormalization[];
  textExtractions: TextExtraction[];
}

export interface SavedRecipe {
  version: 1 | 2;
  name: string;
  savedAt: string;
  recipe: TransformRecipe;
  sourceSchema?: RecipeSourceColumn[];
  exportOptions?: RecipeExportOptions;
}

interface RecipeSourceColumn {
  name: string;
  dataType: string;
}

export type LoadedRecipe = SavedRecipe;

export interface RecipeExportOptions {
  formats: LocalExportFormat[];
  selectedColumns: string[];
  privacyMode: PrivacyMode;
}

export interface TransformRecipeResult {
  dataset: DatasetPreview;
  changed: boolean;
  renamedColumnCount: number;
  convertedColumnCount: number;
  parsedDateColumnCount: number;
  removedRowCount: number;
  calculatedColumnCount: number;
  replacedCellCount: number;
  droppedColumnCount: number;
  splitColumnCount: number;
  mergedColumnCount: number;
  droppedSourceColumnCount: number;
  adjustedOutlierCellCount: number;
  outlierRemovedRowCount: number;
  outlierColumnCount: number;
  groupCount: number;
  aggregatedColumnCount: number;
  collapsedRowCount: number;
  normalizedContactCellCount: number;
  normalizedContactColumnCount: number;
  extractedColumnCount: number;
}
