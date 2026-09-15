import { useEffect, useRef, useState } from "react";

import {
  QUALITY_DATASET_COLUMN,
  deleteDeliveryPreset,
  listDeliveryPresets,
  openDeliveryPreset,
  openLastExport,
  preflightDatabaseExport,
  pickQualityRulesMigration,
  saveDeliveryPreset,
  saveQualityRulesDocument,
  validateQualityRules,
  type DatabaseTarget,
  type DeliveryPreset,
  type DeliveryPresetSummary,
  type DatasetPreview,
  type ExportFormat,
  type PrivacyMode,
  type RemoteExportPreflight,
  type QualityMigrationResult,
  type QualityRulesDocument,
  type QualityComparison,
  type QualityAggregate,
  type QualityMonotonicDirection,
  type QualityRule,
  type QualityRuleKind,
  type SavedRecipe,
} from "../../bridge";
import { OperationProgressView } from "../../components/OperationProgressView";
import { DatasetMetrics, formatFileSize } from "./DatasetMetrics";
import {
  MAX_QUALITY_RULES,
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
    title: "Distribución fuera de tolerancia",
    nextStep: "Verifica que la línea base y la población actual sean comparables antes de ajustar el umbral.",
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
  distribution_drift: "debe permanecer dentro de la variación permitida",
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
  const [databaseTarget, setDatabaseTarget] = useState<DatabaseTarget>(INITIAL_DATABASE_TARGET);
  const [databasePreflightState, setDatabasePreflightState] = useState<
    | { kind: "idle" }
    | { kind: "working" }
    | { kind: "ready"; result: RemoteExportPreflight; fingerprint: string }
    | { kind: "error"; message: string }
  >({ kind: "idle" });
  const databaseRequestGeneration = useRef(0);
  const databaseTargetFingerprint = JSON.stringify({
    target: databaseTarget,
    privacyMode: selectedPrivacyMode,
    dataset: [dataset.fileName, dataset.fileSizeBytes, dataset.rowCount,
      dataset.columns.map((column) => [column.name, column.dataType])],
  });
  const databaseTargetFingerprintRef = useRef(databaseTargetFingerprint);
  databaseTargetFingerprintRef.current = databaseTargetFingerprint;
  const [presets, setPresets] = useState<DeliveryPresetSummary[]>([]);
  const [presetsLoaded, setPresetsLoaded] = useState(false);
  const [presetsLoading, setPresetsLoading] = useState(false);
  const [presetsError, setPresetsError] = useState<string | null>(null);
  const [presetName, setPresetName] = useState("");
  const [selectedPresetId, setSelectedPresetId] = useState("");
  const [openedPreset, setOpenedPreset] = useState<DeliveryPreset | null>(null);
  const [presetNotice, setPresetNotice] = useState<string | null>(null);
  const [presetWorking, setPresetWorking] = useState(false);
  useEffect(() => {
    const kind = databaseKindForExportFormat(selectedExportFormat);
    setDatabaseTarget({ ...INITIAL_DATABASE_TARGET, ...(kind ? { kind } : {}) });
    setDatabasePreflightState({ kind: "idle" });
    databaseRequestGeneration.current += 1;
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
  const [openOutputState, setOpenOutputState] = useState<
    "idle" | "working" | "opened" | "error"
  >("idle");
  const [rulesEditorOpen, setRulesEditorOpen] = useState(false);
  const [pendingRuleFocus, setPendingRuleFocus] = useState<number | null>(null);
  const rules = contract.kind === "with_contract" ? contract.rules : [];
  const validationError = validateQualityRuleDraft(rules, dataset);
  const gatePassed = contract.gate.kind === "ready" && contract.gate.result.passed;
  const needsUnvalidatedConfirmation = contract.kind === "without_contract"
    && contract.confirmation !== "confirmed";
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
  const openedPresetSchemaMatches = openedPreset !== null
    && JSON.stringify(openedPreset.selectedColumns) === JSON.stringify(dataset.columns.map((column) => column.name));
  const busy = exportState.kind === "loading"
    || contract.gate.kind === "loading"
    || migrationState.kind === "working"
    || qualityFileState.kind === "working";
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
    setQualityFileState({ kind: "idle" });
    onContractAction({ kind: "rules_changed", rules: nextRules });
  }

  function addRule() {
    if (rules.length >= MAX_QUALITY_RULES || dataset.columns.length === 0) return;
    changeRules([...rules, {
      column: dataset.columns[0].name,
      kind: "not_null",
      maxInvalid: 0,
    }]);
  }

  function updateRule(index: number, update: Partial<QualityRule>) {
    changeRules(rules.map((rule, ruleIndex) =>
      ruleIndex === index ? { ...rule, ...update } : rule));
  }

  function changeRuleKind(index: number, kind: QualityRuleKind) {
    const rule = rules[index];
    const firstColumn = dataset.columns[0]?.name ?? "";
    const isSchemaRule = kind === "schema_contract";
    const isReferentialRule = kind === "referential_integrity";
    const isMonotonicRule = kind === "monotonic";
    const isAggregateCheckRule = kind === "aggregate_check";
    const isAggregateReconciliationRule = kind === "aggregate_reconciliation";
    const isDistributionDriftRule = kind === "distribution_drift";
    const isAggregateRule = isAggregateCheckRule || isAggregateReconciliationRule;
    const usesMultipleColumns = kind === "unique_together"
      || kind === "column_compare"
      || isReferentialRule
      || isAggregateReconciliationRule;
    const minimumColumns = isReferentialRule ? 1 : 2;
    const retainedColumns = rule.columns?.filter((name) =>
      dataset.columns.some((column) => column.name === name));
    const nextColumns = usesMultipleColumns
      ? retainedColumns && retainedColumns.length >= minimumColumns
        ? retainedColumns
        : dataset.columns.slice(0, minimumColumns).map((column) => column.name)
      : undefined;
    const nextColumn = kind === "row_count" || isSchemaRule
      ? QUALITY_DATASET_COLUMN
      : usesMultipleColumns
        ? nextColumns?.[0] ?? firstColumn
        : rule.column === QUALITY_DATASET_COLUMN ? firstColumn : rule.column;
    const nextConditionColumn = rule.when?.column && dataset.columns.some((column) => column.name === rule.when?.column)
      ? rule.when.column
      : firstColumn;
    const nextThenColumn = rule.then?.column && dataset.columns.some((column) => column.name === rule.then?.column)
      ? rule.then.column
      : nextColumn === QUALITY_DATASET_COLUMN ? firstColumn : nextColumn;
    updateRule(index, {
      kind,
      column: nextColumn,
      min: undefined,
      max: undefined,
      values: kind === "allowed_values" ? rule.values ?? [] : undefined,
      referenceValues: isReferentialRule ? rule.referenceValues ?? [] : undefined,
      baseline: isDistributionDriftRule ? rule.baseline ?? [] : undefined,
      direction: isMonotonicRule ? rule.direction ?? "increasing" : undefined,
      expected: isAggregateCheckRule ? rule.expected ?? 0 : undefined,
      aggregate: isAggregateCheckRule ? rule.aggregate ?? "sum" : undefined,
      toleranceAbs: isAggregateRule ? rule.toleranceAbs : undefined,
      toleranceRel: isAggregateRule ? rule.toleranceRel : undefined,
      threshold: isDistributionDriftRule ? rule.threshold : undefined,
      pattern: kind === "regex" ? rule.pattern ?? "" : undefined,
      dtype: kind === "dtype" ? rule.dtype ?? "string" : undefined,
      columns: isSchemaRule
        ? rule.columns ?? dataset.columns.map((column) => column.name)
        : usesMultipleColumns ? nextColumns : undefined,
      operator: kind === "column_compare" ? rule.operator ?? "eq" : undefined,
      minDate: kind === "date_range" ? rule.minDate : undefined,
      maxDate: kind === "date_range" ? rule.maxDate : undefined,
      when: kind === "conditional"
        ? rule.when ?? { column: nextConditionColumn, operator: "eq", value: "" }
        : undefined,
      then: kind === "conditional"
        ? rule.then ?? { column: nextThenColumn, kind: "not_null", maxInvalid: 0 }
        : undefined,
      allowAdditional: isSchemaRule ? rule.allowAdditional ?? true : undefined,
      requiredOrder: isSchemaRule ? rule.requiredOrder : undefined,
    });
  }

  function updateConditionalThen(index: number, update: Partial<QualityRule>) {
    const rule = rules[index];
    const fallbackColumn = rule.column === QUALITY_DATASET_COLUMN
      ? dataset.columns[0]?.name ?? ""
      : rule.column;
    const then = rule.then ?? { column: fallbackColumn, kind: "not_null" as const, maxInvalid: 0 };
    updateRule(index, { then: { ...then, ...update } });
  }

  function updateTogetherColumns(index: number, name: string, checked: boolean) {
    const current = rules[index].columns ?? [];
    const next = checked
      ? [...current, name]
      : current.filter((column) => column !== name);
    updateRule(index, { columns: next, column: next[0] ?? dataset.columns[0]?.name ?? "" });
  }

  function updateComparisonColumn(index: number, position: 0 | 1, name: string) {
    const rule = rules[index];
    const current = rule.columns ?? dataset.columns.slice(0, 2).map((column) => column.name);
    const next = [...current];
    next[position] = name;
    updateRule(index, {
      columns: next,
      column: next[0] ?? dataset.columns[0]?.name ?? "",
    });
  }

  function updateAggregateColumn(index: number, position: 0 | 1, name: string) {
    const rule = rules[index];
    const current = rule.columns ?? dataset.columns.slice(0, 2).map((column) => column.name);
    const next = [...current];
    next[position] = name;
    updateRule(index, {
      columns: next,
      column: next[0] ?? dataset.columns[0]?.name ?? "",
    });
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

  async function requestExport(format: ExportFormat) {
    setOpenOutputState("idle");
    if (isDatabaseExportFormat(format)
      && (databaseTargetError !== null
        || databasePreflightState.kind !== "ready"
        || databasePreflightState.fingerprint !== databaseTargetFingerprint
        || !databasePreflightState.result.ready)) return;
    const databaseOptions = isDatabaseExportFormat(format) ? { databaseTarget } : {};
    if (contract.kind === "with_contract") {
      if (validationError) return;
      if (!gatePassed) {
        onContractAction({ kind: "gate_changed", gate: { kind: "loading" } });
        try {
          const result = await validateQualityRules(contract.rules);
          onContractAction({ kind: "gate_changed", gate: { kind: "ready", result } });
          if (!result.passed) return;
        } catch (error: unknown) {
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
      onExport({ format, privacyMode: selectedPrivacyMode, ...databaseOptions, validation: { kind: "contract", rules: contract.rules } });
    } else if (contract.confirmation === "confirmed") {
      onExport({ format, privacyMode: selectedPrivacyMode, ...databaseOptions, validation: { kind: "explicitly_unvalidated" } });
    }
  }

  function changeExportFormat(format: ExportFormat) {
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
    setDatabaseTarget((current) => ({ ...current, ...update }));
    setDatabasePreflightState({ kind: "idle" });
    databaseRequestGeneration.current += 1;
  }

  async function preflightDatabaseTarget() {
    if (databaseTargetError) return;
    const requestGeneration = databaseRequestGeneration.current;
    const requestFingerprint = databaseTargetFingerprint;
    const requestedTarget = databaseTarget;
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
      setDatabasePreflightState({ kind: "ready", result, fingerprint: requestFingerprint });
    } catch (error: unknown) {
      if (requestGeneration !== databaseRequestGeneration.current
        || requestFingerprint !== databaseTargetFingerprintRef.current) return;
      setDatabasePreflightState({
        kind: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  function changePrivacyMode(mode: PrivacyMode) {
    setLocalPrivacyMode(mode);
    onPrivacyModeChange?.(mode);
    setDatabasePreflightState({ kind: "idle" });
    databaseRequestGeneration.current += 1;
  }

  async function refreshDeliveryPresets() {
    setPresetsLoading(true);
    setPresetsError(null);
    try {
      setPresets(await listDeliveryPresets());
      setPresetsLoaded(true);
    } catch (error: unknown) {
      setPresetsError(error instanceof Error ? error.message : String(error));
    } finally {
      setPresetsLoading(false);
    }
  }

  async function saveCurrentDeliveryPreset() {
    const trimmedName = presetName.trim();
    if (!trimmedName || presetWorking) return;
    const preset: DeliveryPreset = {
      version: 1,
      name: trimmedName,
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
    };
    setPresetWorking(true);
    setPresetNotice(null);
    setPresetsError(null);
    try {
      const summary = await saveDeliveryPreset(selectedPresetId || null, preset);
      setPresets((current) => [summary, ...current.filter((item) => item.id !== summary.id)]);
      setPresetsLoaded(true);
      setSelectedPresetId(summary.id);
      setOpenedPreset(preset);
      setPresetNotice("Preset guardado localmente. Las credenciales de conexión no se almacenan.");
    } catch (error: unknown) {
      setPresetsError(error instanceof Error ? error.message : String(error));
    } finally {
      setPresetWorking(false);
    }
  }

  async function loadSelectedDeliveryPreset() {
    if (!selectedPresetId || presetWorking) return;
    setPresetWorking(true);
    setPresetNotice(null);
    setPresetsError(null);
    try {
      const preset = await openDeliveryPreset(selectedPresetId);
      setOpenedPreset(preset);
      setPresetName(preset.name);
      setPresetNotice(null);
    } catch (error: unknown) {
      setPresetsError(error instanceof Error ? error.message : String(error));
    } finally {
      setPresetWorking(false);
    }
  }

  function applyOpenedDeliveryPreset() {
    if (!openedPreset) return;
    const currentColumns = dataset.columns.map((column) => column.name);
    const schemaMatches = JSON.stringify(openedPreset.selectedColumns) === JSON.stringify(currentColumns);
    if (!schemaMatches) {
      const missing = openedPreset.selectedColumns.filter((column) => !currentColumns.includes(column));
      const added = currentColumns.filter((column) => !openedPreset.selectedColumns.includes(column));
      setPresetNotice(`Esquema distinto: ${missing.length} columna(s) guardada(s) no aparecen y ${added.length} columna(s) nueva(s). Se aplicará la configuración a todas las columnas actuales.`);
    } else {
      setPresetNotice("Preset verificado contra el esquema actual.");
    }
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
        setPresetNotice("El preset proponía reemplazar la tabla. Se cargó Crear sin reemplazar; elige Reemplazar explícitamente y vuelve a analizar si quieres autorizarlo en esta sesión.");
      } else {
        setPresetNotice("Preset aplicado. Reingresa la conexión de esta sesión y ejecuta el preflight antes de escribir.");
      }
    } else {
      setDatabaseTarget({ ...INITIAL_DATABASE_TARGET, ...(databaseKindForExportFormat(openedPreset.format)
        ? { kind: databaseKindForExportFormat(openedPreset.format)! }
        : {}) });
      setDatabasePreflightState({ kind: "idle" });
      databaseRequestGeneration.current += 1;
    }
  }

  async function deleteSelectedDeliveryPreset() {
    if (!selectedPresetId || presetWorking) return;
    setPresetWorking(true);
    setPresetsError(null);
    setPresetNotice(null);
    try {
      await deleteDeliveryPreset(selectedPresetId);
      setPresets((current) => current.filter((item) => item.id !== selectedPresetId));
      setSelectedPresetId("");
      setOpenedPreset(null);
      setPresetName("");
      setPresetNotice("Preset eliminado del catálogo local.");
    } catch (error: unknown) {
      setPresetsError(error instanceof Error ? error.message : String(error));
    } finally {
      setPresetWorking(false);
    }
  }

  async function revealLastExport() {
    if (openOutputState === "working") return;
    setOpenOutputState("working");
    try {
      await openLastExport();
      setOpenOutputState("opened");
    } catch {
      setOpenOutputState("error");
    }
  }

  return (
    <>
      <header className="phase-header phase-header--compact">
        <div>
          <p className="eyebrow">Entregar · Exportación local</p>
          <h2>Valida y crea una copia</h2>
          <h3 className="phase-file">{dataset.fileName}</h3>
          <p>Genera una copia del dataset preparado. El archivo original nunca se modifica.</p>
        </div>
      </header>
      <DatasetMetrics dataset={dataset} />
      <section className="quality-contract" aria-labelledby="quality-contract-title">
        <div className="quality-contract__header">
          <div>
            <p className="step">Control de entrega</p>
            <h3 id="quality-contract-title">Elige cómo validar la entrega</h3>
            <p>Usa reglas locales para comprobar el resultado o continúa sin validación de calidad.</p>
          </div>
        </div>

        <fieldset className="delivery-route">
          <legend>Ruta de entrega</legend>
          <label data-selected={contract.kind === "with_contract" || undefined}>
            <input
              type="radio"
              name="delivery-validation-route"
              checked={contract.kind === "with_contract"}
              disabled={busy || dataset.columns.length === 0}
              onChange={() => {
                if (contract.kind === "with_contract") return;
                setRulesEditorOpen(true);
                addRule();
              }}
            />
            <span>
              <strong>Validar calidad</strong>
              <small>Recomendado · define hasta {MAX_QUALITY_RULES} comprobaciones locales</small>
              <span className="delivery-route__state">
                {contract.kind === "with_contract" ? "Ruta seleccionada" : "Disponible"}
              </span>
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
              <small>Requiere una confirmación explícita durante esta sesión</small>
              <span className="delivery-route__state">
                {contract.kind === "without_contract" ? "Ruta seleccionada" : "Disponible"}
              </span>
            </span>
          </label>
        </fieldset>

        {contract.kind === "with_contract" ? (
          <>
            <div className="quality-contract__summary" aria-labelledby="quality-rules-summary-title">
              <div>
                <p className="step">Reglas activas</p>
                <h4 id="quality-rules-summary-title">Qué se exige</h4>
                <p>{rules.length === 1 ? "1 regla se comprobará antes de guardar la copia." : `${rules.length} reglas se comprobarán antes de guardar la copia.`}</p>
              </div>
              <ul>
                {rules.map((rule, index) => <li key={index}>{summarizeQualityRule(rule)}</li>)}
              </ul>
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
              <div className="quality-rules">
              {rules.map((rule, index) => {
                const hasCountTolerance = rule.maxInvalid !== undefined;
                const hasPercentageTolerance = rule.maxInvalidPct !== undefined;
                const toleranceMode = hasCountTolerance && hasPercentageTolerance
                  ? "both"
                  : hasPercentageTolerance ? "percentage" : "count";
                const isDatasetRule = rule.kind === "row_count";
                const isTogetherRule = rule.kind === "unique_together";
                const isCompareRule = rule.kind === "column_compare";
                const isReferentialRule = rule.kind === "referential_integrity";
                const isMonotonicRule = rule.kind === "monotonic";
                const isAggregateCheckRule = rule.kind === "aggregate_check";
                const isAggregateReconciliationRule = rule.kind === "aggregate_reconciliation";
                const isAggregateRule = isAggregateCheckRule || isAggregateReconciliationRule;
                const isDistributionDriftRule = rule.kind === "distribution_drift";
                const isConditionalRule = rule.kind === "conditional";
                const isSchemaRule = rule.kind === "schema_contract";
                return (
                  <fieldset
                    className="quality-rule"
                    id={`quality-rule-${index + 1}`}
                    key={index}
                    tabIndex={-1}
                    disabled={busy}
                  >
                    <legend>Regla {index + 1}</legend>
                    {!isDatasetRule && !isSchemaRule && !isTogetherRule && !isCompareRule && !isReferentialRule && !isAggregateReconciliationRule && <label>Columna
                      <select aria-label={`Columna regla ${index + 1}`} value={rule.column}
                        onChange={(event) => updateRule(index, { column: event.target.value })}>
                        {dataset.columns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}
                      </select>
                    </label>}
                    <label>Comprobación
                      <select aria-label={`Comprobación regla ${index + 1}`} value={rule.kind}
                        onChange={(event) => changeRuleKind(index, event.target.value as QualityRuleKind)}>
                        <option value="not_null">Sin nulos</option>
                        <option value="non_empty">Texto no vacío</option>
                        <option value="unique">Valores únicos</option>
                        <option value="numeric_range">Rango numérico</option>
                        <option value="allowed_values">Valores permitidos</option>
                        <option value="regex">Expresión regular</option>
                        <option value="dtype">Tipo esperado</option>
                        <option value="unique_together" disabled={dataset.columns.length < 2}>Unicidad compuesta</option>
                        <option value="column_compare" disabled={dataset.columns.length < 2}>Comparar columnas</option>
                        <option value="referential_integrity">Integridad referencial</option>
                        <option value="monotonic">Monotonicidad</option>
                        <option value="aggregate_check">Comprobación agregada</option>
                        <option value="aggregate_reconciliation" disabled={dataset.columns.length < 2}>Reconciliación agregada</option>
                        <option value="distribution_drift">Drift de distribución</option>
                        <option value="date_range">Rango de fechas</option>
                        <option value="conditional">Comprobación condicional</option>
                        <option value="schema_contract">Contrato de esquema</option>
                        <option value="row_count">Conteo de filas</option>
                      </select>
                    </label>
                    <label>Tolerancia
                      <select aria-label={`Tolerancia regla ${index + 1}`} value={toleranceMode}
                        onChange={(event) => {
                          const mode = event.target.value;
                          updateRule(index, mode === "both"
                            ? { maxInvalid: rule.maxInvalid ?? 0, maxInvalidPct: rule.maxInvalidPct ?? 0 }
                            : mode === "percentage"
                              ? { maxInvalid: undefined, maxInvalidPct: rule.maxInvalidPct ?? 0 }
                              : { maxInvalid: rule.maxInvalid ?? 0, maxInvalidPct: undefined });
                        }}>
                        <option value="count">Máximo inválidos</option>
                        <option value="percentage">Máximo porcentaje</option>
                        <option value="both">Ambas tolerancias</option>
                      </select>
                    </label>
                    {(toleranceMode === "count" || toleranceMode === "both") && <label>Inválidos máximos
                      <input type="number" min="0" step="1"
                        aria-label={`Inválidos regla ${index + 1}`}
                        value={rule.maxInvalid ?? 0}
                        onChange={(event) => updateRule(index, { maxInvalid: Number(event.target.value) })} />
                    </label>}
                    {(toleranceMode === "percentage" || toleranceMode === "both") && <label>Porcentaje máximo
                      <input type="number" min="0" max="100" step="0.1"
                        aria-label={`Porcentaje regla ${index + 1}`}
                        value={rule.maxInvalidPct ?? 0}
                        onChange={(event) => updateRule(index, { maxInvalidPct: Number(event.target.value) })} />
                    </label>}
                    {(rule.kind === "numeric_range" || rule.kind === "row_count") && (
                      <>
                        <label>{rule.kind === "row_count" ? "Filas mínimas" : "Mínimo inclusivo"}
                          <input type="number" aria-label={`Mínimo regla ${index + 1}`}
                            value={rule.min ?? ""}
                            onChange={(event) => updateRule(index, { min: event.target.value === "" ? undefined : Number(event.target.value) })} />
                        </label>
                        <label>{rule.kind === "row_count" ? "Filas máximas" : "Máximo inclusivo"}
                          <input type="number" aria-label={`Máximo regla ${index + 1}`}
                            value={rule.max ?? ""}
                            onChange={(event) => updateRule(index, { max: event.target.value === "" ? undefined : Number(event.target.value) })} />
                        </label>
                      </>
                    )}
                    {rule.kind === "date_range" && (
                      <>
                        <label>Fecha mínima inclusiva
                          <input
                            type="date"
                            aria-label={`Fecha mínima regla ${index + 1}`}
                            value={rule.minDate ?? ""}
                            onChange={(event) => updateRule(index, {
                              minDate: event.target.value || undefined,
                            })}
                          />
                        </label>
                        <label>Fecha máxima inclusiva
                          <input
                            type="date"
                            aria-label={`Fecha máxima regla ${index + 1}`}
                            value={rule.maxDate ?? ""}
                            onChange={(event) => updateRule(index, {
                              maxDate: event.target.value || undefined,
                            })}
                          />
                        </label>
                      </>
                    )}
                    {rule.kind === "allowed_values" && (
                      <label className="quality-rule__wide">Valores permitidos
                        <textarea
                          rows={2}
                          aria-label={`Valores permitidos regla ${index + 1}`}
                          aria-describedby={`quality-values-help-${index}`}
                          value={rule.values?.join("\n") ?? ""}
                          onChange={(event) => updateRule(index, {
                            values: event.target.value.split(/\r?\n/).filter((value) => value.length > 0),
                          })}
                        />
                        <span id={`quality-values-help-${index}`} className="quality-rule__help">Un valor por línea; se compara sin transformar.</span>
                      </label>
                    )}
                    {rule.kind === "regex" && (
                      <label className="quality-rule__wide">Patrón regular
                        <input
                          type="text"
                          aria-label={`Patrón regular regla ${index + 1}`}
                          aria-describedby={`quality-regex-help-${index}`}
                          value={rule.pattern ?? ""}
                          onChange={(event) => updateRule(index, { pattern: event.target.value })}
                        />
                        <span id={`quality-regex-help-${index}`} className="quality-rule__help">Sintaxis regex compatible con Rust.</span>
                      </label>
                    )}
                    {rule.kind === "dtype" && (
                      <label>Tipo esperado
                        <select aria-label={`Tipo esperado regla ${index + 1}`} value={rule.dtype ?? "string"}
                          onChange={(event) => updateRule(index, { dtype: event.target.value })}>
                          <option value="string">Texto</option>
                          <option value="integer">Entero</option>
                          <option value="float">Decimal</option>
                          <option value="boolean">Booleano</option>
                          <option value="date">Fecha</option>
                          <option value="datetime">Fecha y hora</option>
                        </select>
                      </label>
                    )}
                    {isTogetherRule && (
                      <fieldset className="quality-rule__wide quality-rule__columns">
                        <legend>Columnas que deben ser únicas juntas</legend>
                        {dataset.columns.map((column) => (
                          <label key={column.name} className="quality-rule__check">
                            <input
                              type="checkbox"
                              aria-label={`Columna compuesta ${column.name}, regla ${index + 1}`}
                              checked={rule.columns?.includes(column.name) ?? false}
                              onChange={(event) => updateTogetherColumns(index, column.name, event.target.checked)}
                            />
                            {column.name}
                          </label>
                        ))}
                      </fieldset>
                    )}
                    {isReferentialRule && (
                      <fieldset className="quality-rule__wide quality-rule__columns">
                        <legend>Clave y valores de referencia</legend>
                        <p className="quality-rule__help">
                          Selecciona una o más columnas. Para una sola columna, escribe un valor por línea; para varias, usa un arreglo JSON por línea.
                        </p>
                        {dataset.columns.map((column) => (
                          <label key={column.name} className="quality-rule__check">
                            <input
                              type="checkbox"
                              aria-label={`Columna referencial ${column.name}, regla ${index + 1}`}
                              checked={rule.columns?.includes(column.name) ?? false}
                              onChange={(event) => updateTogetherColumns(index, column.name, event.target.checked)}
                            />
                            {column.name}
                          </label>
                        ))}
                        <label className="quality-rule__wide">Valores permitidos de referencia
                          <textarea
                            rows={3}
                            aria-label={`Valores de referencia regla ${index + 1}`}
                            aria-describedby={`quality-reference-values-help-${index}`}
                            value={rule.referenceValues?.join("\n") ?? ""}
                            onChange={(event) => updateRule(index, {
                              referenceValues: event.target.value.split(/\r?\n/).filter((value) => value.length > 0),
                            })}
                          />
                          <span id={`quality-reference-values-help-${index}`} className="quality-rule__help">
                            Clave simple: texto, número o booleano. Clave compuesta: por ejemplo [&quot;DO&quot;, 1].
                          </span>
                        </label>
                      </fieldset>
                    )}
                    {isMonotonicRule && (
                      <label>Dirección de la secuencia
                        <select
                          aria-label={`Dirección monotónica regla ${index + 1}`}
                          value={rule.direction ?? "increasing"}
                          onChange={(event) => updateRule(index, {
                            direction: event.target.value as QualityMonotonicDirection,
                          })}
                        >
                          <option value="increasing">No decreciente</option>
                          <option value="decreasing">No creciente</option>
                        </select>
                      </label>
                    )}
                    {isAggregateCheckRule && (
                      <fieldset className="quality-rule__wide quality-rule__columns">
                        <legend>Comprobación agregada</legend>
                        <label>Agregación
                          <select
                            aria-label={`Agregación regla ${index + 1}`}
                            value={rule.aggregate ?? "sum"}
                            onChange={(event) => updateRule(index, {
                              aggregate: event.target.value as QualityAggregate,
                            })}
                          >
                            <option value="count">Conteo</option>
                            <option value="sum">Suma</option>
                            <option value="min">Mínimo</option>
                            <option value="max">Máximo</option>
                          </select>
                        </label>
                        <label>Valor esperado
                          <input
                            type="number"
                            step="any"
                            aria-label={`Valor esperado agregado regla ${index + 1}`}
                            value={rule.expected ?? ""}
                            onChange={(event) => updateRule(index, {
                              expected: event.target.value === "" ? undefined : Number(event.target.value),
                              referenceValues: undefined,
                            })}
                          />
                        </label>
                        <label className="quality-rule__wide">Referencias numéricas opcionales
                          <textarea
                            rows={2}
                            aria-label={`Referencias agregadas regla ${index + 1}`}
                            value={rule.referenceValues?.join("\n") ?? ""}
                            onChange={(event) => updateRule(index, {
                              referenceValues: event.target.value.split(/\r?\n/).filter((value) => value.length > 0),
                              expected: undefined,
                            })}
                          />
                          <span className="quality-rule__help">Usa el valor esperado o estas referencias; se suman cuando la agregación es suma.</span>
                        </label>
                      </fieldset>
                    )}
                    {isDistributionDriftRule && (
                      <fieldset className="quality-rule__wide quality-rule__columns">
                        <legend>Drift de distribución</legend>
                        <label className="quality-rule__wide">Línea base numérica
                          <textarea
                            rows={3}
                            aria-label={`Línea base de distribución regla ${index + 1}`}
                            aria-describedby={`quality-drift-baseline-help-${index}`}
                            value={rule.baseline?.join("\n") ?? ""}
                            onChange={(event) => updateRule(index, {
                              baseline: event.target.value.split(/\r?\n/).filter((value) => value.length > 0),
                              referenceValues: undefined,
                            })}
                          />
                          <span id={`quality-drift-baseline-help-${index}`} className="quality-rule__help">
                            Un número por línea; se compara la media actual con la media de esta línea base.
                          </span>
                        </label>
                        <label>Umbral absoluto
                          <input
                            type="number"
                            min="0"
                            step="any"
                            aria-label={`Umbral de drift regla ${index + 1}`}
                            value={rule.threshold ?? ""}
                            onChange={(event) => updateRule(index, {
                              threshold: event.target.value === "" ? undefined : Number(event.target.value),
                            })}
                          />
                        </label>
                      </fieldset>
                    )}
                    {isAggregateReconciliationRule && (
                      <fieldset className="quality-rule__wide quality-rule__columns">
                        <legend>Reconciliar sumas</legend>
                        <label>Columna izquierda
                          <select
                            aria-label={`Columna izquierda agregada regla ${index + 1}`}
                            value={rule.columns?.[0] ?? ""}
                            onChange={(event) => updateAggregateColumn(index, 0, event.target.value)}
                          >
                            {dataset.columns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}
                          </select>
                        </label>
                        <label>Columna derecha
                          <select
                            aria-label={`Columna derecha agregada regla ${index + 1}`}
                            value={rule.columns?.[1] ?? ""}
                            onChange={(event) => updateAggregateColumn(index, 1, event.target.value)}
                          >
                            {dataset.columns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}
                          </select>
                        </label>
                      </fieldset>
                    )}
                    {isAggregateRule && (
                      <fieldset className="quality-rule__wide quality-rule__columns">
                        <legend>Tolerancia numérica del agregado</legend>
                        <label>Tolerancia absoluta
                          <input
                            type="number"
                            min="0"
                            step="any"
                            aria-label={`Tolerancia absoluta agregada regla ${index + 1}`}
                            value={rule.toleranceAbs ?? ""}
                            onChange={(event) => updateRule(index, {
                              toleranceAbs: event.target.value === "" ? undefined : Number(event.target.value),
                            })}
                          />
                        </label>
                        <label>Tolerancia relativa
                          <input
                            type="number"
                            min="0"
                            step="any"
                            aria-label={`Tolerancia relativa agregada regla ${index + 1}`}
                            value={rule.toleranceRel ?? ""}
                            onChange={(event) => updateRule(index, {
                              toleranceRel: event.target.value === "" ? undefined : Number(event.target.value),
                            })}
                          />
                        </label>
                      </fieldset>
                    )}
                    {isCompareRule && (
                      <fieldset className="quality-rule__wide quality-rule__columns">
                        <legend>Comparar columnas</legend>
                        <label>Columna izquierda
                          <select
                            aria-label={`Columna izquierda comparar regla ${index + 1}`}
                            value={rule.columns?.[0] ?? ""}
                            onChange={(event) => updateComparisonColumn(index, 0, event.target.value)}
                          >
                            {dataset.columns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}
                          </select>
                        </label>
                        <label>Operador
                          <select
                            aria-label={`Operador comparar regla ${index + 1}`}
                            value={rule.operator ?? "eq"}
                            onChange={(event) => updateRule(index, { operator: event.target.value as QualityComparison })}
                          >
                            <option value="eq">Igual a</option>
                            <option value="ne">Distinta de</option>
                            <option value="lt">Menor que</option>
                            <option value="lte">Menor o igual que</option>
                            <option value="gt">Mayor que</option>
                            <option value="gte">Mayor o igual que</option>
                          </select>
                        </label>
                        <label>Columna derecha
                          <select
                            aria-label={`Columna derecha comparar regla ${index + 1}`}
                            value={rule.columns?.[1] ?? ""}
                            onChange={(event) => updateComparisonColumn(index, 1, event.target.value)}
                          >
                            {dataset.columns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}
                          </select>
                        </label>
                      </fieldset>
                    )}
                    {isConditionalRule && (
                      <>
                        <fieldset className="quality-rule__wide quality-rule__columns">
                          <legend>Cuando se cumpla</legend>
                          <label>Columna condición
                            <select
                              aria-label={`Columna condición regla ${index + 1}`}
                              value={rule.when?.column ?? rule.column}
                              onChange={(event) => updateRule(index, {
                                when: {
                                  column: event.target.value,
                                  operator: rule.when?.operator ?? "eq",
                                  value: rule.when?.value ?? "",
                                },
                              })}
                            >
                              {dataset.columns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}
                            </select>
                          </label>
                          <label>Operador condición
                            <select
                              aria-label={`Operador condición regla ${index + 1}`}
                              value={rule.when?.operator ?? "eq"}
                              onChange={(event) => updateRule(index, {
                                when: {
                                  column: rule.when?.column ?? rule.column,
                                  operator: event.target.value as QualityComparison,
                                  value: rule.when?.value ?? "",
                                },
                              })}
                            >
                              <option value="eq">Igual a</option>
                              <option value="ne">Distinta de</option>
                              <option value="lt">Menor que</option>
                              <option value="lte">Menor o igual que</option>
                              <option value="gt">Mayor que</option>
                              <option value="gte">Mayor o igual que</option>
                            </select>
                          </label>
                          <label>Valor esperado
                            <input
                              type="text"
                              aria-label={`Valor condición regla ${index + 1}`}
                              value={rule.when?.value ?? ""}
                              onChange={(event) => updateRule(index, {
                                when: {
                                  column: rule.when?.column ?? rule.column,
                                  operator: rule.when?.operator ?? "eq",
                                  value: event.target.value,
                                },
                              })}
                            />
                          </label>
                        </fieldset>
                        <fieldset className="quality-rule__wide quality-rule__columns">
                          <legend>Comprobar entonces</legend>
                          <label>Columna objetivo
                            <select
                              aria-label={`Columna objetivo conditional regla ${index + 1}`}
                              value={rule.then?.column ?? rule.column}
                              onChange={(event) => updateConditionalThen(index, { column: event.target.value })}
                            >
                              {dataset.columns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}
                            </select>
                          </label>
                          <label>Comprobación then
                            <select
                              aria-label={`Comprobación then regla ${index + 1}`}
                              value={rule.then?.kind ?? "not_null"}
                              onChange={(event) => {
                                const kind = event.target.value as QualityRuleKind;
                                updateConditionalThen(index, {
                                  kind,
                                  min: undefined,
                                  max: undefined,
                                  values: kind === "allowed_values" ? rule.then?.values ?? [] : undefined,
                                  pattern: kind === "regex" ? rule.then?.pattern ?? "" : undefined,
                                  dtype: kind === "dtype" ? rule.then?.dtype ?? "string" : undefined,
                                  columns: undefined,
                                  operator: undefined,
                                  minDate: undefined,
                                  maxDate: undefined,
                                  when: undefined,
                                  then: undefined,
                                });
                              }}
                            >
                              <option value="not_null">Sin nulos</option>
                              <option value="non_empty">Texto no vacío</option>
                              <option value="numeric_range">Rango numérico</option>
                              <option value="allowed_values">Valores permitidos</option>
                              <option value="regex">Expresión regular</option>
                              <option value="dtype">Tipo esperado</option>
                            </select>
                          </label>
                          {(rule.then?.kind === "numeric_range") && (
                            <>
                              <label>Mínimo then
                                <input
                                  type="number"
                                  aria-label={`Mínimo then regla ${index + 1}`}
                                  value={rule.then.min ?? ""}
                                  onChange={(event) => updateConditionalThen(index, {
                                    min: event.target.value === "" ? undefined : Number(event.target.value),
                                  })}
                                />
                              </label>
                              <label>Máximo then
                                <input
                                  type="number"
                                  aria-label={`Máximo then regla ${index + 1}`}
                                  value={rule.then.max ?? ""}
                                  onChange={(event) => updateConditionalThen(index, {
                                    max: event.target.value === "" ? undefined : Number(event.target.value),
                                  })}
                                />
                              </label>
                            </>
                          )}
                          {rule.then?.kind === "allowed_values" && (
                            <label className="quality-rule__wide">Valores permitidos then
                              <textarea
                                rows={2}
                                aria-label={`Valores permitidos then regla ${index + 1}`}
                                value={rule.then.values?.join("\n") ?? ""}
                                onChange={(event) => updateConditionalThen(index, {
                                  values: event.target.value.split(/\r?\n/).filter((value) => value.length > 0),
                                })}
                              />
                            </label>
                          )}
                          {rule.then?.kind === "regex" && (
                            <label className="quality-rule__wide">Patrón regular then
                              <input
                                type="text"
                                aria-label={`Patrón regular then regla ${index + 1}`}
                                value={rule.then.pattern ?? ""}
                                onChange={(event) => updateConditionalThen(index, { pattern: event.target.value })}
                              />
                            </label>
                          )}
                          {rule.then?.kind === "dtype" && (
                            <label>Tipo esperado then
                              <select
                                aria-label={`Tipo esperado then regla ${index + 1}`}
                                value={rule.then.dtype ?? "string"}
                                onChange={(event) => updateConditionalThen(index, { dtype: event.target.value })}
                              >
                                <option value="string">Texto</option>
                                <option value="integer">Entero</option>
                                <option value="float">Decimal</option>
                                <option value="boolean">Booleano</option>
                                <option value="date">Fecha</option>
                                <option value="datetime">Fecha y hora</option>
                              </select>
                            </label>
                          )}
                        </fieldset>
                      </>
                    )}
                    {isSchemaRule && (
                      <fieldset className="quality-rule__wide quality-rule__columns">
                        <legend>Contrato de esquema</legend>
                        <label className="quality-rule__wide">Columnas requeridas
                          <textarea
                            rows={3}
                            aria-label={`Columnas requeridas esquema regla ${index + 1}`}
                            value={rule.columns?.join("\n") ?? ""}
                            onChange={(event) => updateRule(index, {
                              columns: event.target.value
                                .split(/\r?\n/)
                                .map((value) => value.trim())
                                .filter((value) => value.length > 0),
                            })}
                          />
                          <span className="quality-rule__help">Una columna por línea; el dataset puede tener columnas adicionales si se permite abajo.</span>
                        </label>
                        <label className="quality-rule__check">
                          <input
                            type="checkbox"
                            aria-label={`Permitir columnas adicionales esquema regla ${index + 1}`}
                            checked={rule.allowAdditional ?? true}
                            onChange={(event) => updateRule(index, { allowAdditional: event.target.checked })}
                          />
                          Permitir columnas adicionales
                        </label>
                        <label className="quality-rule__wide">Orden requerido, opcional
                          <textarea
                            rows={2}
                            aria-label={`Orden requerido esquema regla ${index + 1}`}
                            value={rule.requiredOrder?.join("\n") ?? ""}
                            onChange={(event) => updateRule(index, {
                              requiredOrder: event.target.value.trim().length === 0
                                ? undefined
                                : event.target.value
                                  .split(/\r?\n/)
                                  .map((value) => value.trim())
                                  .filter((value) => value.length > 0),
                            })}
                          />
                        </label>
                      </fieldset>
                    )}
                    <button type="button" className="quality-rule__remove" aria-label={`Eliminar regla ${index + 1}`}
                      onClick={() => changeRules(rules.filter((_, ruleIndex) => ruleIndex !== index))}>Eliminar</button>
                  </fieldset>
                );
              })}
              </div>
              <div className="quality-contract__actions">
                <span>{rules.length}/{MAX_QUALITY_RULES} reglas</span>
                <button type="button" onClick={addRule} disabled={busy || rules.length >= MAX_QUALITY_RULES}>Añadir regla</button>
              </div>
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
            {validationError && <p className="notice notice--error" role="alert">{validationError}</p>}
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
        ) : (
          <div className="quality-contract__unvalidated">
            <strong>Entrega no validada</strong>
            <p>No hay reglas activas. Confirma abajo para exportar sin validación durante esta sesión.</p>
            <label>
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
          </div>
        )}

        {contract.gate.kind === "loading" && <p className="notice" role="status">Validando contrato localmente…</p>}
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
                          ({result.invalidPct.toFixed(2)}%). {guidance.nextStep}
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
          <p className="step">Formato de entrega</p>
          <h3 id="export-title">Exportar dataset activo</h3>
          <p>Elige el formato y la protección antes de crear la copia.</p>
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
              aria-describedby="privacy-mode-note"
              value={selectedPrivacyMode}
              onChange={(event) => changePrivacyMode(event.target.value as PrivacyMode)}
              disabled={busy}
            >
              <option value="none">Sin protección adicional</option>
              <option value="mask">Enmascarar columnas detectadas</option>
              <option value="hash">Aplicar hash SHA-256 a columnas detectadas</option>
            </select>
            <small id="privacy-mode-note" className="privacy-mode__note">
              SHA-256 es determinista y no usa salt: valores predecibles pueden adivinarse. No equivale a anonimización; revisa el archivo antes de compartirlo.
            </small>
          </label>
          <details
            className="delivery-presets"
            onToggle={(event) => {
              if (event.currentTarget.open && !presetsLoaded && !presetsLoading) void refreshDeliveryPresets();
            }}
          >
            <summary>Presets de entrega guardados</summary>
            <p>Guarda formatos, protección y columnas para repetirlos. Las credenciales quedan fuera del preset.</p>
            <label>
              Preset local
              <select
                aria-label="Preset de entrega local"
                value={selectedPresetId}
                onChange={(event) => {
                  setSelectedPresetId(event.target.value);
                  setOpenedPreset(null);
                  setPresetNotice(null);
                  const summary = presets.find((item) => item.id === event.target.value);
                  if (summary) setPresetName(summary.name);
                }}
                disabled={presetWorking || presetsLoading}
              >
                <option value="">Selecciona un preset</option>
                {presets.map((preset) => (
                  <option key={preset.id} value={preset.id}>
                    {preset.name} · {preset.format} · {preset.selectedColumnCount} columnas
                  </option>
                ))}
              </select>
            </label>
            <label>
              Nombre del preset
              <input
                aria-label="Nombre del preset de entrega"
                value={presetName}
                maxLength={80}
                onChange={(event) => setPresetName(event.target.value)}
                disabled={presetWorking}
              />
            </label>
            <div className="delivery-presets__actions">
              <button type="button" className="secondary-action" onClick={() => void loadSelectedDeliveryPreset()} disabled={!selectedPresetId || presetWorking || presetsLoading}>
                {presetWorking ? "Procesando preset…" : "Abrir y verificar"}
              </button>
              <button type="button" className="secondary-action" onClick={() => void saveCurrentDeliveryPreset()} disabled={!presetName.trim() || presetWorking}>
                Guardar preset
              </button>
              {selectedPresetId && (
                <button type="button" className="secondary-action" onClick={() => void deleteSelectedDeliveryPreset()} disabled={presetWorking}>
                  Eliminar preset
                </button>
              )}
            </div>
            {presetsLoading && <p role="status">Cargando catálogo local…</p>}
            {!presetsLoading && presetsLoaded && presets.length === 0 && <p role="note">Aún no hay presets locales.</p>}
            {presetsError && (
              <div className="notice notice--error" role="alert">
                <p>{presetsError}</p>
                <button type="button" className="secondary-action" onClick={() => void refreshDeliveryPresets()} disabled={presetsLoading}>Reintentar catálogo</button>
              </div>
            )}
            {openedPreset && (
              <div className="delivery-presets__verification" role="status">
                <p>
                  {openedPresetSchemaMatches
                    ? `Esquema verificado: ${openedPreset.selectedColumns.length} columnas, mismo orden.`
                    : "El esquema guardado no coincide exactamente con el dataset actual; revisa la diferencia antes de aplicar."}
                </p>
                {!openedPresetSchemaMatches && (
                  <p>
                    Faltan: {openedPreset.selectedColumns.filter((column) => !dataset.columns.some((current) => current.name === column)).join(", ") || "ninguna"}. Nuevas: {dataset.columns.map((column) => column.name).filter((column) => !openedPreset.selectedColumns.includes(column)).join(", ") || "ninguna"}.
                  </p>
                )}
                <button type="button" className="secondary-action" onClick={applyOpenedDeliveryPreset} disabled={presetWorking}>
                  {openedPresetSchemaMatches ? "Aplicar preset verificado" : "Aplicar tras revisar esquema"}
                </button>
              </div>
            )}
            {presetNotice && <p className="notice notice--success" role="status">{presetNotice}</p>}
          </details>
          {isDatabaseExportFormat(selectedExportFormat) && (
            <fieldset className="database-target">
              <legend>Destino remoto · {exportFormatLabel}</legend>
              <p>
                Usa el controlador ODBC correspondiente. La cadena y la contraseña solo viven durante esta sesión y no se guardan en el proyecto.
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
                  Esquema (opcional)
                  <input
                    aria-label="Esquema de destino"
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
                    aria-label="Tabla de destino"
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
                disabled={busy || databaseTargetError !== null || databasePreflightState.kind === "working"}
              >
                {databasePreflightState.kind === "working" ? "Analizando compatibilidad…" : "Analizar compatibilidad"}
              </button>
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
          <button
            className="primary-action export-action"
            type="button"
            onClick={() => void requestExport(selectedExportFormat)}
            disabled={busy || validationError !== null || needsUnvalidatedConfirmation || !databaseReady}
          >
            {contract.kind === "with_contract" && !gatePassed
              ? `Validar y exportar ${exportFormatLabel}`
              : `Exportar ${exportFormatLabel}`}
          </button>
        </div>
        {selectedExportFormat === "bundle" && (
          <p className="export-requirement" role="note">
            {recipeDraft
              ? "Este paquete incluirá delivery-summary.md, recipe.json con la receta actual validada y sus referencias y hashes en manifest.json."
              : "No hay una receta activa para incluir; el paquete contendrá dataset.csv, dictionary.json, delivery-summary.md y manifest.json."}
          </p>
        )}
        {contract.kind === "with_contract" && !gatePassed && !validationError && (
          <p className="export-requirement">El contrato se comprobará antes de crear la copia.</p>
        )}
        {needsUnvalidatedConfirmation && (
          <p className="export-requirement">Confirma arriba si quieres exportar sin validar la calidad.</p>
        )}
      </section>
      {exportState.kind === "loading" && (
        <OperationProgressView
          progress={exportState.progress}
          cancellation={exportState.cancellation === "requested"
            ? { kind: "requested" }
            : { kind: "available", onCancel: onCancelExport }}
        />
      )}
      {exportState.kind === "success" && (
        <section className="delivery-result" aria-labelledby="delivery-result-title" aria-live="polite">
          <div className="delivery-result__heading">
            <div>
              <p className="step">Entrega completada</p>
              <h3 id="delivery-result-title">Copia lista</h3>
              <p>La salida se publicó correctamente. El dataset preparado sigue disponible en Columnia.</p>
            </div>
            {exportState.result.format !== "PostgreSQL"
              && exportState.result.format !== "MySQL"
              && exportState.result.format !== "SQL Server" && (
              <button
                type="button"
                className="primary-action"
                onClick={() => void revealLastExport()}
                disabled={openOutputState === "working"}
              >
                {openOutputState === "working" ? "Abriendo carpeta…" : "Abrir carpeta"}
              </button>
            )}
          </div>
          <dl className="delivery-result__facts">
            <div>
              <dt>{exportState.result.format === "PostgreSQL" || exportState.result.format === "MySQL" || exportState.result.format === "SQL Server" ? "Destino" : "Archivo"}</dt>
              <dd>{exportState.result.fileName}</dd>
            </div>
            <div>
              <dt>Formato</dt>
              <dd>{exportState.result.format}</dd>
            </div>
            {exportState.result.format !== "PostgreSQL"
              && exportState.result.format !== "MySQL"
              && exportState.result.format !== "SQL Server" && (
              <div>
                <dt>Tamaño</dt>
                <dd>{formatFileSize(exportState.result.fileSizeBytes)}</dd>
              </div>
            )}
            <div>
              <dt>Calidad</dt>
              <dd>{approvedQualitySummary ?? "Salida confirmada sin reglas de calidad"}</dd>
            </div>
            <div>
              <dt>Preparación incluida</dt>
              <dd>{preparationChanges.length > 0
                ? `${preparationChanges.length.toLocaleString()} cambios del historial activo`
                : "Dataset activo sin cambios registrados en el historial"}</dd>
            </div>
            <div>
              <dt>Protección adicional</dt>
              <dd>{exportState.result.protectedColumnCount > 0
                ? `${exportState.result.protectedColumnCount.toLocaleString()} columnas: ${exportState.result.protectedColumns?.join(", ")}`
                : "No aplicada"}</dd>
            </div>
          </dl>
          {preparationChanges.length > 0 && (
            <details className="delivery-result__changes">
              <summary>Ver cambios incluidos ({preparationChanges.length.toLocaleString()})</summary>
              <ol>{preparationChanges.map((change, index) => <li key={`${index}-${change}`}>{change}</li>)}</ol>
            </details>
          )}
          {exportState.result.format === "Paquete Columnia" && (
            <p className="delivery-result__note">
              Incluye dataset.csv, dictionary.json, delivery-summary.md y manifest.json{recipeDraft ? "; recipe.json incluye la receta validada" : ""}, además del reporte de calidad cuando hay reglas aprobadas.
            </p>
          )}
          {contract.kind === "without_contract" && (
            <p className="notice notice--warning">Esta copia no incluye una validación de calidad.</p>
          )}
          {openOutputState === "opened" && (
            <p className="notice notice--success" role="status">Carpeta de exportación abierta.</p>
          )}
          {openOutputState === "error" && (
            <p className="notice notice--error" role="alert">No se pudo abrir la carpeta de exportación.</p>
          )}
        </section>
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
