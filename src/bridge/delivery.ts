import { invoke } from "@tauri-apps/api/core";
import type {
  SavedRecipe,
  ExportFormat,
  PrivacyMode,
  DatabaseTarget,
  QualityRule,
  QualityValidationResult,
  QualityMigrationResult,
  QualityRulesDocument,
  ExportResult,
  DatabaseConnectionResult,
} from "./contracts";
import { progressChannel, type ProgressHandler } from "./progress";

export function exportDataset(
  format: ExportFormat,
  qualityRules: QualityRule[],
  allowUnvalidated: boolean,
  onProgress?: ProgressHandler,
  privacyMode: PrivacyMode = "none",
  recipe?: SavedRecipe | null,
): Promise<ExportResult | null> {
  return invoke<ExportResult | null>("export_dataset", {
    format,
    qualityRules,
    allowUnvalidated,
    privacyMode,
    onProgress: progressChannel(onProgress),
    recipe: recipe ?? null,
  });
}

export function testDatabaseConnection(target: DatabaseTarget): Promise<DatabaseConnectionResult> {
  return invoke<DatabaseConnectionResult>("test_database_connection", { target });
}

export function exportDatasetToDatabase(
  target: DatabaseTarget,
  qualityRules: QualityRule[],
  allowUnvalidated: boolean,
  onProgress?: ProgressHandler,
  privacyMode: PrivacyMode = "none",
): Promise<ExportResult> {
  return invoke<ExportResult>("export_dataset_to_database", {
    target,
    qualityRules,
    allowUnvalidated,
    privacyMode,
    onProgress: progressChannel(onProgress),
  });
}

export function openLastExport(): Promise<void> {
  return invoke<void>("open_last_export");
}

export function validateQualityRules(
  qualityRules: QualityRule[],
): Promise<QualityValidationResult> {
  return invoke<QualityValidationResult>("validate_quality_rules", { qualityRules });
}

export function pickQualityRulesMigration(): Promise<QualityMigrationResult | null> {
  return invoke<QualityMigrationResult | null>("pick_quality_rules_migration");
}

export function saveQualityRulesDocument(
  qualityRules: QualityRule[],
): Promise<QualityRulesDocument | null> {
  return invoke<QualityRulesDocument | null>("save_quality_rules_document", { qualityRules });
}
