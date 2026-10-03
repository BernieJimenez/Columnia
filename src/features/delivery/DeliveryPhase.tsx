import { isCancellationError } from "../../bridge/cancellation";
import {
  useEffect,
  useEffectEvent,
  useRef,
  useState,
} from "react";

import {
  QUALITY_DATASET_COLUMN,
  cancelOperation,
  preflightDatabaseExport,
  pickQualityRulesMigration,
  saveQualityRulesDocument,
  validateQualityRules,
  type DatabaseTarget,
  type DeliveryPreset,
  type DatasetPreview,
  type ExportFormat,
  type PrivacyMode,
  type RemoteExportPreflight,
  type QualityMigrationResult,
  type QualityRulesDocument,
  type QualityRule,
  type QualityRuleKind,
  type SavedRecipe,
} from "../../bridge";
import { OperationProgressView } from "../../components/OperationProgressView";
import { formatFileSize } from "./DatasetMetrics";
import {
  type DeliveryContractAction,
  type DeliveryContractState,
  type DeliveryExportRequest,
  type DeliveryExportState,
  INITIAL_DATABASE_TARGET,
  databaseKindForExportFormat,
  isDatabaseExportFormat,
  validateDatabaseTargetDraft,
  validateQualityRuleDraft,
} from "./deliveryModel";
import { formatPercent } from "../../format";
import { DeliveryPresets } from "./DeliveryPresets";
import { DeliveryResult } from "./DeliveryResult";
import { QualityRulesEditor, withAddedRule } from "./QualityRulesEditor";

interface DeliveryPhaseProps {
  dataset: DatasetPreview;
  recipeDraft?: SavedRecipe | null;
  preparationChanges?: string[];
  contract: DeliveryContractState;
  exportState: DeliveryExportState;
  exportFormat?: ExportFormat;
  onExportFormatChange?: (format: ExportFormat) => void;
  privacyMode?: PrivacyMode;
  onPrivacyModeChange?: (mode: PrivacyMode) => void;
  /** Columns the quality profile flags as personal data (email, phone, address, name). */
  personalDataColumns?: string[];
  /** Rules the data already meets, offered when the delivery has no contract. */
  suggestedRules?: QualityRule[];
  /** Why the dataset would not open in Excel (RV19); empty when it fits. */
  excelLimitIssues?: string[];
  onContractAction: (action: DeliveryContractAction) => void;
  onExport: (request: DeliveryExportRequest) => void;
  onCancelExport: () => void;
}

const QUALITY_ISSUE_GUIDANCE: Record<QualityRuleKind, { title: string; nextStep: string }> = {
  not_null: {
    title: "Valores nulos",
    nextStep: "Confirma si el campo debe ser obligatorio. Corrige los faltantes en Preparar o ajusta el contrato si son válidos.",
  },
  non_empty: {
    title: "Texto vacío",
    nextStep: "Revisa si los textos vacíos representan datos faltantes y si la regla debe permitirlos.",
  },
  unique: {
    title: "Valores repetidos",
    nextStep: "Confirma que esta columna sea una clave única; corrige duplicados o elige la clave adecuada.",
  },
  numeric_range: {
    title: "Fuera del rango numérico",
    nextStep: "Revisa los límites y unidades esperados; corrige los valores fuera de rango o actualiza el contrato.",
  },
  allowed_values: {
    title: "Fuera de los valores permitidos",
    nextStep: "Compara el catálogo del contrato con los valores esperados; conserva las diferencias válidas o normalízalas en Preparar.",
  },
  regex: {
    title: "Formato distinto al patrón",
    nextStep: "Comprueba que el patrón describa el formato esperado y revisa los datos que no coinciden.",
  },
  dtype: {
    title: "Tipo de dato distinto",
    nextStep: "Verifica el tipo requerido y si corresponde convertir la columna en Preparar.",
  },
  unique_together: {
    title: "Combinación de valores repetida",
    nextStep: "Confirma que las columnas elegidas formen la clave compuesta y revisa las combinaciones repetidas.",
  },
  column_compare: {
    title: "Comparación entre columnas incumplida",
    nextStep: "Revisa las columnas y la relación configurada; verifica las filas que incumplen esa condición.",
  },
  referential_integrity: {
    title: "Valor fuera de las referencias",
    nextStep: "Comprueba que el conjunto de referencias y el alcance de los datos sean los esperados.",
  },
  monotonic: {
    title: "Orden esperado incumplido",
    nextStep: "Verifica la dirección esperada y el orden de los registros antes de cambiar la regla.",
  },
  aggregate_check: {
    title: "Agregado fuera del objetivo",
    nextStep: "Revisa la agregación, el alcance de filas, el valor objetivo y su tolerancia.",
  },
  aggregate_reconciliation: {
    title: "Agregados no conciliados",
    nextStep: "Confirma las columnas comparadas, el alcance de datos y la tolerancia de conciliación.",
  },
  distribution_drift: {
    title: "Media fuera de tolerancia",
    nextStep: "Compara la media actual con la de la línea base: verifica que ambas poblaciones sean comparables antes de ajustar el umbral.",
  },
  date_range: {
    title: "Fecha fuera del rango esperado",
    nextStep: "Comprueba los límites de fecha y si el periodo de los datos coincide con el esperado.",
  },
  conditional: {
    title: "Condición incumplida",
    nextStep: "Revisa la condición y la obligación asociada; corrige datos o ajusta la regla según el proceso esperado.",
  },
  schema_contract: {
    title: "Contrato de esquema incumplido",
    nextStep: "Revisa las columnas requeridas, el orden y las columnas adicionales configuradas en el contrato.",
  },
  row_count: {
    title: "Cantidad de filas fuera del rango",
    nextStep: "Comprueba el periodo y el alcance de los datos, además del mínimo y máximo configurados.",
  },
};

