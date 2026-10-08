import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";

import type {
  AppInfo,
  DatasetColumn,
  DatasetFormat,
  DatasetImportSchemaPreview,
  DatasetPreview,
  DatasetProfile,
  DatasetSourceInspection,
  DelimitedHeaderReview,
  ExplorePanel,
  ExportResult,
  HistoryState,
  ProjectCatalogSnapshot,
  ProjectSummary,
  ProjectWorkspace,
  ReusableTaskSchemaCompatibility,
} from "../../src/bridge";

/**
 * QA-06: the one Tauri IPC mock of the E2E suite. Every response is built
 * from a fixture typed with the bridge contracts in `src/bridge`, so a change
 * of shape in Rust/TypeScript breaks `tsc` here instead of passing green.
 * The mock keeps plain state (saved projects and tasks) but never reproduces
 * engine logic: results that the engine computes are fixtures.
 */

const packageVersion = (JSON.parse(readFileSync(new URL("../../package.json", import.meta.url), "utf8")) as { version: string }).version;

export interface TauriMockOptions {
  dataset: DatasetPreview;
  format?: DatasetFormat;
  /** Workbook sheets for an Excel source; empty for other formats. */
  sheets?: Array<{ id: string; name: string }>;
  profile?: DatasetProfile;
  explore?: ExplorePanel;
  exportResult?: ExportResult;
  /** A saved project; with `recover`, it is also the recovery candidate. */
  project?: { summary: ProjectSummary; workspace?: Partial<ProjectWorkspace>; recover?: boolean };
  /** What the engine answers when a saved task is checked against the schema. */
  taskSchema?: ReusableTaskSchemaCompatibility;
}

/** Everything the page script needs, already shaped by the contracts. */
interface MockFixtures {
  appInfo: AppInfo;
  source: DatasetSourceInspection;
  headerReview: DelimitedHeaderReview;
  schemaPreview: DatasetImportSchemaPreview;
  dataset: DatasetPreview;
  profile: DatasetProfile;
  explore: ExplorePanel | null;
  history: HistoryState;
  exportResult: ExportResult | null;
  emptyCatalog: ProjectCatalogSnapshot;
  project: { summary: ProjectSummary; workspace: ProjectWorkspace; recover: boolean } | null;
  taskSchema: ReusableTaskSchemaCompatibility;
}

export function emptyProfile(dataset: DatasetPreview): DatasetProfile {
  return { rowCount: dataset.rowCount, duplicateRowCount: 0, nearDuplicateRowCount: 0, duplicatePercentage: 0, columns: [] };
}

function generatedColumns(columns: DatasetColumn[]): DatasetColumn[] {
  return columns.map((column, index) => ({ ...column, name: `column_${index + 1}` }));
}

function fixtures(options: TauriMockOptions): MockFixtures {
  const { dataset } = options;
  const format = options.format ?? "csv";
  const sheets = options.sheets ?? [];
  return {
    appInfo: { name: "Columnia", version: packageVersion, platform: "windows", updaterConfigured: false },
    source: {
      selectionId: `selection-${format}`,
      fileName: dataset.fileName,
      fileSizeBytes: dataset.fileSizeBytes,
      format,
      sheets,
      defaultSheetId: sheets[0]?.id ?? null,
      isCompressedContainer: format === "excel",
      resourceEstimate: { processingPath: "inMemory", estimatedMaterializationRamBytes: 268_435_968, estimatedTemporaryDiskBytes: null },
    },
    headerReview: {
      delimiter: ",",
      firstRow: { headerMode: "firstRow", columns: dataset.columns, rows: dataset.rows, includesFirstRow: false, sampleTruncated: false },
      generated: { headerMode: "generated", columns: generatedColumns(dataset.columns), rows: dataset.rows, includesFirstRow: true, sampleTruncated: false },
    },
    schemaPreview: { rowCount: dataset.rowCount, columns: dataset.columns, schemaMismatch: null },
    dataset,
    profile: options.profile ?? emptyProfile(dataset),
    explore: options.explore ?? null,
    history: {
      canUndo: false, canRedo: false, currentIndex: 0, entryCount: 0, entries: [], snapshotsEnabled: true,
      degradedReason: null, maxEntries: 50, diskBytes: 0, diskBudgetBytes: 536_870_912,
    },
    exportResult: options.exportResult ?? null,
    emptyCatalog: { projects: [], recoveryCandidate: null },
    project: options.project
      ? {
        summary: options.project.summary,
        workspace: { qualityRules: [], recipeDraft: null, ...options.project.workspace },
        recover: options.project.recover ?? false,
      }
      : null,
    taskSchema: options.taskSchema ?? { status: "ready", missingColumns: [], addedColumns: [], changedTypes: [], orderChanged: false },
  };
}

