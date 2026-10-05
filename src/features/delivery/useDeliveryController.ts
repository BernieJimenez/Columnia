import { isCancellationError } from "../../bridge/cancellation";
import { useEffect, useRef, useState, type MutableRefObject } from "react";

import {
  cancelOperation,
  exportDataset,
  exportDatasetToDatabase,
  type ExportFormat,
  type OperationProgress,
  type PrivacyMode,
  type QualityRule,
  type SavedRecipe,
} from "../../bridge";
import {
  INITIAL_DELIVERY_CONTRACT,
  deliveryContractFromRules,
  deliveryRules,
  invalidateDeliveryContract,
  isDatabaseExportFormat,
  reduceDeliveryContract,
  type DeliveryContractAction,
  type DeliveryContractState,
  type DeliveryExportRequest,
  type DeliveryExportState,
} from "./deliveryModel";

/** Delivery settings a project or a reusable task stores and restores. */
export interface DeliverySettings {
  qualityRules?: QualityRule[];
  exportFormat?: ExportFormat;
  privacyMode?: PrivacyMode;
}

interface DeliveryControllerOptions {
  /** Revision of the active dataset; stale export responses are dropped against it. */
  datasetRevisionRef: MutableRefObject<number>;
  datasetReady: boolean;
  /**
   * Identity of the visible dataset. When it changes the quality gate and the
   * last export no longer describe the data.
   */
  datasetFingerprint: string | null;
  /** A bundle export carries the recipe draft that produced the data. */
  recipeDraft: SavedRecipe | null;
}


