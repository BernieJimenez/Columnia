import { plural } from "../../plural";
import { isCancellationError } from "../../bridge/cancellation";
import { useRef, useState } from "react";

import {
  applySafeCorrections,
  applyTransformRecipe,
  capOutlierValues,
  dropOutlierValues,
  enableRowAudit,
  fixEncodingValues,
  getHistoryState,
  imputeCategoricalValues,
  imputeMissingValues,
  imputeOutlierValues,
  nullifyInvalidTypeValues,
  normalizeColumnNames,
  normalizeBooleanValues,
  normalizeTextValues,
  parseDateValues,
  castNumericValues,
  cancelOperation,
  maskPersonalValues,
  removeConstantColumns,
  removeEmptyRows,
  removeEmptyColumns,
  removeHighNullColumns,
  removeIdentifierColumns,
  removePersonalColumns,
  removeDuplicates,
  removeNearDuplicates,
  redoLastChange,
  trimTextValues,
  undoLastChange,
  type DatasetPreview,
  type SafeCorrectionOptions,
  type ReusableTaskExceptionPolicy,
  type TransformRecipe,
} from "../../bridge";
import { EMPTY_HISTORY, appliedPlanChanges, nullifiedCellsSentence, type ChangeStatus } from "./prepareModel";
import { isRowAuditColumn } from "../../rowAudit";
import { errorMessage } from "../../bridge/errors";

interface PrepareControllerOptions {
  activeDataset: DatasetPreview | null;
  exceptionPolicy?: ReusableTaskExceptionPolicy | null;
  onDatasetChanged: (dataset: DatasetPreview) => void;
  onProfileInvalidated: () => void;
  onDeliveryInvalidated: () => void;
}

type RecipeResultCounts = Awaited<ReturnType<typeof applyTransformRecipe>>;

/**
 * What a recipe did, only the figures that moved: «3 renombres y 120 filas
 * filtradas», never a list of zeros.
 */
function recipeResultParts(result: Omit<RecipeResultCounts, "dataset">): string {
  const contacts = result.normalizedContactCellCount > 0
    ? `${plural(result.normalizedContactCellCount, "contacto normalizado", "contactos normalizados")} en ${plural(result.normalizedContactColumnCount, "columna", "columnas")}`
    : null;
  const summary = result.aggregatedColumnCount > 0 || result.collapsedRowCount > 0
    ? `resumen de ${plural(result.groupCount, "grupo", "grupos")} con ${plural(result.aggregatedColumnCount, "agregación", "agregaciones")}`
    : null;
  const parts = [
    [result.renamedColumnCount, "renombre", "renombres"],
    [result.convertedColumnCount, "conversión", "conversiones"],
    [result.parsedDateColumnCount, "fecha interpretada", "fechas interpretadas"],
    [result.removedRowCount, "fila filtrada", "filas filtradas"],
    [result.outlierRemovedRowCount, "fila atípica eliminada", "filas atípicas eliminadas"],
    [result.calculatedColumnCount, "columna calculada", "columnas calculadas"],
    [result.replacedCellCount, "celda reemplazada", "celdas reemplazadas"],
    [result.splitColumnCount, "columna dividida", "columnas divididas"],
    [result.mergedColumnCount, "columna combinada", "columnas combinadas"],
    [result.droppedColumnCount + result.droppedSourceColumnCount, "columna descartada", "columnas descartadas"],
    [result.adjustedOutlierCellCount, "valor atípico ajustado", "valores atípicos ajustados"],
  ] as const;
  const extracted = result.extractedColumnCount > 0
    ? plural(result.extractedColumnCount, "columna extraída", "columnas extraídas")
    : null;
  const listed = [
    ...parts.filter(([count]) => count > 0).map(([count, one, many]) => plural(count, one, many)),
    ...[contacts, extracted, summary].filter((part): part is string => part !== null),
  ];
  return listed.length <= 1 ? listed.join("") : `${listed.slice(0, -1).join(", ")} y ${listed.at(-1)}`;
}

