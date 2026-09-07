import type {
  PerformanceProfile,
} from "./system-contracts";
import type {
  DatasetPreview,
  DatasetJoinType,
  DatasetQueryEngine,
  DatasetProfile,
} from "./dataset-contracts";
import type {
  SavedRecipe,
} from "./recipe-contracts";
import type {
  ExportFormat,
  PrivacyMode,
  QualityRule,
} from "./delivery-contracts";

export interface ProjectSummary {
  id: string;
  name: string;
  datasetFileName: string;
  rowCount: number;
  columnCount: number;
  createdAt: string;
  updatedAt: string;
}

export interface ProjectOpenResult {
  project: ProjectSummary;
  dataset: DatasetPreview;
  workspace: ProjectWorkspace;
  profile: DatasetProfile | null;
}

export interface SqlQueryHistoryEntry {
  id: number;
  outcome: "success" | "error" | "cancelled";
  durationMs: number;
  rowCount: number | null;
}

export interface ProjectWorkspace {
  qualityRules: QualityRule[];
  recipeDraft: SavedRecipe | null;
  sqlHistory?: SqlQueryHistoryEntry[];
  reviewTab?: ProjectReviewTab;
  previewOffset?: number;
  activePhase?: ProjectActivePhase;
  queryEngine?: DatasetQueryEngine;
  analysisSampleRows?: 10_000 | 50_000 | 100_000;
  performanceProfile?: PerformanceProfile;
  exportFormat?: ExportFormat;
  privacyMode?: PrivacyMode;
  comparisonKeyColumns?: string[];
  joinType?: DatasetJoinType;
}

export type ProjectReviewTab = "diagnosis" | "preview";
export type ProjectActivePhase = "load" | "review" | "prepare" | "deliver";