function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function useDeliveryController({
  datasetRevisionRef,
  datasetReady,
  datasetFingerprint,
  recipeDraft,
}: DeliveryControllerOptions) {
  const [exportFormat, setExportFormat] = useState<ExportFormat>("csv");
  const [privacyMode, setPrivacyMode] = useState<PrivacyMode>("none");
  const [exportStatus, setExportStatus] = useState<DeliveryExportState>({ kind: "idle" });
  const [contract, setContract] = useState<DeliveryContractState>(INITIAL_DELIVERY_CONTRACT);
  // Once the person picks «Exportar sin validar» for a dataset, suggestions
  // no longer switch validation back on (UX-01).
  const validationDeclinedRef = useRef(false);
  const exportRequestRef = useRef(0);
  const exportInFlightRef = useRef(false);
  const previousFingerprintRef = useRef<string | null>(null);

  function invalidateGate() {
    setContract(invalidateDeliveryContract);
    setExportStatus({ kind: "idle" });
  }

  useEffect(() => {
    if (datasetFingerprint === null) return;
    if (previousFingerprintRef.current !== null &&
        previousFingerprintRef.current !== datasetFingerprint) {
      invalidateGate();
    }
    previousFingerprintRef.current = datasetFingerprint;
  }, [datasetFingerprint]);

  /** A new dataset revision makes a pending export response stale. */
  function invalidateRequests() {
    exportRequestRef.current += 1;
    setExportStatus({ kind: "idle" });
  }

  function updateContract(action: DeliveryContractAction) {
    if (action.kind === "rules_changed") validationDeclinedRef.current = action.rules.length === 0;
    setContract((current) => reduceDeliveryContract(current, action));
    if (action.kind === "rules_changed") setExportStatus({ kind: "idle" });
  }

  /** Output format and protection go back to their defaults. */
  function resetOutput() {
    setExportFormat("csv");
    setPrivacyMode("none");
  }

  function resetContract() {
    validationDeclinedRef.current = false;
    setContract(INITIAL_DELIVERY_CONTRACT);
  }

  /**
   * Entregar starts validating with the checks the data already meets
   * (UX-01), unless the person already chose to export without validating
   * or already has rules of their own.
   */
  function proposeValidation(rules: QualityRule[]) {
    if (rules.length === 0 || validationDeclinedRef.current) return;
    setContract((current) => (current.kind === "without_contract" && current.confirmation === "required"
      ? reduceDeliveryContract(current, { kind: "rules_changed", rules })
      : current));
  }

  function applySettings(settings: DeliverySettings) {
    setContract(deliveryContractFromRules(settings.qualityRules ?? []));
    setExportStatus({ kind: "idle" });
    setExportFormat(settings.exportFormat ?? "csv");
    setPrivacyMode(settings.privacyMode ?? "none");
  }

  async function exportActiveDataset(request: DeliveryExportRequest) {
    if (!datasetReady) return;
    if (exportInFlightRef.current) {
      // FUN-56: an export made stale by a change of data is still writing.
      setExportStatus({ kind: "error", message: "La exportación anterior todavía está terminando; espera unos segundos y vuelve a exportar." });
      return;
    }
    exportInFlightRef.current = true;
    const requestId = ++exportRequestRef.current;
    const requestedRevision = datasetRevisionRef.current;
    const isCurrentRequest = () =>
      exportRequestRef.current === requestId && datasetRevisionRef.current === requestedRevision;
    const rules = request.validation.kind === "contract" ? request.validation.rules : [];
    const allowUnvalidated = request.validation.kind === "explicitly_unvalidated";
    setExportStatus({
      kind: "loading",
      format: request.format,
      progress: { operation: "export", stage: "Esperando destino", percent: 0 },
      cancellation: "available",
    });
    try {
      const onProgress = (progress: OperationProgress) => {
        if (!isCurrentRequest()) return;
        setExportStatus((current) =>
          current.kind === "loading" ? { ...current, progress } : current,
        );
      };
      let result;
      if (isDatabaseExportFormat(request.format)) {
        if (!request.databaseTarget) throw new Error("Falta configurar el destino de base de datos.");
        result = await exportDatasetToDatabase(
          request.databaseTarget,
          rules,
          allowUnvalidated,
          onProgress,
          request.privacyMode,
        );
      } else {
        result = recipeDraft && request.format === "bundle"
          ? await exportDataset(request.format, rules, allowUnvalidated, onProgress, request.privacyMode, recipeDraft)
          : await exportDataset(request.format, rules, allowUnvalidated, onProgress, request.privacyMode);
      }
      if (!isCurrentRequest()) {
        // The file exists, but it holds the data from before the change.
        if (result) {
          setExportStatus((current) => current.kind === "idle" || current.kind === "error"
            ? { kind: "error", message: `La exportación anterior terminó (${result.fileName}), pero ya no corresponde a los datos actuales. Vuelve a exportar.` }
            : current);
        }
        return;
      }
      setExportStatus(result ? { kind: "success", result } : { kind: "idle" });
    } catch (error: unknown) {
      if (!isCurrentRequest()) return;
      if (isCancellationError(error)) {
        setExportStatus({ kind: "cancelled" });
        return;
      }
      setExportStatus({ kind: "error", message: errorMessage(error) });
    } finally {
      exportInFlightRef.current = false;
    }
  }

  async function cancelExport() {
    setExportStatus((current) =>
      current.kind === "loading"
        ? { ...current, cancellation: "requested", cancellationError: undefined }
        : current,
    );
    try {
      await cancelOperation("export");
    } catch (error: unknown) {
      const message = errorMessage(error);
      setExportStatus((current) => current.kind === "loading"
        ? { ...current, cancellation: "available", cancellationError: message }
        : current);
    }
  }

  const qualityRules = deliveryRules(contract);

  return {
    contract,
    qualityRules,
    exportFormat,
    setExportFormat,
    privacyMode,
    setPrivacyMode,
    exportStatus,
    busy: contract.gate.kind === "loading" || exportStatus.kind === "loading",
    workspace: { qualityRules, exportFormat, privacyMode },
    exportActiveDataset,
    cancelExport,
    updateContract,
    proposeValidation,
    invalidateGate,
    invalidateRequests,
    resetOutput,
    resetContract,
    applySettings,
  };
}

export type DeliveryController = ReturnType<typeof useDeliveryController>;
