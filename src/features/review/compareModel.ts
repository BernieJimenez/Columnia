import type { DatasetComparison } from "../../bridge";

export type ComparisonStatus =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "ready"; comparison: DatasetComparison }
  | { kind: "error"; message: string };

export function beginComparison(): ComparisonStatus {
  return { kind: "loading" };
}

export function completeComparison(comparison: DatasetComparison): ComparisonStatus {
  return { kind: "ready", comparison };
}

export function failComparison(message: string): ComparisonStatus {
  return { kind: "error", message };
}

export function clearComparison(): ComparisonStatus {
  return { kind: "idle" };
}
