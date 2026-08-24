import type { DatasetJoinType } from "../../bridge";

export type JoinStatus =
  | { kind: "idle" }
  | { kind: "loading"; joinType: DatasetJoinType }
  | { kind: "error"; message: string };

export function beginJoin(joinType: DatasetJoinType): JoinStatus {
  return { kind: "loading", joinType };
}

export function failJoin(message: string): JoinStatus {
  return { kind: "error", message };
}

export function clearJoin(): JoinStatus {
  return { kind: "idle" };
}
