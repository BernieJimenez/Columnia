import type { ImportProfile, ImportProfileColumn, ImportProfileTypeChange } from "./dataset-contracts";
import type { SavedRecipe } from "./recipe-contracts";
import type { QualityRule } from "./delivery-contracts";

export type ReusableTaskOutputFormat = "csv" | "json" | "parquet" | "sql" | "excel" | "sqlite" | "bundle";
export type ReusableTaskPrivacyMode = "none" | "mask" | "hash";

/** Local reusable flow settings. No source paths, credentials, destinations, or overwrite consent. */
export interface ReusableTask {
  version: 1;
  name: string;
  importProfile: ImportProfile;
  recipe: SavedRecipe | null;
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
