import { useEffect, useRef, useState, type ReactNode } from "react";

import { ModalDialog } from "../../components/ModalDialog";
import {
  pickTransformRecipe,
  saveTransformRecipe,
  type DatasetPreview,
  type LoadedRecipe,
  type PrivacyMode,
  type RecipeMigrationReport,
  type SavedRecipe,
  type TransformRecipe,
} from "../../bridge";
import {
  isLoadedRecipe,
  requiresImpactConfirmation,
  type RecipeFileStatus,
} from "./prepareModel";
import { buildTransformPreview, visibleColumnNames, type TransformPreview } from "./transformAdvisor";

function operationGroupStatus(count: number, singular: string, plural: string) {
  if (count === 0) return "Sin cambios";
  return `${count} ${count === 1 ? singular : plural}`;
}

const migrationOperationLabels: Record<string, string> = {
  renames: "Renombres de columnas",
  casts: "Conversiones de tipo",
  date_parses: "Interpretación de fechas",
  filters: "Filtros de filas",
  calculated_column: "Columnas calculadas",
  find_replace: "Búsqueda y reemplazo",
  keep_columns: "Selección de columnas",
  split_column: "División de columnas",
  merge_columns: "Unión de columnas",
  outlier_treatments: "Tratamiento de atípicos",
  group_summary: "Resumen agrupado",
  contact_normalizations: "Normalización de contactos",
  text_extractions: "Extracción de texto",
  "export.formats": "Formatos de entrega",
  "export.selected_columns": "Columnas de entrega",
  "export.privacy_mode": "Protección de privacidad",
  "export.report_format": "Formato de reporte",
  quality_rules: "Reglas de calidad",
  analysis_checks: "Comprobaciones de análisis",
  source_reference: "Referencia de origen",
  snapshot_reference: "Referencia de snapshot",
};

const migrationWarningLabels: Record<string, string> = {
  "export.report_format": "Formato de reporte",
  "export.formats": "Formatos de entrega",
  "export.selected_columns": "Columnas de entrega",
  "export.privacy_mode": "Protección de privacidad",
  source_reference: "Referencia de origen",
  snapshot_reference: "Referencia de snapshot",
  quality_rules: "Reglas de calidad",
  analysis_checks: "Comprobaciones de análisis",
};

const migrationFormatLabels: Record<string, string> = {
  csv: "CSV",
  json: "JSON",
  parquet: "Parquet",
  sql: "SQL",
  excel: "Excel",
  sqlite: "SQLite",
  bundle: "Paquete Columnia",
};

function safeMigrationCount(value: number) {
  return Math.max(0, Math.round(Number.isFinite(value) ? value : 0));
}

function safeMigrationText(value: string, fallback: string) {
  const sanitized = value
    .replace(/[\u0000-\u001f\u007f]/g, "")
    .replace(/\b[A-Za-z]:[\\/][^\n,;]*/g, "[ruta omitida]")
    .replace(/(^|\s)(?:\.\.?[\\/]|\/)[^\s,;]+/g, "$1[ruta omitida]")
    .replace(/"[^"]*"/g, '"valor omitido"')
    .replace(/'[^']*'/g, "'valor omitido'")
    .replace(/\s+/g, " ")
    .trim();
  if (!sanitized) return fallback;
  return sanitized.length > 120 ? `${sanitized.slice(0, 117)}…` : sanitized;
}

function migrationOperationLabel(operation: string) {
  return migrationOperationLabels[operation] ?? "Operación de compatibilidad";
}

function migrationWarningLabel(path: string) {
  return migrationWarningLabels[path] ?? "Elemento de compatibilidad";
}

function migrationSourceLabel(report: RecipeMigrationReport) {
  const source = report.sourceFormat === "dataprep" ? "DataPrep" : "formato legacy";
  return report.sourceVersion === null ? source : `${source} v${report.sourceVersion}`;
}

function migrationPrivacyLabel(mode: PrivacyMode) {
  return mode === "mask" ? "enmascarada" : mode === "hash" ? "con hash" : "sin transformación";
}

function RecipeMigrationReportPanel({
  report,
  exportOptions,
}: {
  report: RecipeMigrationReport;
  exportOptions: NonNullable<SavedRecipe["exportOptions"]> | null;
}) {
  const convertedItems = safeMigrationCount(report.convertedItems);
  const omittedItems = safeMigrationCount(report.omittedItems);
  const warningCount = safeMigrationCount(report.warningCount);
  const totalItems = convertedItems + omittedItems;
  const conversionPercent = totalItems === 0 ? 0 : Math.round((convertedItems / totalItems) * 100);
  const statusLabel = omittedItems > 0 ? "Revisión necesaria" : warningCount > 0 ? "Revisar" : "Lista para validar";
  const session = report.session;

  return (
    <section
      className="recipe-migration-report"
      role="status"
      aria-label="Informe de migración de receta"
      aria-labelledby="recipe-migration-report-title"
    >
      <div className="recipe-migration-report__header">
        <div>
          <p className="step">Compatibilidad</p>
          <h4 id="recipe-migration-report-title">Informe de migración</h4>
          <p>
            {migrationSourceLabel(report)}. La receta quedó lista para revisión antes de aplicarla.
          </p>
        </div>
        <strong className={`recipe-migration-report__status recipe-migration-report__status--${omittedItems > 0 ? "warning" : "ready"}`}>
          {statusLabel}
        </strong>
      </div>

      <div className="recipe-migration-report__metrics" role="group" aria-label="Resultado de conversión">
        <div><span>Convertidos</span><strong>{convertedItems.toLocaleString()}</strong></div>
        <div><span>Omitidos</span><strong>{omittedItems.toLocaleString()}</strong></div>
        <div><span>Advertencias</span><strong>{warningCount.toLocaleString()}</strong></div>
      </div>
      <div className="recipe-migration-report__progress-row">
        <div
          className="recipe-migration-report__progress"
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={conversionPercent}
          aria-label={`${conversionPercent}% de elementos convertidos`}
        >
          <span style={{ width: `${conversionPercent}%` }} />
        </div>
        <span>{conversionPercent}% convertido</span>
      </div>

      {exportOptions && (
        <p className="recipe-migration-report__delivery">
          <strong>Entrega importada:</strong> {exportOptions.formats.map((format) => migrationFormatLabels[format] ?? "Formato compatible").join(", ")} · {exportOptions.selectedColumns.length > 0 ? `${exportOptions.selectedColumns.length} columnas seleccionadas` : "todas las columnas"} · privacidad {migrationPrivacyLabel(exportOptions.privacyMode)}.
        </p>
      )}

      {session && (
        <div className="recipe-session-summary" aria-label="Resumen de sesión DataPrep">
          <div className="recipe-session-summary__header">
            <strong>Contexto de sesión</strong>
            <small>Metadatos sensibles ocultos</small>
          </div>
          <ul className="recipe-session-summary__facts">
            <li><span>Etapa de trabajo</span><strong>{session.stageLabel ? "Registrada" : "No indicada"}</strong></li>
            <li><span>Hoja de origen</span><strong>{session.sheetName ? "Registrada" : "No indicada"}</strong></li>
            <li><span>Operaciones aplicadas</span><strong>{safeMigrationCount(session.appliedOperationCount).toLocaleString()}</strong></li>
            <li><span>Reglas de calidad</span><strong>{safeMigrationCount(session.qualityRuleCount).toLocaleString()}</strong></li>
            <li><span>Comprobaciones</span><strong>{safeMigrationCount(session.analysisCheckCount).toLocaleString()}</strong></li>
          </ul>
          <p>
            {session.hasSourceReference || session.hasSnapshotReference
              ? "Se detectaron referencias de origen o snapshot; revísalas y vuelve a seleccionarlas en este equipo."
              : "No se detectaron referencias activables de origen o snapshot."}
          </p>
        </div>
      )}

      <div className="recipe-migration-report__details">
        {report.convertedOperations.length > 0 && (
          <details>
            <summary>Operaciones convertidas ({report.convertedOperations.length})</summary>
            <ul>
              {report.convertedOperations.map((operation, index) => (
                <li key={`${operation}-${index}`}>{migrationOperationLabel(operation)}</li>
              ))}
            </ul>
          </details>
        )}
        {report.omittedOperations.length > 0 && (
          <details open={omittedItems > 0}>
            <summary>Operaciones omitidas ({report.omittedOperations.length})</summary>
            <ul>
              {report.omittedOperations.map((operation, index) => (
                <li key={`${operation}-${index}`}>{migrationOperationLabel(operation)}</li>
              ))}
            </ul>
          </details>
        )}
        {report.warnings.length > 0 && (
          <details open={omittedItems > 0}>
            <summary>Advertencias de compatibilidad ({report.warnings.length})</summary>
            <ul>
              {report.warnings.map((warning, index) => (
                <li key={`${warning.severity}-${index}`}>
                  <strong>{migrationWarningLabel(warning.path)}:</strong>{" "}
                  {warning.severity === "omitted"
                    ? "No se convirtió automáticamente; recrea este ajuste si todavía es necesario."
                    : "Se convirtió con una suposición; valida el resultado antes de continuar."}
                </li>
              ))}
            </ul>
          </details>
        )}
        {report.manualActions.length > 0 && (
          <details open={omittedItems > 0}>
            <summary>Acciones manuales ({report.manualActions.length})</summary>
            <ul>
              {report.manualActions.map((action, index) => (
                <li key={`${action}-${index}`}>{safeMigrationText(action, "Revisar el elemento de compatibilidad indicado.")}</li>
              ))}
            </ul>
          </details>
        )}
      </div>
      <p className="recipe-migration-report__privacy">
        {report.artifactSha256 ? "Identificador de origen disponible; su valor se mantiene oculto." : "No hay identificador de origen disponible."} No se muestran rutas ni valores originales en este resumen.
      </p>
    </section>
  );
}