export type RecordedCall = { command: string; args: Record<string, unknown> };

/** Installs the mock before the app loads; calls are kept for assertions. */
export async function installTauriMock(page: Page, options: TauriMockOptions): Promise<void> {
  await page.addInitScript((fixture: MockFixtures) => {
    const calls: Array<{ command: string; args: Record<string, unknown> }> = [];
    let callbackId = 0;
    let projects: ProjectSummary[] = fixture.project?.recover ? [fixture.project.summary] : [];
    let recoveryCandidate: ProjectSummary | null = fixture.project?.recover ? fixture.project.summary : null;
    let task: { id: string; body: Record<string, unknown>; summary: Record<string, unknown> } | null = null;

    const invoke = async (command: string, args: Record<string, unknown> = {}) => {
      calls.push({ command, args });
      switch (command) {
        case "plugin:event|listen": return ++callbackId;
        case "plugin:event|unlisten":
        case "discard_dataset_selection":
        case "clear_dataset_comparison":
        case "cancel_operation":
        case "set_unsaved_work":
          return null;
        case "get_app_info": return fixture.appInfo;
        case "get_session_status": return { previousExitUnclean: false, setAsideCatalogs: [], recentCrashReports: 0 };
        case "list_sample_datasets":
        case "list_delivery_presets":
          return [];
        case "pick_dataset_source": return fixture.source;
        case "inspect_workbook_sheets": return fixture.source.sheets;
        case "preview_delimited_header_review": return fixture.headerReview;
        case "preview_dataset_selection": return fixture.schemaPreview;
        case "load_dataset_selection": return fixture.dataset;
        case "get_dataset_profile": return fixture.profile;
        case "get_history_state": return fixture.history;
        case "get_dataset_page": return { offset: args.offset ?? 0, rows: fixture.dataset.rows };
        case "get_explore_panel":
          if (!fixture.explore) break;
          return fixture.explore;
        case "export_dataset":
          if (!fixture.exportResult) break;
          return fixture.exportResult;
        case "list_projects": return { projects, recoveryCandidate };
        case "save_project": {
          if (!fixture.project) break;
          const saved = {
            ...fixture.project.summary,
            ...(projects[0] ?? {}),
            id: typeof args.projectId === "string" ? args.projectId : fixture.project.summary.id,
            name: typeof args.name === "string" ? args.name : fixture.project.summary.name,
          };
          projects = [saved];
          recoveryCandidate = null;
          return saved;
        }
        case "open_project": {
          if (!fixture.project) break;
          recoveryCandidate = null;
          const opened = projects.find((item) => item.id === args.projectId) ?? fixture.project.summary;
          return { project: opened, dataset: fixture.dataset, workspace: fixture.project.workspace, profile: null };
        }
        case "delete_project":
          projects = [];
          recoveryCandidate = null;
          return null;
        case "list_reusable_tasks": return task ? [task.summary] : [];
        case "save_reusable_task": {
          const body = args.task as Record<string, unknown> & { name: string; importProfile: { schema: unknown[] }; recipe: unknown; qualityRules: unknown[]; outputFormat: string };
          const id = typeof args.taskId === "string" ? args.taskId : "task-e2e";
          const summary = {
            id, name: body.name, createdAt: "2026-09-21T00:00:00.000Z", updatedAt: "2026-09-21T00:00:00.000Z",
            inputColumnCount: body.importProfile.schema.length, hasRecipe: body.recipe !== null,
            qualityRuleCount: body.qualityRules.length, outputFormat: body.outputFormat,
          };
          task = { id, body, summary };
          return summary;
        }
        case "open_reusable_task":
          if (!task || args.taskId !== task.id) throw new Error("Tarea no encontrada");
          return task.body;
        case "check_reusable_task_schema":
          if (!task || args.taskId !== task.id) throw new Error("Tarea no encontrada");
          return fixture.taskSchema;
        case "delete_reusable_task":
          task = null;
          return null;
        default:
          break;
      }
      throw new Error(`Comando Tauri no simulado: ${command}`);
    };

    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: { invoke, transformCallback: () => ++callbackId, unregisterCallback: () => undefined },
    });
    Object.defineProperty(window, "__COLUMNIA_E2E_CALLS__", { configurable: true, value: calls });
  }, fixtures(options));
}

/** The IPC calls the page made, in order. */
export async function recordedCalls(page: Page): Promise<RecordedCall[]> {
  return page.evaluate(() => (window as Window & { __COLUMNIA_E2E_CALLS__?: RecordedCall[] }).__COLUMNIA_E2E_CALLS__ ?? []);
}
