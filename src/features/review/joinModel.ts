import type { DatasetJoinType } from "../../bridge";

export type JoinStatus =
  | { kind: "idle" }
  | { kind: "loading"; joinType: DatasetJoinType }
  | { kind: "error"; message: string };

export type ReviewMutationKind = "join" | "consolidate" | "resolveConflicts";

export type ReviewMutationStatus =
  | { kind: "idle" }
  | {
      kind: "running";
      mutation: ReviewMutationKind;
      cancellation: "available" | "requested";
      cancellationError?: string;
    }
  | { kind: "finalizing"; mutation: ReviewMutationKind }
  | { kind: "error"; mutation: ReviewMutationKind; message: string };

export function beginJoin(joinType: DatasetJoinType): JoinStatus {
  return { kind: "loading", joinType };
}

export function failJoin(message: string): JoinStatus {
  return { kind: "error", message };
}

export function clearJoin(): JoinStatus {
  return { kind: "idle" };
}
