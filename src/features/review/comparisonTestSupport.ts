import { vi } from "vitest";

import type { DatasetComparison } from "../../bridge";
import type { ReviewComparison } from "./useReviewController";

export const testComparison: DatasetComparison = {
  currentFileName: "ventas.csv",
  comparedFileName: "ventas-2.csv",
  currentRowCount: 3,
  comparedRowCount: 3,
  commonRowCount: 2,
  currentOnlyRowCount: 1,
  comparedOnlyRowCount: 1,
  sharedColumns: ["id", "importe"],
  currentOnlyColumns: [],
  comparedOnlyColumns: [],
  schemaCompatible: true,
  keyColumns: ["id"],
  matchedKeyCount: 3,
  currentOnlyKeyCount: 0,
  comparedOnlyKeyCount: 0,
  conflictingKeyCount: 0,
  duplicateKeyCount: 0,
  conflicts: [],
  conflictOffset: 0,
  conflictsTruncated: false,
  canConsolidate: true,
  options: { numericTolerance: false, ignoreCase: false, trimSpaces: false },
};

export const testComparisonWithConflicts: DatasetComparison = {
  ...testComparison,
  conflictingKeyCount: 60,
  canConsolidate: false,
  conflicts: [{ key: ["7"], cells: [{ column: "importe", current: "10", compared: "12" }] }],
  conflictOffset: 50,
};

/** A full comparison controller with spies, for the Revisar and Preparar views. */
export function makeReviewComparison(overrides: Partial<ReviewComparison> = {}): ReviewComparison {
  return {
    status: { kind: "ready", comparison: testComparison },
    cancellationPending: false,
    keyColumns: ["id"],
    onKeyColumnsChange: vi.fn(),
    options: { numericTolerance: false, ignoreCase: false, trimSpaces: false },
    onOptionsChange: vi.fn(),
    onCompare: vi.fn(),
    onCancelComparison: vi.fn(),
    onClear: vi.fn(),
    onConsolidate: vi.fn(),
    onResolveConflicts: vi.fn(),
    onConflictPageChange: vi.fn(),
    joinStatus: { kind: "idle" },
    mutationStatus: { kind: "idle" },
    mutationCancellationPending: false,
    joinType: "inner",
    onJoinTypeChange: vi.fn(),
    onJoin: vi.fn(),
    onCancelMutation: vi.fn(),
    ...overrides,
  };
}