export function usePrepareController({
  activeDataset,
  exceptionPolicy = null,
  onDatasetChanged,
  onProfileInvalidated,
  onDeliveryInvalidated,
}: PrepareControllerOptions) {
  const [changeStatus, setChangeStatus] = useState<ChangeStatus>({ kind: "idle" });
  const [historyStatus, setHistoryStatus] = useState(EMPTY_HISTORY);
  const operationInFlight = useRef(false);
  const cancelRequested = useRef(false);

  function runExclusive<TArgs extends unknown[]>(operation: (...args: TArgs) => Promise<void>) {
    return async (...args: TArgs) => {
      if (operationInFlight.current) return;
      operationInFlight.current = true;
      cancelRequested.current = false;
      try {
        await operation(...args);
      } finally {
        operationInFlight.current = false;
        cancelRequested.current = false;
      }
    };
  }

  function cancelCurrent() {
    if (!operationInFlight.current || cancelRequested.current) return;
    cancelRequested.current = true;
    setChangeStatus((current) => current.kind === "working"
      ? { ...current, cancelRequested: true }
      : current);
    void cancelOperation("prepare").catch(() => {
      cancelRequested.current = false;
      setChangeStatus((current) => current.kind === "working"
        ? { ...current, cancelRequested: false }
        : current);
    });
  }

  function changeFailureStatus(error: unknown): ChangeStatus {
    const message = errorMessage(error);
    return isCancellationError(error)
      ? { kind: "cancelled", message: "Preparación cancelada. El dataset anterior sigue activo." }
      : { kind: "error", message };
  }

  async function refreshHistory() {
    try {
      const history = await getHistoryState();
      setHistoryStatus(history);
      return history;
    } catch {
      return historyStatus;
    }
  }

  function resetChangeStatus() {
    setChangeStatus({ kind: "idle" });
  }

  /**
   * A change published outside the Preparar controller (joining or
   * consolidating with another file) shows the same result line, whose
   * «Deshacer» depends on the history like any other change.
   */
  function reportExternalChange(message: string) {
    setChangeStatus({ kind: "applied", message });
  }

  /**
   * The result of a change. One that touched nothing wrote no version (the
   * engine skips it), so the line offers no «Deshacer», which would undo the
   * change before it, and Revisar and Entregar keep what they had.
   */
  async function settleChange(
    dataset: DatasetPreview,
    changed: boolean,
    status: { message: string; changes?: string[] },
  ) {
    if (changed) {
      onDatasetChanged(dataset);
      onProfileInvalidated();
    }
    setChangeStatus({ kind: "applied", ...status, ...(changed ? {} : { unchanged: true }) });
    await refreshHistory();
    if (changed) onDeliveryInvalidated();
  }

  async function applyDuplicateRemoval() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "duplicates" });
    try {
      const result = await removeDuplicates();
      await settleChange(result.dataset, result.affectedRowCount > 0, {
        message: `${result.affectedRowCount === 1 ? "Se eliminó" : "Se eliminaron"} ${plural(result.affectedRowCount, "fila duplicada adicional", "filas duplicadas adicionales")}.`,
      });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyNearDuplicateRemoval() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "near_duplicates" });
    try {
      const result = await removeNearDuplicates();
      await settleChange(result.dataset, result.affectedRowCount > 0, {
        message: result.affectedRowCount === 0
          ? "No se detectaron duplicados parecidos adicionales."
          : `${result.affectedRowCount === 1 ? "Se eliminó" : "Se eliminaron"} ${plural(result.affectedRowCount, "fila duplicada parecida", "filas duplicadas parecidas")}. La primera fila de cada grupo y las copias exactas se conservaron.`,
      });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyEmptyRowRemoval() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "empty_rows" });
    try {
      const result = await removeEmptyRows();
      await settleChange(result.dataset, result.affectedRowCount > 0, {
        message: result.affectedRowCount === 0
          ? "No se detectaron filas completamente vacías."
          : `${result.affectedRowCount === 1 ? "Se eliminó" : "Se eliminaron"} ${plural(result.affectedRowCount, "fila completamente vacía", "filas completamente vacías")}.`,
      });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyConstantColumnRemoval() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "constant_columns" });
    try {
      const result = await removeConstantColumns();
      await settleChange(result.dataset, result.removedColumnCount > 0, {
        message: result.removedColumnCount === 0
          ? "No se eliminaron columnas constantes; se conserva al menos una columna del dataset."
          : `${result.removedColumnCount === 1 ? "Se eliminó" : "Se eliminaron"} ${plural(result.removedColumnCount, "columna constante", "columnas constantes")}: ${result.removedColumns.join(", ")}.`,
      });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyEmptyColumnRemoval() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "empty_columns" });
    try {
      const result = await removeEmptyColumns();
      await settleChange(result.dataset, result.removedColumnCount > 0, {
        message: result.removedColumnCount === 0
          ? "No se eliminaron columnas vacías; se conserva al menos una columna del dataset."
          : `${result.removedColumnCount === 1 ? "Se eliminó" : "Se eliminaron"} ${plural(result.removedColumnCount, "columna completamente vacía", "columnas completamente vacías")}: ${result.removedColumns.join(", ")}.`,
      });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyHighNullColumnRemoval() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "high_null_columns" });
    try {
      const result = await removeHighNullColumns();
      await settleChange(result.dataset, result.removedColumnCount > 0, {
        message: result.removedColumnCount === 0
          ? `No se detectaron columnas con al menos ${result.thresholdPercentage ?? 80} % de valores nulos.`
          : `${result.removedColumnCount === 1 ? "Se eliminó" : "Se eliminaron"} ${plural(result.removedColumnCount, "columna con alta nulidad", "columnas con alta nulidad")}: ${result.removedColumns.join(", ")}.`,
      });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyIdentifierColumnRemoval() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "identifier_columns" });
    try {
      const result = await removeIdentifierColumns();
      await settleChange(result.dataset, result.removedColumnCount > 0, {
        message: result.removedColumnCount === 0
          ? "No se detectaron columnas identificadoras para retirar; se conserva al menos una columna del dataset."
          : `${result.removedColumnCount === 1 ? "Se retiró" : "Se retiraron"} ${plural(result.removedColumnCount, "columna identificadora", "columnas identificadoras")}: ${result.removedColumns.join(", ")}. La operación puede revertirse desde el historial.`,
      });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyPersonalColumnRemoval() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "personal_columns" });
    try {
      const result = await removePersonalColumns();
      await settleChange(result.dataset, result.removedColumnCount > 0, {
        message: result.removedColumnCount === 0
          ? "No se detectaron columnas de datos personales para retirar; se conserva al menos una columna del dataset."
          : `${result.removedColumnCount === 1 ? "Se retiró" : "Se retiraron"} ${plural(result.removedColumnCount, "columna de datos personales", "columnas de datos personales")}. No se muestran nombres ni valores. La operación puede revertirse desde el historial.`,
      });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyPersonalValueMasking() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "personal_mask" });
    try {
      const result = await maskPersonalValues();
      const cells = result.changedCellCount === 1
        ? "1 valor"
        : `${result.changedCellCount.toLocaleString("es-ES")} valores`;
      const columns = result.changedColumnCount === 1
        ? "1 columna"
        : `${result.changedColumnCount.toLocaleString("es-ES")} columnas`;
      await settleChange(result.dataset, result.changedCellCount > 0, {
        message: result.changedCellCount === 0
          ? "No se encontraron valores personales no nulos que proteger."
          : `Se ocultaron ${cells} en ${columns}: ahora dicen [REDACTED]. No se muestran nombres ni valores; el cambio puede revertirse desde el historial.`,
      });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyBooleanNormalization() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "booleans" });
    try {
      const result = await normalizeBooleanValues();
      const columns = result.changedColumns.map((column) => column.name).join(", ");
      await settleChange(result.dataset, result.changedCellCount > 0, {
        message: result.changedCellCount === 0
          ? "No se encontraron alias booleanos que necesitaran normalización."
          : `${result.changedCellCount === 1 ? "Se normalizó" : "Se normalizaron"} ${plural(result.changedCellCount, "valor booleano", "valores booleanos")} en: ${columns}.`,
      });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyDateParsing() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "parse_dates" });
    try {
      const result = await parseDateValues();
      const columns = result.changedColumns.map((column) => column.name).join(", ");
      await settleChange(result.dataset, result.changedCellCount > 0, {
        message: result.changedCellCount === 0
          ? "No se detectaron columnas de texto con un formato de fecha dominante y seguro."
          : `${result.changedCellCount === 1 ? "Se interpretó" : "Se interpretaron"} ${plural(result.changedCellCount, "valor de fecha", "valores de fecha")} en: ${columns}.${nullifiedCellsSentence(result.changedColumns)} Las columnas ambiguas se dejaron intactas y el cambio puede revertirse desde el historial.`,
      });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyEncodingFix() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "encoding" });
    try {
      const result = await fixEncodingValues();
      const detail = result.changedCellCount === 1
        ? "1 celda"
        : `${result.changedCellCount.toLocaleString("es-ES")} celdas`;
      await settleChange(result.dataset, result.changedCellCount > 0, {
        message: result.changedCellCount === 0
          ? "No se detectaron acentos mal leídos."
          : `Se corrigieron acentos mal leídos en ${detail}.`,
      });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyNumericCast() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "cast_numeric" });
    try {
      const result = await castNumericValues();
      const columns = result.changedColumns.map((column) => column.name).join(", ");
      await settleChange(result.dataset, result.changedCellCount > 0, {
        message: result.changedCellCount === 0
          ? "No se detectaron columnas de texto numéricas seguras para convertir."
          : `${result.changedCellCount === 1 ? "Se convirtió" : "Se convirtieron"} ${plural(result.changedCellCount, "valor numérico", "valores numéricos")} en: ${columns}.${nullifiedCellsSentence(result.changedColumns)} Los identificadores y códigos con ceros iniciales se conservaron; el cambio puede revertirse desde el historial.`,
      });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyInvalidTypeCleanup() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "invalid_types" });
    try {
      const result = await nullifyInvalidTypeValues();
      const detail = result.changedCellCount === 1
        ? "1 valor"
        : `${result.changedCellCount.toLocaleString("es-ES")} valores`;
      await settleChange(result.dataset, result.changedCellCount > 0, {
        message: result.changedCellCount === 0
          ? "No se encontraron valores incompatibles con un tipo sugerido con confianza."
          : result.changedCellCount === 1
            ? `Se apartó ${detail} incompatible como nulo; el cambio puede revertirse desde el historial.`
            : `Se apartaron ${detail} incompatibles como nulos; el cambio puede revertirse desde el historial.`,
      });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyMissingValueImputation() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "impute" });
    try {
      const result = await imputeMissingValues();
      const columns = result.changedColumns.map((column) => column.name).join(", ");
      await settleChange(result.dataset, result.changedCellCount > 0, {
        message: result.changedCellCount === 0
          ? "No se encontraron nulos imputables con una señal conservadora."
          : `${result.changedCellCount === 1 ? "Se imputó" : "Se imputaron"} ${plural(result.changedCellCount, "valor nulo", "valores nulos")} en: ${columns}.`,
      });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyOutlierImputation() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "outlier_impute" });
    try {
      const result = await imputeOutlierValues();
      const columns = result.changedColumns.map((column) => column.name).join(", ");
      await settleChange(result.dataset, result.changedCellCount > 0, {
        message: result.changedCellCount === 0
          ? "No se detectaron valores atípicos que necesitaran reemplazo."
          : `${result.changedCellCount === 1 ? "Se reemplazó" : "Se reemplazaron"} ${plural(result.changedCellCount, "valor atípico", "valores atípicos")} por la mediana en: ${columns}. El cambio puede revertirse desde el historial.`,
      });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyOutlierTreatment(action: "cap" | "drop") {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: action === "cap" ? "outlier_cap" : "outlier_drop" });
    try {
      const result = action === "cap"
        ? await capOutlierValues()
        : await dropOutlierValues();
      const columns = result.changedColumns.map((column) => column.name).join(", ");
      await settleChange(result.dataset, (action === "cap" ? result.changedCellCount : result.affectedRowCount) > 0, {
        message: action === "cap"
          ? result.changedCellCount === 0
            ? "No se detectaron valores atípicos que necesitaran limitación."
            : `${result.changedCellCount === 1 ? "Se limitó" : "Se limitaron"} ${plural(result.changedCellCount, "valor atípico", "valores atípicos")} al límite del rango normal (regla IQR) en: ${columns}. El cambio puede revertirse desde el historial.`
          : result.affectedRowCount === 0
            ? "No se detectaron filas atípicas para eliminar."
            : `${result.affectedRowCount === 1 ? "Se eliminó" : "Se eliminaron"} ${plural(result.affectedRowCount, "fila atípica", "filas atípicas")} fuera del rango normal (regla IQR). El cambio puede revertirse desde el historial.`,
      });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyCategoricalImputation() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "categorical_impute" });
    try {
      const result = await imputeCategoricalValues();
      const columns = result.changedColumns.map((column) => column.name).join(", ");
      await settleChange(result.dataset, result.changedCellCount > 0, {
        message: result.changedCellCount === 0
          ? "No se encontraron nulos textuales para completar como Desconocido."
          : `${result.changedCellCount === 1 ? "Se completó" : "Se completaron"} ${plural(result.changedCellCount, "nulo textual", "nulos textuales")} como Desconocido en: ${columns}. El cambio puede revertirse desde el historial.`,
      });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyRowAudit() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "audit" });
    try {
      const result = await enableRowAudit();
      const enabled = result.dataset.columns.some((column) => isRowAuditColumn(column.name));
      await settleChange(result.dataset, enabled, {
        message: enabled
          ? "La trazabilidad por fila está activa; los cambios futuros se anotarán en _cambios."
          : "La trazabilidad por fila no produjo cambios.",
      });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyColumnNormalization() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "columns" });
    try {
      const result = await normalizeColumnNames();
      await settleChange(result.dataset, result.renamedColumnCount > 0, {
        message:
          result.renamedColumnCount === 0
            ? "Los nombres de las columnas ya estaban normalizados."
            : result.renamedColumnCount === 1
              ? "Se normalizó 1 nombre de columna."
              : `${result.renamedColumnCount === 1 ? "Se normalizó" : "Se normalizaron"} ${plural(result.renamedColumnCount, "nombre de columna", "nombres de columnas")}.`,
      });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyTextChange(action: "trim" | "text", columns: string[] = [], removeAccents = true) {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action });
    try {
      const result = action === "trim"
        ? await trimTextValues()
        : await normalizeTextValues(columns, removeAccents);
      const cells = result.changedCellCount === 1
        ? "1 celda"
        : `${result.changedCellCount.toLocaleString("es-ES")} celdas`;
      const rows = result.affectedRowCount === 1
        ? "1 fila"
        : `${result.affectedRowCount.toLocaleString("es-ES")} filas`;
      const detail = `${cells} en ${rows}`;
      await settleChange(result.dataset, result.changedCellCount > 0, {
        message:
          result.changedCellCount === 0
            ? "No se encontraron valores que necesitaran esta corrección."
            : action === "trim"
              ? `Se recortaron espacios en ${detail}.`
              : `Se normalizó texto en ${detail}.`,
      });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyRecommendedCorrections(options: SafeCorrectionOptions) {
    if (activeDataset === null) return;
    const castsColumns = (options.castColumns?.length ?? 0) > 0 || (options.dateColumns?.length ?? 0) > 0;
    if (!options.trimText && !options.normalizeSentinels && !options.normalizeColumnNames && !options.removeDuplicates && !options.imputeMissing && !castsColumns) return;
    setChangeStatus({ kind: "working", action: "safe" });
    try {
      const result = await applySafeCorrections(options, (progress) =>
        setChangeStatus((current) => (current.kind === "working" ? { ...current, progress } : current)),
      );
      const imputedCellCount = result.imputedCellCount;
      const changed = result.changedCellCount > 0
        || result.removedRowCount > 0
        || result.renamedColumnCount > 0
        || result.typedColumnCount > 0
        || result.datedColumnCount > 0
        || imputedCellCount > 0;
      if (changed) {
        onDatasetChanged(result.dataset);
        onProfileInvalidated();
      }
      const changedCells = result.changedCellCount === 1
        ? "1 celda actualizada"
        : `${result.changedCellCount.toLocaleString("es-ES")} celdas actualizadas`;
      const renamedColumns = result.renamedColumnCount === 1
        ? "1 columna renombrada"
        : `${result.renamedColumnCount.toLocaleString("es-ES")} columnas renombradas`;
      const changes = [
        (options.trimText || options.normalizeSentinels) && result.changedCellCount > 0 ? changedCells : null,
        options.normalizeColumnNames && result.renamedColumnCount > 0 ? renamedColumns : null,
        options.removeDuplicates && result.removedRowCount > 0
          ? `${result.removedRowCount === 1 ? "se retiró" : "se retiraron"} ${plural(result.removedRowCount, "fila duplicada exacta", "filas duplicadas exactas")}`
          : null,
        result.typedColumnCount > 0
          ? `${result.typedColumnCount.toLocaleString("es-ES")} ${result.typedColumnCount === 1 ? "columna convertida" : "columnas convertidas"} a número`
          : null,
        result.datedColumnCount > 0
          ? `${result.datedColumnCount.toLocaleString("es-ES")} ${result.datedColumnCount === 1 ? "columna convertida" : "columnas convertidas"} a fecha`
          : null,
        options.imputeMissing && imputedCellCount > 0
          ? `se rellenaron ${imputedCellCount.toLocaleString("es-ES")} ${imputedCellCount === 1 ? "nulo" : "nulos"}`
          : null,
      ].filter((change): change is string => change !== null);
      // «Rellenar» of one column in «Corregir columna por columna» uses the same
      // engine with only imputation: it is not the proposal.
      const onlyFill = options.imputeMissing === true && !options.trimText && !options.normalizeSentinels
        && !options.normalizeColumnNames && !options.removeDuplicates && !castsColumns;
      const filledColumns = options.imputeColumns ?? [];
      setChangeStatus({
        kind: "applied",
        message: !changed
          ? onlyFill ? "No quedaba ningún nulo que rellenar." : "El dataset ya cumplía los cambios seleccionados."
          : onlyFill
            ? `${imputedCellCount === 1 ? "Se rellenó" : "Se rellenaron"} ${plural(imputedCellCount, "nulo", "nulos")}${filledColumns.length > 0 ? ` en ${filledColumns.join(", ")}` : ""}.`
            : "Propuesta aplicada: " + changes.join(" y ") + ".",
        changes: appliedPlanChanges(options, result),
        unchanged: !changed,
      });
      await refreshHistory();
      if (changed) onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyStructuralTransforms(recipe: TransformRecipe) {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "transform" });
    try {
      const result = await applyTransformRecipe(recipe, exceptionPolicy);
      const total = result.renamedColumnCount + result.convertedColumnCount +
        result.parsedDateColumnCount + result.removedRowCount + result.calculatedColumnCount +
        result.replacedCellCount + result.droppedColumnCount + result.splitColumnCount +
        result.mergedColumnCount + result.droppedSourceColumnCount + result.adjustedOutlierCellCount +
        result.outlierRemovedRowCount + result.collapsedRowCount + result.aggregatedColumnCount +
        result.normalizedContactCellCount + result.extractedColumnCount;
      await settleChange(result.dataset, total > 0, {
        message: total === 0
          ? "La receta no produjo cambios en el dataset."
          : `Receta aplicada: ${recipeResultParts(result)}.`,
      });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function changeHistory(direction: "undo" | "redo") {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: direction });
    try {
      const result = direction === "undo" ? await undoLastChange() : await redoLastChange();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      setHistoryStatus(result.history);
      onDeliveryInvalidated();
      setChangeStatus({ kind: "applied", message: result.message, origin: direction });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  return {
    changeStatus,
    historyStatus,
    cancelCurrent,
    resetChangeStatus,
    reportExternalChange,
    refreshHistory,
    applyDuplicateRemoval: runExclusive(applyDuplicateRemoval),
    applyNearDuplicateRemoval: runExclusive(applyNearDuplicateRemoval),
    applyEmptyRowRemoval: runExclusive(applyEmptyRowRemoval),
    applyConstantColumnRemoval: runExclusive(applyConstantColumnRemoval),
    applyEmptyColumnRemoval: runExclusive(applyEmptyColumnRemoval),
    applyHighNullColumnRemoval: runExclusive(applyHighNullColumnRemoval),
    applyIdentifierColumnRemoval: runExclusive(applyIdentifierColumnRemoval),
    applyPersonalColumnRemoval: runExclusive(applyPersonalColumnRemoval),
    applyPersonalValueMasking: runExclusive(applyPersonalValueMasking),
    applyBooleanNormalization: runExclusive(applyBooleanNormalization),
    applyDateParsing: runExclusive(applyDateParsing),
    applyNumericCast: runExclusive(applyNumericCast),
    applyEncodingFix: runExclusive(applyEncodingFix),
    applyInvalidTypeCleanup: runExclusive(applyInvalidTypeCleanup),
    applyMissingValueImputation: runExclusive(applyMissingValueImputation),
    applyCategoricalImputation: runExclusive(applyCategoricalImputation),
    applyOutlierImputation: runExclusive(applyOutlierImputation),
    applyOutlierCapping: runExclusive(() => applyOutlierTreatment("cap")),
    applyOutlierRemoval: runExclusive(() => applyOutlierTreatment("drop")),
    applyRowAudit: runExclusive(applyRowAudit),
    applyColumnNormalization: runExclusive(applyColumnNormalization),
    applyRecommendedCorrections: runExclusive(applyRecommendedCorrections),
    trimText: runExclusive(() => applyTextChange("trim")),
    normalizeText: runExclusive((columns: string[], removeAccents: boolean) =>
      applyTextChange("text", columns, removeAccents),
    ),
    applyStructuralTransforms: runExclusive(applyStructuralTransforms),
    undoChange: runExclusive(() => changeHistory("undo")),
    redoChange: runExclusive(() => changeHistory("redo")),
  };
}