function TransformAdvisor({ preview }: { preview: TransformPreview }) {
  const riskLabel = preview.risk === "high" ? "Alto impacto" : preview.risk === "medium" ? "Impacto medio" : "Bajo riesgo";
  const riskDescription = preview.risk === "high"
    ? "Puede reducir filas, columnas o granularidad. Revisa la estimación antes de aplicar."
    : preview.risk === "medium"
      ? "Puede cambiar valores o retirar fuentes. Conserva una alternativa hasta validar."
      : "No se detectan eliminaciones de filas ni columnas en esta receta.";
  const rowDelta = preview.rowsDelta === null
    ? "Por determinar"
    : preview.rowsDelta === 0
      ? "Sin cambio esperado"
      : `${preview.rowsDelta > 0 ? "+" : ""}${preview.rowsDelta.toLocaleString()} filas`;
  const columnDelta = preview.columnsDelta === 0
    ? "Sin cambio"
    : `${preview.columnsDelta > 0 ? "+" : ""}${preview.columnsDelta.toLocaleString()} columnas`;

  return (
    <section className={`transform-advisor transform-advisor--${preview.risk}`} aria-labelledby="transform-advisor-title">
      <div className="transform-advisor__header">
        <div>
          <p className="step">Revisión antes de ejecutar</p>
          <h4 id="transform-advisor-title">Impacto estimado de la receta</h4>
          <p>{riskDescription}</p>
        </div>
        <strong className="transform-advisor__risk" aria-label={`Riesgo: ${riskLabel}`}>{riskLabel}</strong>
      </div>
      <div className="transform-advisor__metrics" aria-label="Comparación antes y después">
        <div>
          <span>Antes</span>
          <strong>{preview.beforeRows.toLocaleString()} filas · {preview.beforeColumns} columnas</strong>
          <small>{visibleColumnNames(preview.beforeColumnNames)}</small>
        </div>
        <div>
          <span>Después</span>
          <strong>{preview.afterRows === null ? "Filas no estimables" : `${preview.afterRows.toLocaleString()} filas`} · {preview.afterColumns} columnas</strong>
          <small>{visibleColumnNames(preview.afterColumnNames)}</small>
        </div>
      </div>
      <div className="transform-advisor__signals">
        <span>Variación de filas: <strong>{rowDelta}</strong></span>
        <span>Variación de columnas: <strong>{columnDelta}</strong></span>
        <span>Confianza: <strong>{preview.confidence}% ({preview.confidenceLabel})</strong></span>
      </div>
      <p className="transform-advisor__basis">{preview.basis}</p>
      <div className="transform-advisor__columns">
        <div>
          <strong>Operaciones detectadas</strong>
          <p>{preview.operationLabels.join(" · ")}</p>
        </div>
        <div>
          <strong>Recomendaciones</strong>
          <ul>{preview.recommendations.map((recommendation) => <li key={recommendation}>{recommendation}</li>)}</ul>
        </div>
      </div>
      <details className="transform-advisor__recovery">
        <summary>Alternativas de recuperación</summary>
        <ul>{preview.recoveryOptions.map((option) => <li key={option}>{option}</li>)}</ul>
      </details>
    </section>
  );
}

function RecipeOperationGroup({
  title,
  status,
  active = false,
  children,
}: {
  title: string;
  status: string;
  active?: boolean;
  children: ReactNode;
}) {
  return (
    <details className={`transform-recipe__group${active ? " transform-recipe__group--active" : ""}`}>
      <summary className="transform-recipe__group-summary">
        <span className="transform-recipe__group-title">{title}</span>
        <small className="transform-recipe__group-status">{status}</small>
      </summary>
      <fieldset>
        <legend className="transform-recipe__group-legend">{title}</legend>
        {children}
      </fieldset>
    </details>
  );
}

