import type {
  DatabaseTarget,
  ExportFormat,
  ExportResult,
  OperationProgress,
  PrivacyMode,
  QualityRule,
  QualityValidationResult,
  SqlServerScriptOptions,
} from "../../bridge";

// The state of the delivery contract and of the export, read by the delivery
// controller that App mounts. Kept apart from deliveryModel so the entry chunk
// does not carry the rest of the delivery model, which only Entregar uses.

export function isDatabaseExportFormat(format: ExportFormat): boolean {
  return format === "postgresql" || format === "mysql" || format === "sqlserver";
}

type QualityGateState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "ready"; result: QualityValidationResult }
  | { kind: "stale"; result: QualityValidationResult }
  | { kind: "error"; message: string };

export type DeliveryContractState =
  | {
      kind: "without_contract";
      confirmation: "required" | "confirmed";
      gate: QualityGateState;
    }
  | {
      kind: "with_contract";
      rules: QualityRule[];
      gate: QualityGateState;
    };

export type DeliveryContractAction =
  | { kind: "rules_changed"; rules: QualityRule[] }
  | { kind: "gate_changed"; gate: QualityGateState }
  | { kind: "confirmation_changed"; confirmation: "required" | "confirmed" };

export type DeliveryExportState =
  | { kind: "idle" }
  | {
      kind: "loading";
      format: ExportFormat;
      progress: OperationProgress;
      cancellation: "available" | "requested";
      cancellationError?: string;
    }
  | { kind: "success"; result: ExportResult }
  | { kind: "cancelled" }
  // AUD-030: `uncertain` is set where the error is caught, never read back from the text.
  | { kind: "error"; message: string; uncertain?: boolean };

export type DeliveryExportRequest =
  | {
      format: ExportFormat;
      privacyMode: PrivacyMode;
      databaseTarget?: DatabaseTarget;
      sqlServerScript?: SqlServerScriptOptions;
      validation: { kind: "contract"; rules: QualityRule[] };
    }
  | {
      format: ExportFormat;
      privacyMode: PrivacyMode;
      databaseTarget?: DatabaseTarget;
      sqlServerScript?: SqlServerScriptOptions;
      validation: { kind: "explicitly_unvalidated" };
    };

export const INITIAL_DELIVERY_CONTRACT: DeliveryContractState = {
  kind: "without_contract",
  confirmation: "required",
  gate: { kind: "idle" },
};

export function deliveryContractFromRules(rules: QualityRule[]): DeliveryContractState {
  return rules.length === 0
    ? INITIAL_DELIVERY_CONTRACT
    : { kind: "with_contract", rules, gate: { kind: "idle" } };
}

export function deliveryRules(state: DeliveryContractState): QualityRule[] {
  return state.kind === "with_contract" ? state.rules : [];
}

function invalidateGate(gate: QualityGateState): QualityGateState {
  return gate.kind === "ready" || gate.kind === "stale"
    ? { kind: "stale", result: gate.result }
    : { kind: "idle" };
}

export function reduceDeliveryContract(
  state: DeliveryContractState,
  action: DeliveryContractAction,
): DeliveryContractState {
  if (action.kind === "gate_changed") return { ...state, gate: action.gate };

  if (action.kind === "confirmation_changed") {
    return state.kind === "without_contract"
      ? { ...state, confirmation: action.confirmation }
      : state;
  }

  const gate = invalidateGate(state.gate);
  return action.rules.length === 0
    ? { kind: "without_contract", confirmation: "required", gate }
    : { kind: "with_contract", rules: action.rules, gate };
}

export function invalidateDeliveryContract(state: DeliveryContractState): DeliveryContractState {
  const gate = invalidateGate(state.gate);
  return state.kind === "without_contract"
    ? { ...state, confirmation: "required", gate }
    : { ...state, gate };
}
