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
export type TransformTarget = RecipeCastTarget;
export type DateInputFormat = RecipeDateFormat;
export type DateTarget = RecipeDateTarget;
export type FilterOperator = RecipeFilterOperator;
export type CalculationOperation = CalculatedOperation;

export interface RecipeRename {
  from: string;
  to: string;
}

export interface RecipeCast {
  column: string;
  target: RecipeCastTarget;
}

export interface RecipeDateParse {
  column: string;
  format: RecipeDateFormat;
  target: RecipeDateTarget;
}

export interface RecipeFilter {
  column: string;
  operator: RecipeFilterOperator;
  value: string | null;
}

export interface CalculatedOperand {
  kind: CalculatedOperandKind;
  value: string;
}

export type CalculationOperand = CalculatedOperand;

export interface CalculatedColumnRecipe {
  name: string;
  source: string;
  operation: CalculatedOperation;
  operand: CalculatedOperand | null;
}

export interface FindReplaceRecipe {
  scope: FindReplaceScope;
  column: string | null;
  find: string;
  replace: string;
  regex: boolean;
}

export interface SplitColumnRecipe {
  source: string;
  delimiter: string;
  names: string[];
  dropSource: boolean;
}

export interface MergeColumnsRecipe {
  sources: string[];
  name: string;
  separator: string;
  dropSources: boolean;
}

export interface OutlierTreatment {
  column: string;
  action: OutlierAction;
}

export interface SummaryAggregation {
  column: string;
  operation: SummaryOperation;
}

export interface GroupSummaryRecipe {
  groupBy: string[];
  aggregations: SummaryAggregation[];
}

export interface ContactNormalization {
  column: string;
  kind: ContactKind;
}

export interface TextExtraction {
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
  version: 1;
  name: string;
  savedAt: string;
  recipe: TransformRecipe;
  exportOptions?: RecipeExportOptions;
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