const QUALITY_RULE_SUMMARY: Record<QualityRuleKind, string> = {
  not_null: "no admite valores nulos",
  non_empty: "no admite texto vacío",
  unique: "debe contener valores únicos",
  numeric_range: "debe permanecer dentro del rango definido",
  allowed_values: "solo admite los valores permitidos",
  regex: "debe cumplir el formato configurado",
  dtype: "debe conservar el tipo de dato esperado",
  unique_together: "debe formar una combinación única",
  column_compare: "debe cumplir la comparación entre columnas",
  referential_integrity: "solo admite valores incluidos en la referencia",
  monotonic: "debe mantener el orden esperado",
  aggregate_check: "debe cumplir el total o agregado esperado",
  aggregate_reconciliation: "debe conciliar los agregados comparados",
  distribution_drift: "debe mantener su media cerca de la de la línea base",
  date_range: "debe permanecer dentro del periodo definido",
  conditional: "debe cumplir la condición configurada",
  schema_contract: "debe conservar las columnas requeridas",
  row_count: "debe mantener la cantidad de filas permitida",
};


function summarizeQualityRule(rule: QualityRule): string {
  const columns = rule.kind === "schema_contract" || rule.kind === "row_count"
    ? "Dataset"
    : rule.columns?.length
      ? rule.columns.join(", ")
      : rule.column;
  const tolerance = [
    rule.maxInvalid !== undefined ? `${rule.maxInvalid} incumplimientos` : null,
    rule.maxInvalidPct !== undefined ? `${rule.maxInvalidPct}%` : null,
  ].filter(Boolean).join(" y ");
  const toleranceSummary = rule.maxInvalid === 0 && rule.maxInvalidPct === undefined
    ? " · no se permiten incumplimientos"
    : tolerance ? ` · tolerancia ${tolerance}` : "";
  return `${columns}: ${QUALITY_RULE_SUMMARY[rule.kind]}${toleranceSummary}.`;
}

function databaseTargetErrorField(
  message: string | null,
): "connectionString" | "schema" | "table" | null {
  if (!message) return null;
  if (message.startsWith("Indica la cadena") || message.startsWith("La cadena ODBC")) {
    return "connectionString";
  }
  if (message.startsWith("El esquema")) return "schema";
  if (message.startsWith("La tabla")) return "table";
  return null;
}

function focusQualityRule(index: number) {
  const ruleElement = document.getElementById(`quality-rule-${index + 1}`);
  ruleElement?.scrollIntoView?.({ behavior: "smooth", block: "center" });
  ruleElement?.focus({ preventScroll: true });
}