export function TransformRecipeEditor({
  dataset,
  busy,
  initialDraft,
  onApply,
  onDraftChange,
}: {
  dataset: DatasetPreview;
  busy: boolean;
  initialDraft: SavedRecipe | null;
  onApply: (recipe: TransformRecipe) => void;
  onDraftChange: (draft: SavedRecipe) => void;
}) {
  type RenameDraft = TransformRecipe["renames"][number];
  type CastDraft = TransformRecipe["casts"][number];
  type DateDraft = TransformRecipe["dateParses"][number];
  type FilterDraft = TransformRecipe["filters"][number];
  type CalculationDraft = NonNullable<TransformRecipe["calculatedColumn"]>;
  type FindReplaceDraft = NonNullable<TransformRecipe["findReplace"]>;
  type SplitDraft = NonNullable<TransformRecipe["splitColumn"]>;
  type MergeDraft = NonNullable<TransformRecipe["mergeColumns"]>;
  type OutlierDraft = TransformRecipe["outlierTreatments"][number];
  type GroupSummaryDraft = NonNullable<TransformRecipe["groupSummary"]>;
  type ContactDraft = TransformRecipe["contactNormalizations"][number];
  type ExtractionDraft = TransformRecipe["textExtractions"][number];

  const initialRecipe = initialDraft?.recipe;
  const [renames, setRenames] = useState<RenameDraft[]>(
    initialRecipe?.renames.length ? initialRecipe.renames : [{ from: "", to: "" }],
  );
  const [casts, setCasts] = useState<CastDraft[]>(
    initialRecipe?.casts.length ? initialRecipe.casts : [{ column: "", target: "string" }],
  );
  const [dateParses, setDateParses] = useState<DateDraft[]>(
    initialRecipe?.dateParses.length
      ? initialRecipe.dateParses
      : [{ column: "", format: "iso8601", target: "date" }],
  );
  const [filters, setFilters] = useState<FilterDraft[]>(initialRecipe?.filters ?? []);
  const [calculationEnabled, setCalculationEnabled] = useState(initialRecipe?.calculatedColumn !== null && initialRecipe?.calculatedColumn !== undefined);
  const [calculation, setCalculation] = useState<CalculationDraft>(initialRecipe?.calculatedColumn ?? {
    name: "",
    source: "",
    operation: "add",
    operand: { kind: "literal", value: "" },
  });
  const [pendingConfirmation, setPendingConfirmation] = useState<TransformRecipe | null>(null);
  const [findReplaceEnabled, setFindReplaceEnabled] = useState(initialRecipe?.findReplace !== null && initialRecipe?.findReplace !== undefined);
  const [findReplace, setFindReplace] = useState<FindReplaceDraft>(initialRecipe?.findReplace ?? { scope: "column", column: null, find: "", replace: "" });
  const [keptColumns, setKeptColumns] = useState<string[]>(initialRecipe?.keepColumns ?? dataset.columns.map((column) => column.name));
  const [splitEnabled, setSplitEnabled] = useState(initialRecipe?.splitColumn !== null && initialRecipe?.splitColumn !== undefined);
  const [split, setSplit] = useState<SplitDraft>(initialRecipe?.splitColumn ?? { source: "", delimiter: "", names: [], dropSource: false });
  const [splitNamesInput, setSplitNamesInput] = useState(initialRecipe?.splitColumn?.names.join(", ") ?? "");
  const [mergeEnabled, setMergeEnabled] = useState(initialRecipe?.mergeColumns !== null && initialRecipe?.mergeColumns !== undefined);
  const [merge, setMerge] = useState<MergeDraft>(initialRecipe?.mergeColumns ?? { sources: [], name: "", separator: "", dropSources: false });
  const [outlierTreatments, setOutlierTreatments] = useState<OutlierDraft[]>(initialRecipe?.outlierTreatments ?? []);
  const [groupEnabled, setGroupEnabled] = useState(initialRecipe?.groupSummary !== null && initialRecipe?.groupSummary !== undefined);
  const [groupSummary, setGroupSummary] = useState<GroupSummaryDraft>(initialRecipe?.groupSummary ?? { groupBy: [], aggregations: [] });
  const [contacts, setContacts] = useState<ContactDraft[]>(initialRecipe?.contactNormalizations ?? []);
  const [extractions, setExtractions] = useState<ExtractionDraft[]>(initialRecipe?.textExtractions ?? []);
  const [recipeName, setRecipeName] = useState(initialDraft?.name ?? "Mi receta");
  const [migrationReport, setMigrationReport] = useState<RecipeMigrationReport | null>(initialDraft?.migrationReport ?? null);
  const [exportOptions, setExportOptions] = useState(initialDraft?.exportOptions ?? null);
  const [recipeFileStatus, setRecipeFileStatus] = useState<RecipeFileStatus>({ kind: "idle" });
  const draftSavedAt = useRef(initialDraft?.savedAt ?? new Date().toISOString());
  const acknowledgedRecipeFingerprint = useRef<string | null>(null);
  const recipeBusy = busy || recipeFileStatus.kind === "working";

  const activeRenames = renames.filter((item) => item.from || item.to);
  const activeCasts = casts.filter((item) => item.column);
  const activeDateParses = dateParses.filter((item) => item.column);
  const activeFilters = filters.filter((item) => item.column);
  const searchableTextColumns = dataset.columns.filter((column) => {
    if (activeDateParses.some((item) => item.column === column.name)) return false;
    const cast = [...activeCasts].reverse().find((item) => item.column === column.name);
    return cast ? cast.target === "string" : column.dataType === "String";
  });
  const numericColumns = dataset.columns.filter((column) => {
    if (activeDateParses.some((item) => item.column === column.name)) return false;
    const cast = [...activeCasts].reverse().find((item) => item.column === column.name);
    return cast ? ["integer", "decimal"].includes(cast.target) : ["Int64", "Float64"].includes(column.dataType);
  });
  function effectiveName(name: string) { return activeRenames.find((item) => item.from === name)?.to.trim() || name; }
  function effectiveType(name: string) {
    if (activeDateParses.some((item) => item.column === name)) return activeDateParses.find((item) => item.column === name)?.target === "date" ? "Date" : "Datetime";
    const cast = [...activeCasts].reverse().find((item) => item.column === name);
    if (cast) return cast.target === "integer" ? "Int64" : cast.target === "decimal" ? "Float64" : cast.target === "string" ? "String" : "Boolean";
    return dataset.columns.find((column) => column.name === name)?.dataType ?? "";
  }
  const operandRequired = !["year", "month", "day"].includes(calculation.operation);
  const calculationInvalid =
    calculationEnabled &&
    (!calculation.name.trim() ||
      !calculation.source ||
      (operandRequired && calculation.operation !== "concat" && !calculation.operand?.value.trim()) ||
      (calculation.operand?.kind === "column" && !calculation.operand.value));
  const calculationValid =
    !calculationEnabled ||
    !calculationInvalid;
  const calculationSourceDropped = calculationEnabled &&
    (!keptColumns.includes(calculation.source) ||
      (calculation.operand?.kind === "column" && !keptColumns.includes(calculation.operand.value)));
  const findReplaceInvalid = findReplaceEnabled &&
    (!findReplace.find || searchableTextColumns.length === 0 || (findReplace.scope === "column" && !findReplace.column));
  const dropsColumns = keptColumns.length < dataset.columns.length;
  const parsedSplitNames = splitNamesInput.split(",").map((name) => name.trim()).filter(Boolean);
  const postRenameNames = new Set(dataset.columns.map((column) => activeRenames.find((item) => item.from === column.name)?.to.trim() || column.name));
  const calculatedName = calculationEnabled ? calculation.name.trim() : "";
  const splitInvalid = splitEnabled && (!split.source || !split.delimiter || parsedSplitNames.length < 2 || parsedSplitNames.length > 16 || new Set(parsedSplitNames).size !== parsedSplitNames.length || parsedSplitNames.some((name) => postRenameNames.has(name) || name === calculatedName));
  const mergeInvalid = mergeEnabled && (merge.sources.length < 2 || merge.sources.length > 16 || !merge.name.trim() || postRenameNames.has(merge.name.trim()) || merge.name.trim() === calculatedName || parsedSplitNames.includes(merge.name.trim()));
  const sourceConflict = splitEnabled && mergeEnabled && split.dropSource && merge.sources.includes(split.source);
  const sourceNotKept = (splitEnabled && !keptColumns.includes(split.source)) || (mergeEnabled && merge.sources.some((source) => !keptColumns.includes(source)));
  const outlierDuplicate = new Set(outlierTreatments.map((item) => item.column)).size !== outlierTreatments.length;
  const outlierDependencyInvalid = outlierTreatments.some((item) => !keptColumns.includes(item.column) || (splitEnabled && split.dropSource && split.source === item.column) || (mergeEnabled && merge.dropSources && merge.sources.includes(item.column)));
  const outlierInvalid = outlierTreatments.some((item) => !item.column || !numericColumns.some((column) => column.name === item.column)) || outlierDuplicate || outlierTreatments.length > 16 || outlierDependencyInvalid;
  const groupPairs = groupSummary.aggregations.map((item) => `${item.column}:${item.operation}`);
  const groupOutputs = groupSummary.aggregations.map((item) => `${effectiveName(item.column)}_${item.operation}`);
  const groupDependenciesInvalid = [...groupSummary.groupBy, ...groupSummary.aggregations.map((item) => item.column)].some((name) => !keptColumns.includes(name) || (splitEnabled && split.dropSource && split.source === name) || (mergeEnabled && merge.dropSources && merge.sources.includes(name)));
  const groupOperationInvalid = groupSummary.aggregations.some((item) => {
    const dtype = effectiveType(item.column);
    if (!item.column) return true;
    if (["sum", "mean"].includes(item.operation)) return !["Int64", "Float64"].includes(dtype);
    if (["min", "max"].includes(item.operation)) return !["Int64", "Float64", "String", "Date", "Datetime"].includes(dtype);
    return false;
  });
  const groupInvalid = groupEnabled && (groupSummary.groupBy.length < 1 || groupSummary.groupBy.length > 8 || groupSummary.aggregations.length < 1 || groupSummary.aggregations.length > 32 || new Set(groupPairs).size !== groupPairs.length || new Set(groupOutputs).size !== groupOutputs.length || groupOutputs.some((name) => groupSummary.groupBy.map(effectiveName).includes(name)) || groupDependenciesInvalid || groupOperationInvalid);
  const contactDuplicate = new Set(contacts.map((item) => item.column)).size !== contacts.length;
  const extractionNames = extractions.map((item) => item.name.trim());
  const contactDependencyInvalid = contacts.some((item) => !keptColumns.includes(item.column) || (splitEnabled && split.dropSource && split.source === item.column) || (mergeEnabled && merge.dropSources && merge.sources.includes(item.column)));
  const extractionDependencyInvalid = extractions.some((item) => !keptColumns.includes(item.source) || (splitEnabled && split.dropSource && split.source === item.source) || (mergeEnabled && merge.dropSources && merge.sources.includes(item.source)));
  const contactInvalid = contacts.some((item) => !item.column || !searchableTextColumns.some((column) => column.name === item.column)) || contactDuplicate || contacts.length > 16 || contactDependencyInvalid;
  const extractionInvalid = extractions.length > 16 || (groupEnabled && extractions.length > 0) || extractionDependencyInvalid || new Set(extractionNames).size !== extractionNames.length || extractions.some((item) => !item.source || !searchableTextColumns.some((column) => column.name === item.source) || !item.name.trim() || item.name !== item.name.trim() || postRenameNames.has(item.name.trim()) || item.name.trim() === calculatedName || parsedSplitNames.includes(item.name.trim()) || item.name.trim() === merge.name.trim() || (["before", "after"].includes(item.kind) && !item.delimiter));
  const operationCount = activeRenames.length + activeCasts.length + activeDateParses.length + activeFilters.length + (calculationEnabled ? 1 : 0) + (findReplaceEnabled ? 1 : 0) + (dropsColumns ? 1 : 0) + (splitEnabled ? 1 : 0) + (mergeEnabled ? 1 : 0) + outlierTreatments.length + (groupEnabled ? 1 : 0) + contacts.length + extractions.length;
  const [operationsOpen, setOperationsOpen] = useState(operationCount > 0);
  const renameInvalid = activeRenames.some((item) => !item.from || !item.to.trim());
  const filterInvalid = activeFilters.some((item) =>
    !["eq", "neq", "is_null", "not_null"].includes(item.operator) && !item.value?.trim(),
  );
  const invalid = renameInvalid || filterInvalid || findReplaceInvalid || keptColumns.length === 0 || calculationSourceDropped || splitInvalid || mergeInvalid || sourceConflict || sourceNotKept || outlierInvalid || groupInvalid || contactInvalid || extractionInvalid ||
    !calculationValid;
  const transformPreview = operationCount > 0 && !invalid
    ? buildTransformPreview(dataset, buildRecipe())
    : null;

  function columnOptions() {
    return dataset.columns.map((column) => (
      <option key={column.name} value={column.name}>
        {column.name}
      </option>
    ));
  }

  function buildRecipe(): TransformRecipe {
    return {
      renames: activeRenames.map((item) => ({ from: item.from, to: item.to.trim() })),
      casts: activeCasts,
      dateParses: activeDateParses,
      filters: activeFilters.map((item) => ({
        ...item,
        value: ["is_null", "not_null"].includes(item.operator) ? null : item.value,
      })),
      calculatedColumn: calculationEnabled
        ? {
            ...calculation,
            name: calculation.name.trim(),
            operand: operandRequired ? calculation.operand : null,
          }
        : null,
      findReplace: findReplaceEnabled ? findReplace : null,
      keepColumns: dropsColumns ? dataset.columns.map((column) => column.name).filter((name) => keptColumns.includes(name)) : null,
      splitColumn: splitEnabled ? { ...split, names: parsedSplitNames } : null,
      mergeColumns: mergeEnabled ? { ...merge, name: merge.name.trim() } : null,
      outlierTreatments,
      groupSummary: groupEnabled ? groupSummary : null,
      contactNormalizations: contacts,
      textExtractions: extractions.map((item) => ({ ...item, name: item.name.trim(), delimiter: ["before", "after"].includes(item.kind) ? item.delimiter : null })),
    };
  }

  const draftFingerprint = JSON.stringify(buildRecipe());
  const workspaceDraftFingerprint = `${recipeName}\u0000${draftFingerprint}`;
  const lastWorkspaceDraftFingerprint = useRef(
    `${initialDraft?.name ?? "Mi receta"}\u0000${JSON.stringify(initialDraft?.recipe ?? buildRecipe())}`,
  );
  useEffect(() => {
    if (invalid) return;
    if (lastWorkspaceDraftFingerprint.current === workspaceDraftFingerprint) return;
    lastWorkspaceDraftFingerprint.current = workspaceDraftFingerprint;
    const nextDraft: SavedRecipe = {
      version: 1,
      name: recipeName.trim() || "Mi receta",
      savedAt: draftSavedAt.current,
      recipe: buildRecipe(),
    };
    if (migrationReport) nextDraft.migrationReport = migrationReport;
    if (exportOptions) nextDraft.exportOptions = exportOptions;
    onDraftChange(nextDraft);
  }, [invalid, workspaceDraftFingerprint, onDraftChange]);

  useEffect(() => {
    if (recipeFileStatus.kind !== "success") return;
    if (acknowledgedRecipeFingerprint.current === null) {
      acknowledgedRecipeFingerprint.current = draftFingerprint;
      return;
    }
    if (acknowledgedRecipeFingerprint.current !== draftFingerprint) {
      setRecipeFileStatus({ kind: "idle" });
    }
  }, [draftFingerprint, recipeFileStatus.kind]);

  function submitRecipe() {
    if (operationCount === 0 || invalid) return;
    const recipe = buildRecipe();
    if (requiresImpactConfirmation(recipe)) setPendingConfirmation(recipe);
    else onApply(recipe);
  }

  async function saveRecipeDraft() {
    if (recipeBusy || operationCount === 0 || invalid || !recipeName.trim()) return;
    setRecipeFileStatus({ kind: "working", action: "save" });
    try {
      const saved = migrationReport || exportOptions
        ? await saveTransformRecipe(buildRecipe(), recipeName.trim(), migrationReport, exportOptions)
        : await saveTransformRecipe(buildRecipe(), recipeName.trim());
      if (saved) {
        acknowledgedRecipeFingerprint.current = draftFingerprint;
        draftSavedAt.current = saved.savedAt;
        lastWorkspaceDraftFingerprint.current = `${saved.name}\u0000${JSON.stringify(saved.recipe)}`;
        const persisted: SavedRecipe = { ...saved };
        if (!persisted.migrationReport && migrationReport) persisted.migrationReport = migrationReport;
        if (!persisted.exportOptions && exportOptions) persisted.exportOptions = exportOptions;
        onDraftChange(persisted);
      }
      setRecipeFileStatus(saved
        ? { kind: "success", message: `Receta guardada: ${saved.name}. Los cambios posteriores no se guardan automáticamente.` }
        : { kind: "idle" });
    } catch (error) {
      setRecipeFileStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
    }
  }

  function replaceDraft(loaded: LoadedRecipe) {
    const recipe = loaded.recipe;
    setRenames(recipe.renames.length > 0 ? recipe.renames : [{ from: "", to: "" }]);
    setCasts(recipe.casts.length > 0 ? recipe.casts : [{ column: "", target: "string" }]);
    setDateParses(recipe.dateParses.length > 0 ? recipe.dateParses : [{ column: "", format: "iso8601", target: "date" }]);
    setFilters(recipe.filters);
    setCalculationEnabled(recipe.calculatedColumn !== null);
    setCalculation(recipe.calculatedColumn ?? { name: "", source: "", operation: "add", operand: { kind: "literal", value: "" } });
    setFindReplaceEnabled(recipe.findReplace !== null);
    setFindReplace(recipe.findReplace ?? { scope: "column", column: null, find: "", replace: "" });
    setKeptColumns(recipe.keepColumns ?? dataset.columns.map((column) => column.name));
    setSplitEnabled(recipe.splitColumn !== null);
    setSplit(recipe.splitColumn ?? { source: "", delimiter: "", names: [], dropSource: false });
    setSplitNamesInput(recipe.splitColumn?.names.join(", ") ?? "");
    setMergeEnabled(recipe.mergeColumns !== null);
    setMerge(recipe.mergeColumns ?? { sources: [], name: "", separator: "", dropSources: false });
    setOutlierTreatments(recipe.outlierTreatments);
    setGroupEnabled(recipe.groupSummary !== null);
    setGroupSummary(recipe.groupSummary ?? { groupBy: [], aggregations: [] });
    setContacts(recipe.contactNormalizations);
    setExtractions(recipe.textExtractions);
    setPendingConfirmation(null);
    setRecipeName(loaded.name);
    setMigrationReport(loaded.migrationReport ?? null);
    setExportOptions(loaded.exportOptions ?? null);
    setOperationsOpen(true);
    draftSavedAt.current = loaded.savedAt;
  }

  async function loadRecipeDraft() {
    if (recipeBusy) return;
    setRecipeFileStatus({ kind: "working", action: "load" });
    try {
      const loaded = await pickTransformRecipe();
      if (loaded === null) {
        setRecipeFileStatus({ kind: "idle" });
        return;
      }
      if (!isLoadedRecipe(loaded)) throw new Error("El archivo no contiene una receta compatible con Columnia o DataPrep v1–v3.");
      if (operationCount > 0 && !window.confirm("La receta cargada reemplazará el borrador actual. ¿Deseas continuar?")) {
        setRecipeFileStatus({ kind: "idle" });
        return;
      }
      replaceDraft(loaded);
      acknowledgedRecipeFingerprint.current = null;
      setRecipeFileStatus({ kind: "success", message: `Receta cargada: ${loaded.name}. Revísala antes de aplicarla.` });
    } catch (error) {
      setRecipeFileStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
    }
  }

  return (
    <section className="transform-recipe" aria-labelledby="transform-recipe-title">
      <div className="transform-recipe__intro">
        <div>
          <p className="step">Receta estructural</p>
          <h3 id="transform-recipe-title">Preparar estructura y tipos</h3>
          <p>
            Configura varios cambios y aplícalos juntos. Si una operación no es válida, no se
            modifica ninguna columna.
          </p>
        </div>
        <span aria-live="polite">
          {operationCount === 0
            ? "Aún no hay operaciones"
            : `${operationCount} ${operationCount === 1 ? "operación lista" : "operaciones listas"}`}
        </span>
      </div>

      {operationCount === 0 && (
        <ol className="recipe-steps" aria-label="Cómo aplicar una receta">
          <li><span aria-hidden="true">1</span><strong>Añade</strong><small>Elige una operación</small></li>
          <li><span aria-hidden="true">2</span><strong>Revisa</strong><small>Comprueba sus campos</small></li>
          <li><span aria-hidden="true">3</span><strong>Aplica</strong><small>Ejecuta todo junto</small></li>
        </ol>
      )}

      <div className="recipe-files" aria-label="Archivo de receta">
        <label>
          <span>Nombre de la receta</span>
          <input aria-label="Nombre de la receta" value={recipeName} maxLength={80}
            disabled={recipeBusy}
            onChange={(event) => { setRecipeName(event.target.value); if (recipeFileStatus.kind !== "working") setRecipeFileStatus({ kind: "idle" }); }} />
        </label>
        <button type="button" onClick={saveRecipeDraft}
          disabled={recipeBusy || operationCount === 0 || invalid || !recipeName.trim()}>
          {recipeFileStatus.kind === "working" && recipeFileStatus.action === "save" ? "Guardando…" : "Guardar receta"}
        </button>
        <button type="button" onClick={loadRecipeDraft} disabled={recipeBusy}>
          {recipeFileStatus.kind === "working" && recipeFileStatus.action === "load" ? "Cargando…" : "Cargar receta"}
        </button>
        <p className="recipe-file-status recipe-file-status--hint">
          Acepta recetas Columnia y pipelines DataPrep v1–v3; las operaciones no equivalentes se rechazan para no perder semántica.
        </p>
        {recipeFileStatus.kind === "success" && <p className="recipe-file-status" role="status">{recipeFileStatus.message}</p>}
        {recipeFileStatus.kind === "error" && <p className="recipe-error recipe-file-status" role="alert">No se pudo completar la operación: {recipeFileStatus.message}</p>}
        {migrationReport && <RecipeMigrationReportPanel report={migrationReport} exportOptions={exportOptions} />}
      </div>

      <details
        className="transform-recipe__operations"
        open={operationsOpen}
        onToggle={(event) => setOperationsOpen(event.currentTarget.open)}
      >
        <summary>
          <span>{operationCount === 0 ? "Añadir la primera operación" : "Configurar operaciones"}</span>
          <small>Renombres, tipos, filtros y transformaciones estructurales</small>
        </summary>
      <div className="transform-recipe__grid">
        <RecipeOperationGroup
          title="Renombrar columnas"
          status={operationGroupStatus(activeRenames.length, "renombre", "renombres")}
          active={activeRenames.length > 0}
        >
          {renames.map((rename, index) => (
            <div className="recipe-row recipe-row--rename" key={`rename-${index}`}>
              <label>
                <span>Columna</span>
                <select
                  aria-label={`Columna para renombrar ${index + 1}`}
                  value={rename.from}
                  onChange={(event) =>
                    setRenames((current) =>
                      current.map((item, itemIndex) =>
                        itemIndex === index ? { ...item, from: event.target.value } : item,
                      ),
                    )
                  }
                >
                  <option value="">Selecciona…</option>
                  {columnOptions()}
                </select>
              </label>
              <label>
                <span>Nuevo nombre</span>
                <input
                  aria-label={`Nuevo nombre ${index + 1}`}
                  value={rename.to}
                  onChange={(event) =>
                    setRenames((current) =>
                      current.map((item, itemIndex) =>
                        itemIndex === index ? { ...item, to: event.target.value } : item,
                      ),
                    )
                  }
                />
              </label>
              <button type="button" aria-label={`Quitar renombre ${index + 1}`} title="Quitar renombre" onClick={() => setRenames((current) => current.filter((_, itemIndex) => itemIndex !== index))}>×</button>
            </div>
          ))}
          <button type="button" className="recipe-add" onClick={() => setRenames((current) => [...current, { from: "", to: "" }])}>+ Añadir renombre</button>
        </RecipeOperationGroup>

        <RecipeOperationGroup
          title="Convertir tipos"
          status={operationGroupStatus(activeCasts.length, "conversión", "conversiones")}
          active={activeCasts.length > 0}
        >
          {casts.map((cast, index) => (
            <div className="recipe-row" key={`cast-${index}`}>
              <label>
                <span>Columna</span>
                <select aria-label={`Columna para convertir ${index + 1}`} value={cast.column} onChange={(event) => setCasts((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, column: event.target.value } : item))}>
                  <option value="">Selecciona…</option>
                  {columnOptions()}
                </select>
              </label>
              <label>
                <span>Tipo destino</span>
                <select aria-label={`Tipo destino ${index + 1}`} value={cast.target} onChange={(event) => setCasts((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, target: event.target.value as CastDraft["target"] } : item))}>
                  <option value="string">Texto</option>
                  <option value="integer">Entero</option>
                  <option value="decimal">Decimal</option>
                  <option value="boolean">Booleano</option>
                </select>
              </label>
              <button type="button" aria-label={`Quitar conversión ${index + 1}`} title="Quitar conversión" onClick={() => setCasts((current) => current.filter((_, itemIndex) => itemIndex !== index))}>×</button>
            </div>
          ))}
          <button type="button" className="recipe-add" onClick={() => setCasts((current) => [...current, { column: "", target: "string" }])}>+ Añadir conversión</button>
        </RecipeOperationGroup>

        <RecipeOperationGroup
          title="Interpretar fechas"
          status={operationGroupStatus(activeDateParses.length, "fecha", "fechas")}
          active={activeDateParses.length > 0}
        >
          {dateParses.map((dateParse, index) => (
            <div className="recipe-row recipe-row--date" key={`date-${index}`}>
              <label>
                <span>Columna</span>
                <select aria-label={`Columna de fecha ${index + 1}`} value={dateParse.column} onChange={(event) => setDateParses((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, column: event.target.value } : item))}>
                  <option value="">Selecciona…</option>
                  {columnOptions()}
                </select>
              </label>
              <label>
                <span>Formato origen</span>
                <select aria-label={`Formato de fecha ${index + 1}`} value={dateParse.format} onChange={(event) => setDateParses((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, format: event.target.value as DateDraft["format"] } : item))}>
                  <option value="iso8601">ISO 8601</option>
                  <option value="ymd">AAAA-MM-DD</option>
                  <option value="dmy">DD/MM/AAAA</option>
                  <option value="mdy">MM/DD/AAAA</option>
                </select>
              </label>
              <label>
                <span>Tipo destino</span>
                <select aria-label={`Tipo de fecha destino ${index + 1}`} value={dateParse.target} onChange={(event) => setDateParses((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, target: event.target.value as DateDraft["target"] } : item))}>
                  <option value="date">Fecha</option>
                  <option value="datetime">Fecha y hora</option>
                </select>
              </label>
              <button type="button" aria-label={`Quitar fecha ${index + 1}`} title="Quitar fecha" onClick={() => setDateParses((current) => current.filter((_, itemIndex) => itemIndex !== index))}>×</button>
            </div>
          ))}
          <button type="button" className="recipe-add" onClick={() => setDateParses((current) => [...current, { column: "", format: "iso8601", target: "date" }])}>+ Añadir fecha</button>
        </RecipeOperationGroup>

        <RecipeOperationGroup
          title="Filtrar filas (AND)"
          status={operationGroupStatus(activeFilters.length, "filtro", "filtros")}
          active={activeFilters.length > 0}
        >
          <p className="recipe-hint">
            Todas las condiciones deben cumplirse. Puedes añadir hasta 3 filtros. Mayor que,
            menor que, mayor o igual y menor o igual son comparaciones numéricas estrictas; para
            fechas, extrae primero año, mes o día. La comparación directa de fechas se incorporará
            cuando exista conversión compatible.
          </p>
          {filters.map((filter, index) => {
            const unary = ["is_null", "not_null"].includes(filter.operator);
            return (
              <div className="recipe-row recipe-row--date" key={`filter-${index}`}>
                <label><span>Columna</span><select aria-label={`Columna del filtro ${index + 1}`} value={filter.column} onChange={(event) => setFilters((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, column: event.target.value } : item))}><option value="">Selecciona…</option>{columnOptions()}</select></label>
                <label><span>Condición</span><select aria-label={`Operador del filtro ${index + 1}`} value={filter.operator} onChange={(event) => setFilters((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, operator: event.target.value as FilterDraft["operator"], value: ["is_null", "not_null"].includes(event.target.value) ? null : (item.value ?? "") } : item))}>
                  <option value="eq">Igual a</option><option value="neq">Distinto de</option><option value="gt">Mayor que</option><option value="lt">Menor que</option><option value="gte">Mayor o igual</option><option value="lte">Menor o igual</option><option value="contains">Contiene</option><option value="not_contains">No contiene</option><option value="is_null">Es nulo</option><option value="not_null">No es nulo</option>
                </select></label>
                <label><span>Valor</span><input aria-label={`Valor del filtro ${index + 1}`} value={filter.value ?? ""} disabled={unary} placeholder={unary ? "No requerido" : "Valor estricto"} onChange={(event) => setFilters((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, value: event.target.value } : item))} /></label>
                <button type="button" aria-label={`Quitar filtro ${index + 1}`} title="Quitar filtro" onClick={() => setFilters((current) => current.filter((_, itemIndex) => itemIndex !== index))}>×</button>
              </div>
            );
          })}
          {filters.length < 3 && <button type="button" className="recipe-add" onClick={() => setFilters((current) => [...current, { column: "", operator: "eq", value: "" }])}>+ Añadir filtro AND</button>}
        </RecipeOperationGroup>

        <RecipeOperationGroup
          title="Columna calculada"
          status={calculationEnabled ? "Activa" : "Inactiva"}
          active={calculationEnabled}
        >
          <label className="option-toggle"><input type="checkbox" checked={calculationEnabled} onChange={(event) => setCalculationEnabled(event.target.checked)} />Crear una columna en esta receta</label>
          {calculationEnabled && (
            <div className="calculation-grid">
              <label><span>Nombre nuevo</span><input aria-label="Nombre de la columna calculada" value={calculation.name} onChange={(event) => setCalculation((current) => ({ ...current, name: event.target.value }))} /></label>
              <label><span>Columna origen</span><select aria-label="Columna origen del cálculo" value={calculation.source} onChange={(event) => setCalculation((current) => ({ ...current, source: event.target.value }))}><option value="">Selecciona…</option>{columnOptions()}</select></label>
              <label><span>Operación</span><select aria-label="Operación calculada" value={calculation.operation} onChange={(event) => { const operation = event.target.value as CalculationDraft["operation"]; setCalculation((current) => ({ ...current, operation, operand: ["year", "month", "day"].includes(operation) ? null : (current.operand ?? { kind: "literal", value: "" }) })); }}>
                <option value="add">Sumar</option><option value="subtract">Restar</option><option value="multiply">Multiplicar</option><option value="divide">Dividir</option><option value="concat">Concatenar</option><option value="year">Extraer año</option><option value="month">Extraer mes</option><option value="day">Extraer día</option>
              </select></label>
              {operandRequired && <><label><span>Operando</span><select aria-label="Origen del operando" value={calculation.operand?.kind ?? "literal"} onChange={(event) => setCalculation((current) => ({ ...current, operand: { kind: event.target.value as "literal" | "column", value: "" } }))}><option value="literal">Valor fijo</option><option value="column">Columna</option></select></label>{calculation.operand?.kind === "column" ? <label><span>Columna operando</span><select aria-label="Columna operando" value={calculation.operand.value} onChange={(event) => setCalculation((current) => ({ ...current, operand: { kind: "column", value: event.target.value } }))}><option value="">Selecciona…</option>{columnOptions()}</select></label> : <label><span>Valor fijo</span><input aria-label="Valor fijo del cálculo" value={calculation.operand?.value ?? ""} onChange={(event) => setCalculation((current) => ({ ...current, operand: { kind: "literal", value: event.target.value } }))} /></label>}</>}
            </div>
          )}
          <p className="recipe-hint">La evaluación es estricta: tipos incompatibles o división por cero cancelan toda la receta.</p>
        </RecipeOperationGroup>

        <RecipeOperationGroup
          title="Buscar y reemplazar literal"
          status={findReplaceEnabled ? "Activo" : "Inactivo"}
          active={findReplaceEnabled}
        >
          <label className="option-toggle"><input type="checkbox" checked={findReplaceEnabled} onChange={(event) => setFindReplaceEnabled(event.target.checked)} />Añadir búsqueda y reemplazo</label>
          {findReplaceEnabled && <div className="calculation-grid">
            <label><span>Alcance</span><select aria-label="Alcance de búsqueda" value={findReplace.scope} disabled={searchableTextColumns.length === 0} onChange={(event) => { const scope = event.target.value as FindReplaceDraft["scope"]; setFindReplace((current) => ({ ...current, scope, column: scope === "column" ? current.column : null })); }}><option value="column">Una columna</option><option value="all_text_columns">Todas las columnas de texto</option></select></label>
            {findReplace.scope === "column" && <label><span>Columna</span><select aria-label="Columna para buscar" value={findReplace.column ?? ""} disabled={searchableTextColumns.length === 0} onChange={(event) => setFindReplace((current) => ({ ...current, column: event.target.value || null }))}><option value="">Selecciona…</option>{searchableTextColumns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}</select></label>}
            <label><span>Buscar</span><input aria-label="Texto a buscar" value={findReplace.find} onChange={(event) => setFindReplace((current) => ({ ...current, find: event.target.value }))} /></label>
            <label><span>Reemplazar por</span><input aria-label="Texto de reemplazo" value={findReplace.replace} placeholder="Vacío elimina la coincidencia" onChange={(event) => setFindReplace((current) => ({ ...current, replace: event.target.value }))} /></label>
          </div>}
          <p className="recipe-hint">Busca texto literal, distingue mayúsculas y minúsculas y no interpreta expresiones regulares. Se permite buscar espacios y reemplazar por vacío.</p>
          {searchableTextColumns.length === 0 && <p className="recipe-error">Este dataset no contiene columnas de texto disponibles.</p>}
        </RecipeOperationGroup>

        <RecipeOperationGroup
          title="Columnas a conservar"
          status={dropsColumns ? `${keptColumns.length} de ${dataset.columns.length} columnas` : "Todas las columnas"}
          active={dropsColumns}
        >
          <div className="keep-columns" role="group" aria-label="Seleccionar columnas a conservar">
            {dataset.columns.map((column) => <label key={column.name}><input type="checkbox" checked={keptColumns.includes(column.name)} onChange={(event) => setKeptColumns((current) => event.target.checked ? dataset.columns.map((item) => item.name).filter((name) => current.includes(name) || name === column.name) : current.filter((name) => name !== column.name))} />{column.name}</label>)}
          </div>
          <p className="recipe-hint">Se conserva el orden actual. Debe permanecer al menos una columna.</p>
        </RecipeOperationGroup>

        <RecipeOperationGroup
          title="Dividir columna de texto"
          status={splitEnabled ? "Activa" : "Inactiva"}
          active={splitEnabled}
        >
          <label className="option-toggle"><input type="checkbox" checked={splitEnabled} onChange={(event) => setSplitEnabled(event.target.checked)} />Dividir una columna</label>
          {splitEnabled && <div className="calculation-grid">
            <label><span>Columna origen</span><select aria-label="Columna para dividir" value={split.source} onChange={(event) => setSplit((current) => ({ ...current, source: event.target.value }))}><option value="">Selecciona…</option>{searchableTextColumns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}</select></label>
            <label><span>Delimitador literal</span><input aria-label="Delimitador para dividir" value={split.delimiter} onChange={(event) => setSplit((current) => ({ ...current, delimiter: event.target.value }))} /></label>
            <label className="calculation-grid__wide"><span>Nombres separados por coma</span><input aria-label="Nombres de columnas divididas" value={splitNamesInput} placeholder="nombre, apellido" onChange={(event) => setSplitNamesInput(event.target.value)} /></label>
            <label className="option-toggle"><input type="checkbox" checked={split.dropSource} onChange={(event) => setSplit((current) => ({ ...current, dropSource: event.target.checked }))} />Eliminar columna origen</label>
          </div>}
          <p className="recipe-hint">Define entre 2 y 16 nombres únicos. La última columna recibe el resto; las partes faltantes quedan como null.</p>
        </RecipeOperationGroup>

        <RecipeOperationGroup
          title="Combinar columnas de texto"
          status={mergeEnabled ? "Activa" : "Inactiva"}
          active={mergeEnabled}
        >
          <label className="option-toggle"><input type="checkbox" checked={mergeEnabled} onChange={(event) => setMergeEnabled(event.target.checked)} />Combinar columnas</label>
          {mergeEnabled && <>
            <div className="keep-columns" role="group" aria-label="Columnas para combinar">{searchableTextColumns.map((column) => <label key={column.name}><input type="checkbox" checked={merge.sources.includes(column.name)} onChange={(event) => setMerge((current) => ({ ...current, sources: event.target.checked ? searchableTextColumns.map((item) => item.name).filter((name) => current.sources.includes(name) || name === column.name) : current.sources.filter((name) => name !== column.name) }))} />{column.name}</label>)}</div>
            <div className="calculation-grid"><label><span>Nombre nuevo</span><input aria-label="Nombre de columna combinada" value={merge.name} onChange={(event) => setMerge((current) => ({ ...current, name: event.target.value }))} /></label><label><span>Separador</span><input aria-label="Separador para combinar" value={merge.separator} placeholder="Vacío permitido" onChange={(event) => setMerge((current) => ({ ...current, separator: event.target.value }))} /></label><label className="option-toggle"><input type="checkbox" checked={merge.dropSources} onChange={(event) => setMerge((current) => ({ ...current, dropSources: event.target.checked }))} />Eliminar columnas origen</label></div>
          </>}
          <p className="recipe-hint">Selecciona entre 2 y 16 columnas existentes. Los valores null se omiten; si todos son null, el resultado queda null.</p>
        </RecipeOperationGroup>

        <RecipeOperationGroup
          title="Tratar valores atípicos"
          status={operationGroupStatus(outlierTreatments.length, "tratamiento", "tratamientos")}
          active={outlierTreatments.length > 0}
        >
          <p className="recipe-hint">Usa límites IQR de 1.5 con al menos 4 valores finitos. Los null se preservan; valores no finitos cancelan toda la receta. Imputar usa la mediana y conserva enteros; limitar puede convertirlos a decimal y rechaza enteros fuera del rango exacto ±2^53 para evitar pérdida de precisión.</p>
          {outlierTreatments.map((treatment, index) => <div className="recipe-row" key={`outlier-${index}`}>
            <label><span>Columna numérica</span><select aria-label={`Columna de outliers ${index + 1}`} value={treatment.column} onChange={(event) => setOutlierTreatments((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, column: event.target.value } : item))}><option value="">Selecciona…</option>{numericColumns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}</select></label>
            <label><span>Acción</span><select aria-label={`Acción de outliers ${index + 1}`} value={treatment.action} onChange={(event) => setOutlierTreatments((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, action: event.target.value as OutlierDraft["action"] } : item))}><option value="cap">Limitar a los umbrales</option><option value="impute">Reemplazar por la mediana</option><option value="drop">Eliminar filas</option></select></label>
            <button type="button" aria-label={`Quitar tratamiento ${index + 1}`} title="Quitar tratamiento" onClick={() => setOutlierTreatments((current) => current.filter((_, itemIndex) => itemIndex !== index))}>×</button>
          </div>)}
          {outlierTreatments.length < 16 && <button type="button" className="recipe-add" onClick={() => setOutlierTreatments((current) => [...current, { column: "", action: "cap" }])}>+ Añadir tratamiento</button>}
        </RecipeOperationGroup>

        <RecipeOperationGroup
          title="Resumen agrupado"
          status={groupEnabled
            ? `${groupSummary.groupBy.length} claves, ${groupSummary.aggregations.length} cálculos`
            : "Inactivo"}
          active={groupEnabled}
        >
          <label className="option-toggle"><input type="checkbox" checked={groupEnabled} onChange={(event) => setGroupEnabled(event.target.checked)} />Reemplazar el dataset por un resumen</label>
          {groupEnabled && <>
            <div className="keep-columns" role="group" aria-label="Columnas para agrupar">{dataset.columns.map((column) => <label key={column.name}><input type="checkbox" checked={groupSummary.groupBy.includes(column.name)} disabled={!groupSummary.groupBy.includes(column.name) && groupSummary.groupBy.length >= 8} onChange={(event) => setGroupSummary((current) => ({ ...current, groupBy: event.target.checked ? dataset.columns.map((item) => item.name).filter((name) => current.groupBy.includes(name) || name === column.name) : current.groupBy.filter((name) => name !== column.name) }))} />{effectiveName(column.name)}</label>)}</div>
            {groupSummary.aggregations.map((aggregation, index) => {
              const dtype = effectiveType(aggregation.column);
              const numeric = ["Int64", "Float64"].includes(dtype);
              const orderable = numeric || ["String", "Date", "Datetime"].includes(dtype);
              return <div className="recipe-row" key={`aggregation-${index}`}><label><span>Columna</span><select aria-label={`Columna de agregación ${index + 1}`} value={aggregation.column} onChange={(event) => setGroupSummary((current) => ({ ...current, aggregations: current.aggregations.map((item, itemIndex) => itemIndex === index ? { ...item, column: event.target.value, operation: "count" } : item) }))}><option value="">Selecciona…</option>{dataset.columns.map((column) => <option key={column.name} value={column.name}>{effectiveName(column.name)}</option>)}</select></label><label><span>Operación</span><select aria-label={`Operación de agregación ${index + 1}`} value={aggregation.operation} onChange={(event) => setGroupSummary((current) => ({ ...current, aggregations: current.aggregations.map((item, itemIndex) => itemIndex === index ? { ...item, operation: event.target.value as GroupSummaryDraft["aggregations"][number]["operation"] } : item) }))}>{numeric && <><option value="sum">Suma</option><option value="mean">Promedio</option></>}{orderable && <><option value="min">Mínimo</option><option value="max">Máximo</option></>}<option value="count">Contar filas</option><option value="count_unique">Contar únicos</option></select></label><span className="recipe-output" aria-label={`Salida ${index + 1}`}>{aggregation.column ? `${effectiveName(aggregation.column)}_${aggregation.operation}` : "—"}</span><button type="button" aria-label={`Quitar agregación ${index + 1}`} onClick={() => setGroupSummary((current) => ({ ...current, aggregations: current.aggregations.filter((_, itemIndex) => itemIndex !== index) }))}>×</button></div>;
            })}
            {groupSummary.aggregations.length < 32 && <button type="button" className="recipe-add" onClick={() => setGroupSummary((current) => ({ ...current, aggregations: [...current.aggregations, { column: "", operation: "count" }] }))}>+ Añadir agregación</button>}
          </>}
          <p className="recipe-hint">Los grupos null forman un grupo propio. Contar filas incluye null; contar únicos excluye null. Se conserva el orden de primera aparición y el resumen reemplaza la granularidad actual.</p>
        </RecipeOperationGroup>

        <RecipeOperationGroup
          title="Normalizar datos de contacto"
          status={operationGroupStatus(contacts.length, "regla", "reglas")}
          active={contacts.length > 0}
        >
          {contacts.map((contact, index) => <div className="recipe-row" key={`contact-${index}`}><label><span>Columna de texto</span><select aria-label={`Columna de contacto ${index + 1}`} value={contact.column} onChange={(event) => setContacts((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, column: event.target.value } : item))}><option value="">Selecciona…</option>{searchableTextColumns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}</select></label><label><span>Regla</span><select aria-label={`Regla de contacto ${index + 1}`} value={contact.kind} onChange={(event) => setContacts((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, kind: event.target.value as ContactDraft["kind"] } : item))}><option value="email">Email: recortar y minúsculas</option><option value="phone">Teléfono: + opcional y dígitos ASCII</option><option value="address">Dirección: compactar espacios</option></select></label><button type="button" aria-label={`Quitar contacto ${index + 1}`} onClick={() => setContacts((current) => current.filter((_, itemIndex) => itemIndex !== index))}>×</button></div>)}
          {contacts.length < 16 && <button type="button" className="recipe-add" onClick={() => setContacts((current) => [...current, { column: "", kind: "email" }])}>+ Añadir contacto</button>}
          <p className="recipe-hint">Email recorta y pasa a minúsculas; teléfono conserva un + inicial opcional y dígitos ASCII; dirección compacta espacios sin aplicar título.</p>
        </RecipeOperationGroup>

        <RecipeOperationGroup
          title="Extraer texto"
          status={operationGroupStatus(extractions.length, "extracción", "extracciones")}
          active={extractions.length > 0}
        >
          {extractions.map((extraction, index) => <div className="recipe-row recipe-row--date" key={`extraction-${index}`}><label><span>Origen</span><select aria-label={`Columna de extracción ${index + 1}`} value={extraction.source} onChange={(event) => setExtractions((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, source: event.target.value } : item))}><option value="">Selecciona…</option>{searchableTextColumns.map((column) => <option key={column.name} value={column.name}>{column.name}</option>)}</select></label><label><span>Extracción</span><select aria-label={`Regla de extracción ${index + 1}`} value={extraction.kind} onChange={(event) => { const kind = event.target.value as ExtractionDraft["kind"]; setExtractions((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, kind, delimiter: ["before", "after"].includes(kind) ? "" : null } : item)); }}><option value="first_token">Primer token</option><option value="last_token">Último token</option><option value="digits">Dígitos</option><option value="letters">Letras</option><option value="before">Antes de delimitador</option><option value="after">Después de delimitador</option></select></label><label><span>Nombre nuevo</span><input aria-label={`Nombre de extracción ${index + 1}`} value={extraction.name} onChange={(event) => setExtractions((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, name: event.target.value } : item))} /></label>{["before", "after"].includes(extraction.kind) && <label><span>Delimitador literal</span><input aria-label={`Delimitador de extracción ${index + 1}`} value={extraction.delimiter ?? ""} onChange={(event) => setExtractions((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, delimiter: event.target.value } : item))} /></label>}<button type="button" aria-label={`Quitar extracción ${index + 1}`} onClick={() => setExtractions((current) => current.filter((_, itemIndex) => itemIndex !== index))}>×</button></div>)}
          {extractions.length < 16 && <button type="button" className="recipe-add" disabled={groupEnabled} onClick={() => setExtractions((current) => [...current, { source: "", kind: "first_token", name: "", delimiter: null }])}>+ Añadir extracción</button>}
          <p className="recipe-hint">Las extracciones crean columnas nuevas desde entradas originales. Antes/después requiere delimitador literal no vacío; se permiten espacios.</p>
          {groupEnabled && <p className="recipe-error">Las extracciones no son compatibles con un resumen agrupado en la misma receta; las normalizaciones de contacto sí.</p>}
        </RecipeOperationGroup>
      </div>
      </details>

      {transformPreview && <TransformAdvisor preview={transformPreview} />}

      {renameInvalid && <p className="recipe-error" role="alert">Renombres: completa la columna y su nombre nuevo.</p>}
      {filterInvalid && <p className="recipe-error" role="alert">Filtros: las comparaciones numéricas y de contenido requieren un valor.</p>}
      {calculationInvalid && <p className="recipe-error" role="alert">Columna calculada: completa el nombre, el origen y el operando requerido.</p>}
      {calculationSourceDropped && <p className="recipe-error" role="alert">Columna calculada: conserva la columna origen y la columna usada como operando.</p>}
      {findReplaceInvalid && <p className="recipe-error" role="alert">Buscar y reemplazar: selecciona el alcance y escribe un texto de búsqueda; el reemplazo puede quedar vacío.</p>}
      {keptColumns.length === 0 && <p className="recipe-error" role="alert">Columnas: conserva al menos una columna.</p>}
      {splitInvalid && <p className="recipe-error" role="alert">Dividir: selecciona una columna, un delimitador no vacío y entre 2 y 16 nombres únicos que no colisionen.</p>}
      {mergeInvalid && <p className="recipe-error" role="alert">Combinar: selecciona entre 2 y 16 columnas y usa un nombre nuevo sin colisiones.</p>}
      {sourceConflict && <p className="recipe-error" role="alert">Dependencias: no elimines al dividir una columna que también usarás para combinar.</p>}
      {sourceNotKept && <p className="recipe-error" role="alert">Dependencias: conserva todas las columnas usadas para dividir o combinar.</p>}
      {outlierDuplicate && <p className="recipe-error" role="alert">Outliers: configura una sola acción por columna.</p>}
      {outlierDependencyInvalid && <p className="recipe-error" role="alert">Outliers: conserva cada columna objetivo y no la elimines como fuente antes del tratamiento.</p>}
      {outlierInvalid && !outlierDuplicate && !outlierDependencyInvalid && <p className="recipe-error" role="alert">Outliers: selecciona únicamente columnas numéricas elegibles.</p>}
      {groupInvalid && <p className="recipe-error" role="alert">Resumen: elige entre 1 y 8 claves, agrega al menos una operación, evita pares o salidas duplicadas y conserva todas las columnas utilizadas.</p>}
      {contactDuplicate && <p className="recipe-error" role="alert">Contactos: configura una sola regla por columna.</p>}
      {contactInvalid && !contactDuplicate && <p className="recipe-error" role="alert">Contactos: usa columnas de texto que sobrevivan a la receta.</p>}
      {extractionInvalid && <p className="recipe-error" role="alert">Extracciones: completa entradas y nombres únicos sin colisiones, conserva sus fuentes y no las combines con un resumen.</p>}
      <div className="transform-recipe__footer">
        <p>
          Toda la receta referencia los nombres actuales. Booleano acepta únicamente true/false;
          decimal usa punto y las fechas ambiguas requieren formato explícito.
        </p>
        <button
          type="button"
          className="primary-action"
          aria-label="Aplicar receta"
          onClick={submitRecipe}
          disabled={recipeBusy || operationCount === 0 || invalid}
        >
          {busy
            ? "Aplicando receta…"
            : operationCount === 0
              ? "Aplicar receta"
              : operationCount === 1 ? "Aplicar 1 operación" : `Aplicar ${operationCount} operaciones`}
        </button>
      </div>
      {pendingConfirmation && (
        <ModalDialog
          role="alertdialog"
          labelledBy="filter-confirm-title"
          describedBy="filter-confirm-description"
          onDismiss={() => setPendingConfirmation(null)}
        >
            <p className="step">Cambio de alto impacto</p>
            <h3 id="filter-confirm-title">Confirmar cambios de alto impacto</h3>
            <p id="filter-confirm-description">
              {pendingConfirmation.filters.length > 0 && <>La receta aplicará {pendingConfirmation.filters.length} filtros unidos por AND sobre {dataset.rowCount.toLocaleString()} filas actuales. El número final de filas depende de los datos. </>}
              {(pendingConfirmation.keepColumns !== null || pendingConfirmation.splitColumn?.dropSource || pendingConfirmation.mergeColumns?.dropSources) && <> En total se eliminarán {new Set([...(pendingConfirmation.keepColumns ? dataset.columns.map((column) => column.name).filter((name) => !pendingConfirmation.keepColumns?.includes(name)) : []), ...(pendingConfirmation.splitColumn?.dropSource ? [pendingConfirmation.splitColumn.source] : []), ...(pendingConfirmation.mergeColumns?.dropSources ? pendingConfirmation.mergeColumns.sources : [])]).size} columnas originales, sin contar dos veces las fuentes compartidas.</>}
              {pendingConfirmation.outlierTreatments.some((item) => item.action === "cap") && <> Se limitarán valores atípicos en {pendingConfirmation.outlierTreatments.filter((item) => item.action === "cap").length} columnas.</>}
              {pendingConfirmation.outlierTreatments.some((item) => item.action === "impute") && <> Se reemplazarán valores atípicos por la mediana en {pendingConfirmation.outlierTreatments.filter((item) => item.action === "impute").length} columnas.</>}
              {pendingConfirmation.outlierTreatments.some((item) => item.action === "drop") && <> Se podrán eliminar filas atípicas detectadas en {pendingConfirmation.outlierTreatments.filter((item) => item.action === "drop").length} columnas.</>}
              {pendingConfirmation.groupSummary && <> El dataset será reemplazado por un resumen de {pendingConfirmation.groupSummary.groupBy.length} claves y {pendingConfirmation.groupSummary.aggregations.length} agregaciones sobre {dataset.rowCount.toLocaleString()} filas actuales.</>}
              {pendingConfirmation.contactNormalizations.length > 0 && <> Se normalizarán valores de contacto en {pendingConfirmation.contactNormalizations.length} columnas.</>}
            </p>
            <div className="sheet-dialog__actions"><button type="button" onClick={() => setPendingConfirmation(null)}>Cancelar</button><button type="button" className="primary-action" onClick={() => { const recipe = pendingConfirmation; setPendingConfirmation(null); onApply(recipe); }}>Confirmar y aplicar</button></div>
        </ModalDialog>
      )}
    </section>
  );
}
