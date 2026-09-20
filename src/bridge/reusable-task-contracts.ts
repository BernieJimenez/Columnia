import type { ImportProfile, ImportProfileColumn, ImportProfileTypeChange } from "./dataset-contracts";
import type {
  RecipeCastTarget,
  RecipeDateFormat,
  RecipeDateTarget,
  SavedRecipe,
} from "./recipe-contracts";
import type { QualityRule } from "./delivery-contracts";

export type ReusableTaskOutputFormat = "csv" | "json" | "parquet" | "sql" | "excel" | "sqlite" | "bundle";
export type ReusableTaskPrivacyMode = "none" | "mask" | "hash";
export type ReusableTaskInvalidConversionAction = "review" | "nullify" | "excludeRow";

export type ReusableTaskConversionDecision =
  | {
      kind: "cast";
      column: string;
      target: RecipeCastTarget;
      onInvalid: ReusableTaskInvalidConversionAction;
    }
  | {
      kind: "date";
      column: string;
      format: RecipeDateFormat;
      target: RecipeDateTarget;
      onInvalid: ReusableTaskInvalidConversionAction;
    };

/** Conversion choices contain column metadata only and are valid for one exact input schema. */
export interface ImportExceptionPolicy {
  version: 1;
  baseline: "lexical";
  schema: ImportProfileColumn[];
  conversions: ReusableTaskConversionDecision[];
}

export type ReusableTaskExceptionPolicy = ImportExceptionPolicy;

/** Local reusable flow settings. No source paths, credentials, destinations, or overwrite consent. */
export interface ReusableTask {
  version: 1;
  name: string;
  importProfile: ImportProfile;
  recipe: SavedRecipe | null;
  exceptionPolicy?: ImportExceptionPolicy;
  qualityRules: QualityRule[];
  outputFormat: ReusableTaskOutputFormat;
  privacyMode: ReusableTaskPrivacyMode;
}

export interface ReusableTaskSummary {
  id: string;
  name: string;
  createdAt: string;
  updatedAt: string;
  inputColumnCount: number;
  hasRecipe: boolean;
  qualityRuleCount: number;
  outputFormat: ReusableTaskOutputFormat;
}

export interface ReusableTaskSchemaCompatibility {
  status: "ready" | "review_required";
  missingColumns: string[];
  addedColumns: string[];
  changedTypes: ImportProfileTypeChange[];
  orderChanged: boolean;
}

export type ReusableTaskSchema = ImportProfileColumn[];