export function DeliveryPhase({
  dataset,
  recipeDraft = null,
  preparationChanges = [],
  contract,
  exportState,
  exportFormat,
  onExportFormatChange,
  privacyMode,
  onPrivacyModeChange,
  personalDataColumns = [],
  suggestedRules = [],
  excelLimitIssues = [],
  onContractAction,
  onExport,
  onCancelExport,
}: DeliveryPhaseProps) {
  const [localPrivacyMode, setLocalPrivacyMode] = useState<PrivacyMode>("none");
  const [localExportFormat, setLocalExportFormat] = useState<ExportFormat>("csv");
  const [migrationState, setMigrationState] = useState<
    | { kind: "idle" }
    | { kind: "working" }
    | { kind: "ready"; result: QualityMigrationResult }
    | { kind: "error"; message: string }
  >({ kind: "idle" });
  const selectedPrivacyMode = privacyMode ?? localPrivacyMode;
  const selectedExportFormat = exportFormat ?? localExportFormat;
  const exceedsExcel = selectedExportFormat === "excel" && excelLimitIssues.length > 0;
  const [databaseTarget, setDatabaseTarget] = useState<DatabaseTarget>(INITIAL_DATABASE_TARGET);
  const [databasePreflightState, setDatabasePreflightState] = useState<
    | { kind: "idle" }
    | { kind: "working" }
    | { kind: "ready"; result: RemoteExportPreflight; fingerprint: string }
    | { kind: "error"; message: string }
  >({ kind: "idle" });
  const [databasePreflightCancellationPending, setDatabasePreflightCancellationPending] = useState(false);
  const [databasePreflightCancellationError, setDatabasePreflightCancellationError] = useState<string | null>(null);
  const databaseRequestGeneration = useRef(0);
  const exportRequestGeneration = useRef(0);
  const exportInFlightRef = useRef(false);
  // Set by «Exportar y abrir en Power BI»; the next successful export opens it.
  const openPowerBiAfterExportRef = useRef(false);
  const [autoOpenResult, setAutoOpenResult] = useState<object | null>(null);
  const databaseTargetFingerprint = JSON.stringify({
    target: databaseTarget,
    privacyMode: selectedPrivacyMode,
    dataset: [dataset.fileName, dataset.fileSizeBytes, dataset.rowCount,
      dataset.columns.map((column) => [column.name, column.dataType])],
  });
  const databaseTargetFingerprintRef = useRef(databaseTargetFingerprint);
  databaseTargetFingerprintRef.current = databaseTargetFingerprint;
  // A new dataset resets the remote target for the format chosen at that moment.
  const resetDatabaseTarget = useEffectEvent(() => {
    exportRequestGeneration.current += 1;
    const kind = databaseKindForExportFormat(selectedExportFormat);
    setDatabaseTarget({ ...INITIAL_DATABASE_TARGET, ...(kind ? { kind } : {}) });
    setDatabasePreflightState({ kind: "idle" });
    databaseRequestGeneration.current += 1;
  });
  useEffect(() => {
    resetDatabaseTarget();
  }, [dataset.fileName, dataset.fileSizeBytes, dataset.rowCount]);
  useEffect(() => {
    const kind = databaseKindForExportFormat(selectedExportFormat);
    if (!kind || databaseTarget.kind === kind) return;
    setDatabaseTarget((current) => ({ ...current, kind }));
    setDatabasePreflightState({ kind: "idle" });
    databaseRequestGeneration.current += 1;
  }, [databaseTarget.kind, selectedExportFormat]);
  const [qualityFileState, setQualityFileState] = useState<
    | { kind: "idle" }
    | { kind: "working" }
    | { kind: "ready"; document: QualityRulesDocument }
    | { kind: "error"; message: string }
  >({ kind: "idle" });
  const [qualityValidationCancellationPending, setQualityValidationCancellationPending] = useState(false);
  const [qualityValidationCancellationError, setQualityValidationCancellationError] = useState<string | null>(null);
  const [rulesEditorOpen, setRulesEditorOpen] = useState(false);
  const [pendingRuleFocus, setPendingRuleFocus] = useState<number | null>(null);
  const rules = contract.kind === "with_contract" ? contract.rules : [];
  const validationError = validateQualityRuleDraft(rules, dataset);
  const validationErrorRuleNumber = validationError?.match(/^Regla (\d+):/)?.[1];
  const validationErrorRuleIndex = validationErrorRuleNumber
    ? Number(validationErrorRuleNumber) - 1
    : null;
  const gatePassed = contract.gate.kind === "ready" && contract.gate.result.passed;
  const needsUnvalidatedConfirmation = contract.kind === "without_contract"
    && contract.confirmation !== "confirmed";
  const personalDataKey = personalDataColumns.join("\u0000");
  const suggestionsKey = suggestedRules.map((rule) => `${rule.kind}:${rule.column}`).join("|");
  const [suggestionChoice, setSuggestionChoice] = useState<{ key: string; excluded: number[] }>({ key: "", excluded: [] });
  const excludedSuggestions = suggestionChoice.key === suggestionsKey ? suggestionChoice.excluded : [];
  const chosenSuggestions = suggestedRules.filter((_, index) => !excludedSuggestions.includes(index));

  function toggleSuggestion(index: number) {
    setSuggestionChoice({
      key: suggestionsKey,
      excluded: excludedSuggestions.includes(index)
        ? excludedSuggestions.filter((value) => value !== index)
        : [...excludedSuggestions, index],
    });
  }
  const [confirmedUnprotectedKey, setConfirmedUnprotectedKey] = useState<string | null>(null);
  const exportsUnprotectedPersonalData = personalDataColumns.length > 0 && selectedPrivacyMode === "none";
  const needsPersonalDataConfirmation = exportsUnprotectedPersonalData && confirmedUnprotectedKey !== personalDataKey;
  const databaseTargetError = isDatabaseExportFormat(selectedExportFormat)
    ? validateDatabaseTargetDraft(databaseTarget)
    : null;
  const invalidDatabaseTargetField = databaseTargetErrorField(databaseTargetError);
  const databaseReady = !isDatabaseExportFormat(selectedExportFormat)
    || (databaseTargetError === null
      && databasePreflightState.kind === "ready"
      && databasePreflightState.fingerprint === databaseTargetFingerprint
      && databasePreflightState.result.kind === databaseTarget.kind
      && databasePreflightState.result.ready);
  const activePreflight = databasePreflightState.kind === "ready"
    && databasePreflightState.fingerprint === databaseTargetFingerprint
    ? databasePreflightState.result
    : null;
  const busy = exportState.kind === "loading"
    || contract.gate.kind === "loading"
    || migrationState.kind === "working"
    || qualityFileState.kind === "working"
    || databasePreflightState.kind === "working";
  const exportFormatLabel = {
    csv: "CSV",
    json: "JSON",
    parquet: "Parquet",
    sql: "SQL",
    excel: "Excel",
    sqlite: "SQLite",
    bundle: "Paquete ZIP",
    postgresql: "PostgreSQL",
    mysql: "MySQL",
    sqlserver: "SQL Server",
  }[selectedExportFormat];
  const approvedQualitySummary = contract.kind === "with_contract"
    && contract.gate.kind === "ready"
    && contract.gate.result.passed
    ? `${contract.gate.result.totalRules.toLocaleString()} ${contract.gate.result.totalRules === 1 ? "regla aprobada" : "reglas aprobadas"} sobre ${contract.gate.result.rowCount.toLocaleString()} ${contract.gate.result.rowCount === 1 ? "fila" : "filas"}`
    : null;
  useEffect(() => {
    if (!rulesEditorOpen || pendingRuleFocus === null) return;
    focusQualityRule(pendingRuleFocus);
    setPendingRuleFocus(null);
  }, [pendingRuleFocus, rulesEditorOpen]);

  function editQualityRule(index: number) {
    setPendingRuleFocus(index);
    setRulesEditorOpen(true);
  }
  function changeRules(nextRules: QualityRule[]) {
    exportRequestGeneration.current += 1;
    setQualityFileState({ kind: "idle" });
    onContractAction({ kind: "rules_changed", rules: nextRules });
  }

  async function importQualityRules() {
    setQualityFileState({ kind: "idle" });
    setMigrationState({ kind: "working" });
    try {
      const result = await pickQualityRulesMigration();
      if (result) {
        onContractAction({ kind: "rules_changed", rules: result.convertedRules });
        setMigrationState({ kind: "ready", result });
      } else {
        setMigrationState({ kind: "idle" });
      }
    } catch (error: unknown) {
      setMigrationState({
        kind: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  async function saveQualityContract() {
    if (contract.kind !== "with_contract" || validationError) return;
    setMigrationState({ kind: "idle" });
    setQualityFileState({ kind: "working" });
    try {
      const document = await saveQualityRulesDocument(contract.rules);
      setQualityFileState(document ? { kind: "ready", document } : { kind: "idle" });
    } catch (error: unknown) {
      setQualityFileState({
        kind: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  async function cancelQualityValidation() {
    if (contract.gate.kind !== "loading" || qualityValidationCancellationPending) return;
    setQualityValidationCancellationPending(true);
    setQualityValidationCancellationError(null);
    try {
      await cancelOperation("qualityValidation");
    } catch (error: unknown) {
      setQualityValidationCancellationError(error instanceof Error ? error.message : String(error));
      setQualityValidationCancellationPending(false);
    }
  }

  async function requestExport(format: ExportFormat) {
    if (exportInFlightRef.current) return;
    exportInFlightRef.current = true;
    const requestId = exportRequestGeneration.current + 1;
    exportRequestGeneration.current = requestId;
    const requestedGeneration = requestId;
    try {
      if (isDatabaseExportFormat(format)
        && (databaseTargetError !== null
          || databasePreflightState.kind !== "ready"
          || databasePreflightState.fingerprint !== databaseTargetFingerprint
          || !databasePreflightState.result.ready)) return;
      const databaseOptions = isDatabaseExportFormat(format) ? { databaseTarget } : {};
      if (contract.kind === "with_contract") {
        if (validationError) return;
        if (!gatePassed) {
          const previousGate = contract.gate;
          setQualityValidationCancellationPending(false);
          setQualityValidationCancellationError(null);
          onContractAction({ kind: "gate_changed", gate: { kind: "loading" } });
          try {
            const result = await validateQualityRules(contract.rules);
            if (exportRequestGeneration.current !== requestedGeneration) return;
            onContractAction({ kind: "gate_changed", gate: { kind: "ready", result } });
            if (!result.passed) return;
          } catch (error: unknown) {
            if (exportRequestGeneration.current !== requestedGeneration) return;
            if (isCancellationError(error)) {
              onContractAction({ kind: "gate_changed", gate: previousGate });
              return;
            }
            onContractAction({
              kind: "gate_changed",
              gate: {
                kind: "error",
                message: error instanceof Error ? error.message : String(error),
              },
            });
            return;
          }
        }
        if (exportRequestGeneration.current !== requestedGeneration) return;
        onExport({ format, privacyMode: selectedPrivacyMode, ...databaseOptions, validation: { kind: "contract", rules: contract.rules } });
      } else if (contract.confirmation === "confirmed") {
        if (exportRequestGeneration.current !== requestedGeneration) return;
        onExport({ format, privacyMode: selectedPrivacyMode, ...databaseOptions, validation: { kind: "explicitly_unvalidated" } });
      }
    } finally {
      exportInFlightRef.current = false;
      setQualityValidationCancellationPending(false);
    }
  }

  function changeExportFormat(format: ExportFormat) {
    exportRequestGeneration.current += 1;
    setLocalExportFormat(format);
    onExportFormatChange?.(format);
    const kind = databaseKindForExportFormat(format);
    if (kind) {
      setDatabaseTarget((current) => ({
        ...current,
        kind,
        ...(kind === "mysql" && current.tablePolicy === "replace"
          ? { tablePolicy: "create_only" as const }
          : {}),
      }));
      setDatabasePreflightState({ kind: "idle" });
      databaseRequestGeneration.current += 1;
    }
  }

  function changeDatabaseTarget(update: Partial<DatabaseTarget>) {
    exportRequestGeneration.current += 1;
    setDatabaseTarget((current) => ({ ...current, ...update }));
    setDatabasePreflightState({ kind: "idle" });
    databaseRequestGeneration.current += 1;
  }

  async function preflightDatabaseTarget() {
    if (databaseTargetError || databasePreflightState.kind === "working") return;
    const requestGeneration = databaseRequestGeneration.current + 1;
    databaseRequestGeneration.current = requestGeneration;
    const requestFingerprint = databaseTargetFingerprint;
    const requestedTarget = databaseTarget;
    const previousState = databasePreflightState;
    setDatabasePreflightCancellationPending(false);
    setDatabasePreflightCancellationError(null);
    setDatabasePreflightState({ kind: "working" });
    try {
      const result = await preflightDatabaseExport(requestedTarget, selectedPrivacyMode);
      if (requestGeneration !== databaseRequestGeneration.current
        || requestFingerprint !== databaseTargetFingerprintRef.current) return;
      if (result.kind !== requestedTarget.kind) {
        setDatabasePreflightState({
          kind: "error",
          message: "El preflight no corresponde al motor de destino seleccionado.",
        });
        return;
      }
      setDatabasePreflightCancellationError(null);
      setDatabasePreflightState({ kind: "ready", result, fingerprint: requestFingerprint });
    } catch (error: unknown) {
      if (requestGeneration !== databaseRequestGeneration.current
        || requestFingerprint !== databaseTargetFingerprintRef.current) return;
      if (isCancellationError(error)) {
        setDatabasePreflightState(previousState.kind === "ready"
          && previousState.fingerprint === requestFingerprint
          ? previousState
          : { kind: "idle" });
        return;
      }
      setDatabasePreflightState({
        kind: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    } finally {
      if (requestGeneration === databaseRequestGeneration.current) {
        setDatabasePreflightCancellationPending(false);
      }
    }
  }

  async function cancelDatabasePreflight() {
    if (databasePreflightState.kind !== "working" || databasePreflightCancellationPending) return;
    setDatabasePreflightCancellationPending(true);
    setDatabasePreflightCancellationError(null);
    try {
      await cancelOperation("databasePreflight");
    } catch (error: unknown) {
      setDatabasePreflightCancellationError(error instanceof Error ? error.message : String(error));
      setDatabasePreflightCancellationPending(false);
    }
  }

  function changePrivacyMode(mode: PrivacyMode) {
    exportRequestGeneration.current += 1;
    setLocalPrivacyMode(mode);
    onPrivacyModeChange?.(mode);
    setDatabasePreflightState({ kind: "idle" });
    databaseRequestGeneration.current += 1;
  }

  /** Applies a delivery preset; returns the notice about its database target, if any. */
  function applyDeliveryPreset(openedPreset: DeliveryPreset): string | null {
    changeExportFormat(openedPreset.format);
    setLocalPrivacyMode(openedPreset.privacyMode);
    onPrivacyModeChange?.(openedPreset.privacyMode);
    if (openedPreset.databaseTarget) {
      const storedTarget = openedPreset.databaseTarget;
      const destructivePolicy = storedTarget.tablePolicy === "replace";
      setDatabaseTarget({
        ...INITIAL_DATABASE_TARGET,
        kind: storedTarget.kind,
        schema: storedTarget.schema,
        table: storedTarget.table,
        tablePolicy: destructivePolicy ? "create_only" : storedTarget.tablePolicy,
      });
      setDatabasePreflightState({ kind: "idle" });
      databaseRequestGeneration.current += 1;
      if (destructivePolicy) {
        return "El preset proponía reemplazar la tabla. Se cargó Crear sin reemplazar; elige Reemplazar explícitamente y vuelve a analizar si quieres autorizarlo en esta sesión.";
      }
      return "Preset aplicado. Reingresa la conexión de esta sesión y ejecuta el preflight antes de escribir.";
    } else {
      setDatabaseTarget({ ...INITIAL_DATABASE_TARGET, ...(databaseKindForExportFormat(openedPreset.format)
        ? { kind: databaseKindForExportFormat(openedPreset.format)! }
        : {}) });
      setDatabasePreflightState({ kind: "idle" });
      databaseRequestGeneration.current += 1;
    }
    return null;
  }


  // «Exportar y abrir en Power BI»: the next successful CSV, Excel or Parquet export opens it.
  useEffect(() => {
    // Only the export that asked for it opens Power BI, once.
    if (exportState.kind !== "success") setAutoOpenResult(null);
    if (!openPowerBiAfterExportRef.current) return;
    if (exportState.kind === "success") {
      openPowerBiAfterExportRef.current = false;
      if (["CSV", "Excel", "Parquet"].includes(exportState.result.format)) {
        setAutoOpenResult(exportState.result);
      }
    } else if (exportState.kind === "cancelled" || exportState.kind === "error") {
      openPowerBiAfterExportRef.current = false;
    }
  }, [exportState]);

  return (
    <>
      <header className="phase-header phase-header--compact">
        <div>
          <h2>Valida y crea una copia</h2>
          <h3 className="phase-file">{dataset.fileName}</h3>
          <p className="phase-meta">
            {dataset.rowCount.toLocaleString()} filas · {dataset.columnCount.toLocaleString()} columnas · {formatFileSize(dataset.fileSizeBytes)}
          </p>
        </div>
      </header>
      <section className="quality-contract" aria-labelledby="quality-contract-title">
        <div className="quality-contract__header">
          <div>
            <h3 id="quality-contract-title">Elige cómo validar la entrega</h3>
          </div>
        </div>

        <fieldset className="delivery-route">
          <legend className="visually-hidden">Ruta de entrega</legend>
          <label data-selected={contract.kind === "with_contract" || undefined}>
            <input
              type="radio"
              name="delivery-validation-route"
              checked={contract.kind === "with_contract"}
              disabled={busy || dataset.columns.length === 0}
              onChange={() => {
                if (contract.kind === "with_contract") return;
                setRulesEditorOpen(true);
                const next = withAddedRule(rules, dataset);
                if (next) changeRules(next);
              }}
            />
            <span>
              <strong>Validar calidad</strong>
              <small>Recomendado</small>
            </span>
          </label>
          <label data-selected={contract.kind === "without_contract" || undefined}>
            <input
              type="radio"
              name="delivery-validation-route"
              checked={contract.kind === "without_contract"}
              disabled={busy}
              onChange={() => contract.kind !== "without_contract" && changeRules([])}
            />
            <span>
              <strong>Exportar sin validar</strong>
            </span>
          </label>
        </fieldset>

        {contract.kind === "without_contract" && suggestedRules.length > 0 && (
          <div className="quality-suggestions" role="group" aria-labelledby="quality-suggestions-title">
            <h4 id="quality-suggestions-title">
              {suggestedRules.length === 1
                ? "Columnia propone 1 comprobación"
                : `Columnia propone ${suggestedRules.length} comprobaciones`}
            </h4>
            <ul>
              {suggestedRules.map((rule, index) => (
                <li key={`${rule.kind}:${rule.column}`}>
                  <label>
                    <input
                      type="checkbox"
                      checked={!excludedSuggestions.includes(index)}
                      disabled={busy}
                      onChange={() => toggleSuggestion(index)}
                    />
                    {summarizeQualityRule(rule)}
                  </label>
                </li>
              ))}
            </ul>
            <button
              type="button"
              className="secondary-action"
              disabled={busy || chosenSuggestions.length === 0}
              onClick={() => changeRules(chosenSuggestions)}
            >
              {chosenSuggestions.length === 1 ? "Usar esta comprobación" : `Usar estas ${chosenSuggestions.length} comprobaciones`}
            </button>
          </div>
        )}

        {contract.kind === "with_contract" ? (
          <>
            <div className="quality-contract__summary" aria-labelledby="quality-rules-summary-title">
              <div>
                <h4 id="quality-rules-summary-title">Qué se exige</h4>
                <p>{rules.length === 1 ? "1 regla se comprobará antes de guardar la copia." : `${rules.length} reglas se comprobarán antes de guardar la copia.`}</p>
              </div>
              {/* While editing, the editor itself lists every rule. */}
              {!rulesEditorOpen && (
                <ul>
                  {rules.map((rule, index) => <li key={index}>{summarizeQualityRule(rule)}</li>)}
                </ul>
              )}
              <button
                type="button"
                className="secondary-action"
                aria-expanded={rulesEditorOpen}
                aria-controls="quality-rules-editor"
                onClick={() => setRulesEditorOpen((current) => !current)}
                disabled={busy}
              >
                {rulesEditorOpen ? "Cerrar edición" : "Editar reglas"}
              </button>
            </div>
            {rulesEditorOpen && <div id="quality-rules-editor" className="quality-rules-editor">
              <QualityRulesEditor rules={rules} dataset={dataset} busy={busy} validationErrorRuleIndex={validationErrorRuleIndex} onRulesChange={changeRules} />
            </div>}
            <details className="quality-contract__utilities">
              <summary>Importar o guardar reglas</summary>
              <div className="quality-contract__commandbar" aria-label="Acciones del contrato">
              <div className="quality-contract__management">
                <button type="button" aria-label="Importar contrato" onClick={() => void importQualityRules()} disabled={busy}>Importar</button>
                <button type="button" aria-label="Guardar contrato" onClick={() => void saveQualityContract()}
                  disabled={busy || validationError !== null}>Guardar</button>
              </div>
              </div>
            </details>
            {validationError && (
              <p id="quality-rule-validation-error" className="notice notice--error" role="alert">
                {validationError}
              </p>
            )}
            {migrationState.kind === "working" && <p className="notice" role="status">Importando y comprobando compatibilidad…</p>}
            {migrationState.kind === "ready" && (
              <div className="notice quality-migration-result" role="status" aria-live="polite">
                <strong>Importación revisada</strong>
                <span>
                  {migrationState.result.report.convertedItems} reglas importadas · {migrationState.result.report.omittedItems} omitidas de {migrationState.result.report.totalItems} · origen {migrationState.result.sourceFormat === "columnia" ? "Columnia" : "legado"}
                  {migrationState.result.sourceVersion ? ` v${migrationState.result.sourceVersion}` : " sin versión"}
                </span>
                {migrationState.result.report.artifactSha256 && (
                  <small>SHA-256 del artefacto: {migrationState.result.report.artifactSha256}</small>
                )}
                {migrationState.result.report.manualActions.length > 0 && (
                  <div>
                    <strong>Acciones manuales</strong>
                    <ul>
                      {migrationState.result.report.manualActions.map((action) => <li key={action}>{action}</li>)}
                    </ul>
                  </div>
                )}
                {migrationState.result.warnings.length > 0 && (
                  <ul>
                    {migrationState.result.warnings.map((warning, index) => (
                      <li key={`${warning.ruleIndex}-${index}`}>
                        <strong>{warning.severity === "omitted" ? "Omitida" : "Advertencia"} · regla {warning.ruleIndex} · {warning.sourceKind}</strong>: {warning.message}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            )}
            {migrationState.kind === "error" && <p className="notice notice--error" role="alert">No se pudo importar el contrato: {migrationState.message}</p>}
            {qualityFileState.kind === "working" && <p className="notice" role="status">Guardando contrato versionado…</p>}
            {qualityFileState.kind === "ready" && (
              <p className="notice" role="status" aria-live="polite">
                Contrato guardado · formato Columnia v{qualityFileState.document.version} · {qualityFileState.document.rules.length} reglas.
              </p>
            )}
            {qualityFileState.kind === "error" && <p className="notice notice--error" role="alert">No se pudo guardar el contrato: {qualityFileState.message}</p>}
          </>
        ) : null}

        {contract.gate.kind === "loading" && (
          <div className="quality-contract__validation-progress">
            <p className="notice" role="status">
              {qualityValidationCancellationPending ? "Esperando que termine la validación…" : "Validando contrato localmente…"}
            </p>
            <button
              type="button"
              className="secondary-action"
              onClick={() => void cancelQualityValidation()}
              disabled={qualityValidationCancellationPending}
            >
              {qualityValidationCancellationPending ? "Esperando cancelación…" : "Cancelar validación"}
            </button>
            {qualityValidationCancellationError && (
              <p className="notice notice--error" role="alert">
                No se pudo cancelar la validación: {qualityValidationCancellationError}
              </p>
            )}
          </div>
        )}
        {contract.gate.kind === "error" && <p className="notice notice--error" role="alert">No se pudo validar: {contract.gate.message}</p>}
        {(contract.gate.kind === "ready" || contract.gate.kind === "stale") && (
          <div className={`quality-gate quality-gate--${contract.gate.result.passed ? "passed" : "failed"}`} role="status">
            <strong>{contract.gate.kind === "stale"
              ? "Resultado desactualizado"
              : contract.gate.result.passed ? "Contrato aprobado" : "Contrato fallido"}</strong>
            <span>{contract.gate.result.failedRules} de {contract.gate.result.totalRules} reglas fallaron · {contract.gate.result.rowCount.toLocaleString()} filas comprobadas</span>
            {contract.gate.result.failedRules > 0 && (
              <div className="quality-gate__issues" aria-label="Resumen de problemas de calidad">
                <strong>Problemas detectados</strong>
                <ul>
                  {contract.gate.result.rules.map((result, index) => {
                    if (result.passed) return null;
                    const guidance = QUALITY_ISSUE_GUIDANCE[result.kind];
                    return (
                      <li key={index}>
                        <strong>{guidance.title} · {result.column === QUALITY_DATASET_COLUMN ? "Dataset" : result.column}</strong>
                        <span>
                          {result.invalidCount.toLocaleString()} incumplimientos entre {result.checkedCount.toLocaleString()} elementos evaluados
                          ({formatPercent(result.invalidPct, 2)}). {guidance.nextStep}
                        </span>
                        {contract.gate.kind === "ready" && rules[index] && (
                          <button type="button" onClick={() => editQualityRule(index)}>
                            Revisar regla {index + 1}
                          </button>
                        )}
                      </li>
                    );
                  })}
                </ul>
              </div>
            )}
          </div>
        )}
      </section>
      <section className="export-panel" aria-labelledby="export-title">
        <div>
          <h3 id="export-title">Exportar dataset activo</h3>
        </div>
        <div className="export-controls">
          <label className="export-format">
            Formato
            <select
              aria-label="Formato de exportación"
              value={selectedExportFormat}
              onChange={(event) => changeExportFormat(event.target.value as ExportFormat)}
              disabled={busy}
            >
              <option value="csv">CSV</option>
              <option value="json">JSON</option>
              <option value="parquet">Parquet</option>
              <option value="sql">SQL</option>
              <option value="excel">Excel</option>
              <option value="sqlite">SQLite</option>
              <option value="bundle">Paquete ZIP (dataset + diccionario + receta + calidad)</option>
              <optgroup label="Bases de datos mediante ODBC">
                <option value="postgresql">PostgreSQL</option>
                <option value="mysql">MySQL</option>
                <option value="sqlserver">SQL Server</option>
              </optgroup>
            </select>
          </label>
          <label className="privacy-mode">
            Protección de datos personales
            <select
              aria-label="Protección de datos personales"
              aria-describedby={selectedPrivacyMode === "hash" ? "privacy-mode-note" : undefined}
              value={selectedPrivacyMode}
              onChange={(event) => changePrivacyMode(event.target.value as PrivacyMode)}
              disabled={busy}
            >
              <option value="none">Sin protección adicional</option>
              <option value="mask">Enmascarar columnas detectadas</option>
              <option value="hash">Seudonimizar columnas detectadas (hash con sal)</option>
            </select>
            {selectedPrivacyMode === "hash" && (
              <small id="privacy-mode-note" className="privacy-mode__note">
                Cada valor se sustituye por un hash SHA-256 con una sal nueva en cada exportación: no se puede adivinar probando valores, pero tampoco se pueden cruzar dos exportaciones. Es seudonimización, no anonimización; revisa el archivo antes de compartirlo.
              </small>
            )}
          </label>
          {personalDataColumns.length > 0 && (
            <div className="notice privacy-signals" role="note" aria-labelledby="privacy-signals-title">
              <p id="privacy-signals-title">
                <strong>Datos personales detectados</strong> en {personalDataColumns.length === 1 ? "1 columna" : `${personalDataColumns.length} columnas`}: {personalDataColumns.join(", ")}.
              </p>
              {exportsUnprotectedPersonalData ? (
                <label className="quality-rule__check">
                  <input
                    type="checkbox"
                    checked={confirmedUnprotectedKey === personalDataKey}
                    disabled={busy}
                    onChange={(event) => setConfirmedUnprotectedKey(event.target.checked ? personalDataKey : null)}
                  />
                  Confirmo que exporto estas columnas sin enmascarar ni aplicar hash
                </label>
              ) : (
                <p>La protección elegida se aplicará a estas columnas en la copia.</p>
              )}
            </div>
          )}
          {isDatabaseExportFormat(selectedExportFormat) && (
            <fieldset className="database-target">
              <legend>Destino remoto · {exportFormatLabel}</legend>
              <p>
                Usa el controlador ODBC correspondiente. La cadena y la contraseña solo viven durante esta sesión y no se guardan en el proyecto. El esquema de destino es opcional.
              </p>
              <label>
                Cadena de conexión ODBC
                <input
                  aria-label="Cadena de conexión ODBC"
                  type="password"
                  autoComplete="off"
                  value={databaseTarget.connectionString}
                  aria-invalid={invalidDatabaseTargetField === "connectionString" || undefined}
                  aria-describedby={invalidDatabaseTargetField === "connectionString" ? "database-target-validation-error" : undefined}
                  onChange={(event) => changeDatabaseTarget({ connectionString: event.target.value })}
                  disabled={busy}
                  placeholder="Driver={...};Server=...;Database=...;Uid=...;Pwd=..."
                />
              </label>
              <div className="database-target__grid">
                <label>
                  Esquema de destino
                  <input
                    value={databaseTarget.schema}
                    aria-invalid={invalidDatabaseTargetField === "schema" || undefined}
                    aria-describedby={invalidDatabaseTargetField === "schema" ? "database-target-validation-error" : undefined}
                    onChange={(event) => changeDatabaseTarget({ schema: event.target.value })}
                    disabled={busy}
                  />
                </label>
                <label>
                  Tabla
                  <input
                    value={databaseTarget.table}
                    aria-invalid={invalidDatabaseTargetField === "table" || undefined}
                    aria-describedby={invalidDatabaseTargetField === "table" ? "database-target-validation-error" : undefined}
                    onChange={(event) => changeDatabaseTarget({ table: event.target.value })}
                    disabled={busy}
                  />
                </label>
                <label>
                  Política de tabla
                  <select
                    aria-label="Política de tabla"
                    value={databaseTarget.tablePolicy}
                    onChange={(event) => changeDatabaseTarget({ tablePolicy: event.target.value as DatabaseTarget["tablePolicy"] })}
                    disabled={busy}
                  >
                    <option value="create_only">Crear; fallar si existe</option>
                    <option value="append">Añadir a tabla existente</option>
                    <option value="replace" disabled={databaseTarget.kind === "mysql"}>Reemplazar tabla explícitamente</option>
                  </select>
                </label>
              </div>
              {databaseTargetError && (
                <p
                  id={invalidDatabaseTargetField ? "database-target-validation-error" : undefined}
                  className="notice notice--error"
                  role="alert"
                >
                  {databaseTargetError}
                </p>
              )}
              <button
                className="secondary-action"
                type="button"
                onClick={() => void preflightDatabaseTarget()}
                disabled={busy || databaseTargetError !== null}
              >
                {databasePreflightState.kind === "working" ? "Analizando compatibilidad…" : "Analizar compatibilidad"}
              </button>
              {databasePreflightState.kind === "working" && (
                <div className="database-preflight__progress">
                  <p className="notice" role="status">
                    {databasePreflightCancellationPending
                      ? "Esperando que termine el análisis…"
                      : "Analizando el dataset y el esquema del destino. No se han escrito datos."}
                  </p>
                  <button
                    className="secondary-action"
                    type="button"
                    onClick={() => void cancelDatabasePreflight()}
                    disabled={databasePreflightCancellationPending}
                  >
                    {databasePreflightCancellationPending ? "Esperando cancelación…" : "Cancelar análisis"}
                  </button>
                  {databasePreflightCancellationError && (
                    <p className="notice notice--error" role="alert">
                      No se pudo cancelar el análisis: {databasePreflightCancellationError}
                    </p>
                  )}
                </div>
              )}
              {activePreflight && (
                <div className={activePreflight.ready ? "notice notice--success" : "notice notice--error"} role={activePreflight.ready ? "status" : "alert"}>
                  <p>{activePreflight.ready
                    ? `Preflight completo para ${activePreflight.schema ? `${activePreflight.schema}.` : ""}${activePreflight.table}. No se ha escrito ningún dato.`
                    : "Preflight bloqueado. Resuelve los problemas antes de exportar; no se ha escrito ningún dato."}</p>
                  {activePreflight.issues.length > 0 && (
                    <ul>
                      {activePreflight.issues.map((issue, index) => (
                        <li key={`${issue.category}-${issue.column ?? "dataset"}-${index}`}>
                          <strong>{issue.severity === "blocking" ? "Bloqueo" : issue.severity === "warning" ? "Advertencia" : "Información"}{issue.column ? ` · ${issue.column}` : ""}:</strong> {issue.message}
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              )}
              {databasePreflightState.kind === "error" && (
                <p className="notice notice--error" role="alert">{databasePreflightState.message}</p>
              )}
              {!activePreflight && databasePreflightState.kind !== "error" && !databaseTargetError && (
                <p className="export-requirement" role="note">Analiza el esquema y los datos para habilitar la entrega remota. Este paso no escribe en el destino.</p>
              )}
            </fieldset>
          )}
          {contract.kind === "without_contract" && (
            <label className="export-confirmation">
              <input
                type="checkbox"
                checked={contract.confirmation === "confirmed"}
                disabled={busy}
                onChange={(event) => onContractAction({
                  kind: "confirmation_changed",
                  confirmation: event.target.checked ? "confirmed" : "required",
                })}
              />
              Confirmo que quiero exportar sin validar la calidad
            </label>
          )}
          <div className="export-actions-row">
            <button
              className="primary-action export-action"
              type="button"
              onClick={() => {
                openPowerBiAfterExportRef.current = false;
                void requestExport(selectedExportFormat);
              }}
              disabled={busy || validationError !== null || needsUnvalidatedConfirmation || needsPersonalDataConfirmation || !databaseReady || exceedsExcel}
            >
              {contract.kind === "with_contract" && !gatePassed
                ? `Validar y exportar ${exportFormatLabel}`
                : `Exportar ${exportFormatLabel}`}
            </button>
            {(selectedExportFormat === "csv" || selectedExportFormat === "excel" || selectedExportFormat === "parquet") && (
              <button
                className="secondary-action"
                type="button"
                onClick={() => {
                  openPowerBiAfterExportRef.current = true;
                  void requestExport(selectedExportFormat);
                }}
                disabled={busy || validationError !== null || needsUnvalidatedConfirmation || needsPersonalDataConfirmation || exceedsExcel}
              >
                Exportar y abrir en Power BI
              </button>
            )}
          </div>
          <DeliveryPresets
            dataset={dataset}
            currentPreset={(name) => ({
              version: 1,
              name,
              format: selectedExportFormat,
              selectedColumns: dataset.columns.map((column) => column.name),
              privacyMode: selectedPrivacyMode,
              ...(isDatabaseExportFormat(selectedExportFormat) ? {
                databaseTarget: {
                  kind: databaseTarget.kind,
                  schema: databaseTarget.schema,
                  table: databaseTarget.table,
                  tablePolicy: databaseTarget.tablePolicy,
                },
              } : {}),
            })}
            onApply={applyDeliveryPreset}
          />
        </div>
        {exceedsExcel && (
          <div className="notice notice--warning" role="alert">
            <p><strong>Este dataset no cabe en Excel.</strong> CSV y Parquet lo conservan entero.</p>
            <ul>{excelLimitIssues.map((issue) => <li key={issue}>{issue}</li>)}</ul>
            <button type="button" className="secondary-action" disabled={busy} onClick={() => changeExportFormat("csv")}>
              Exportar como CSV
            </button>
          </div>
        )}
        {selectedExportFormat === "bundle" && (
          <p className="export-requirement" role="note">
            {recipeDraft
              ? "Este paquete incluirá delivery-summary.md, recipe.json con la receta actual validada y sus referencias y hashes en manifest.json."
              : "No hay una receta activa para incluir; el paquete contendrá dataset.csv, dictionary.json, delivery-summary.md y manifest.json."}
          </p>
        )}
        {contract.kind === "with_contract" && !gatePassed && !validationError && (
          <p className="export-requirement">Las reglas se comprobarán antes de crear la copia.</p>
        )}
      </section>
      {exportState.kind === "loading" && (
        <OperationProgressView
          progress={exportState.progress}
          cancellation={exportState.cancellation === "requested"
            ? { kind: "requested" }
            : {
                kind: "available",
                onCancel: onCancelExport,
                ...(exportState.cancellationError ? { error: exportState.cancellationError } : {}),
              }}
        />
      )}
      {exportState.kind === "success" && (
        <DeliveryResult
          result={exportState.result}
          autoOpenPowerBi={autoOpenResult === exportState.result}
          contract={contract}
          recipeDraft={recipeDraft}
          preparationChanges={preparationChanges}
          approvedQualitySummary={approvedQualitySummary}
        />
      )}
      {exportState.kind === "cancelled" && (
        <div className="notice" role="status">
          <strong>Exportación cancelada.</strong> No se publicó una salida; el dataset preparado sigue disponible para reintentar.
        </div>
      )}
      {exportState.kind === "error" && (
        <div className="notice notice--error" role="alert">
          <strong>No se pudo crear la copia.</strong> {exportState.message}
          <p>El dataset preparado sigue disponible. Revisa el destino e inténtalo de nuevo.</p>
        </div>
      )}
    </>
  );
}
