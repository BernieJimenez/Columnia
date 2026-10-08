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

  async function applyDuplicateRemoval() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "duplicates" });
    try {
      const result = await removeDuplicates();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      setChangeStatus({
        kind: "applied",
        message: `${result.affectedRowCount === 1 ? "Se eliminó" : "Se eliminaron"} ${plural(result.affectedRowCount, "fila duplicada adicional", "filas duplicadas adicionales")}.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyNearDuplicateRemoval() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "near_duplicates" });
    try {
      const result = await removeNearDuplicates();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      setChangeStatus({
        kind: "applied",
        message: result.affectedRowCount === 0
          ? "No se detectaron duplicados parecidos adicionales."
          : `${result.affectedRowCount === 1 ? "Se eliminó" : "Se eliminaron"} ${plural(result.affectedRowCount, "fila duplicada parecida", "filas duplicadas parecidas")}. La primera fila de cada grupo y las copias exactas se conservaron.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyEmptyRowRemoval() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "empty_rows" });
    try {
      const result = await removeEmptyRows();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      setChangeStatus({
        kind: "applied",
        message: result.affectedRowCount === 0
          ? "No se detectaron filas completamente vacías."
          : `${result.affectedRowCount === 1 ? "Se eliminó" : "Se eliminaron"} ${plural(result.affectedRowCount, "fila completamente vacía", "filas completamente vacías")}.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyConstantColumnRemoval() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "constant_columns" });
    try {
      const result = await removeConstantColumns();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      setChangeStatus({
        kind: "applied",
        message: result.removedColumnCount === 0
          ? "No se eliminaron columnas constantes; se conserva al menos una columna del dataset."
          : `${result.removedColumnCount === 1 ? "Se eliminó" : "Se eliminaron"} ${plural(result.removedColumnCount, "columna constante", "columnas constantes")}: ${result.removedColumns.join(", ")}.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyEmptyColumnRemoval() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "empty_columns" });
    try {
      const result = await removeEmptyColumns();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      setChangeStatus({
        kind: "applied",
        message: result.removedColumnCount === 0
          ? "No se eliminaron columnas vacías; se conserva al menos una columna del dataset."
          : `${result.removedColumnCount === 1 ? "Se eliminó" : "Se eliminaron"} ${plural(result.removedColumnCount, "columna completamente vacía", "columnas completamente vacías")}: ${result.removedColumns.join(", ")}.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyHighNullColumnRemoval() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "high_null_columns" });
    try {
      const result = await removeHighNullColumns();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      setChangeStatus({
        kind: "applied",
        message: result.removedColumnCount === 0
          ? `No se detectaron columnas con al menos ${result.thresholdPercentage ?? 80} % de valores nulos.`
          : `${result.removedColumnCount === 1 ? "Se eliminó" : "Se eliminaron"} ${plural(result.removedColumnCount, "columna con alta nulidad", "columnas con alta nulidad")}: ${result.removedColumns.join(", ")}.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyIdentifierColumnRemoval() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "identifier_columns" });
    try {
      const result = await removeIdentifierColumns();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      setChangeStatus({
        kind: "applied",
        message: result.removedColumnCount === 0
          ? "No se detectaron columnas identificadoras para retirar; se conserva al menos una columna del dataset."
          : `${result.removedColumnCount === 1 ? "Se retiró" : "Se retiraron"} ${plural(result.removedColumnCount, "columna identificadora", "columnas identificadoras")}: ${result.removedColumns.join(", ")}. La operación puede revertirse desde el historial.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyPersonalColumnRemoval() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "personal_columns" });
    try {
      const result = await removePersonalColumns();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      setChangeStatus({
        kind: "applied",
        message: result.removedColumnCount === 0
          ? "No se detectaron columnas de datos personales para retirar; se conserva al menos una columna del dataset."
          : `${result.removedColumnCount === 1 ? "Se retiró" : "Se retiraron"} ${plural(result.removedColumnCount, "columna de datos personales", "columnas de datos personales")}. No se muestran nombres ni valores. La operación puede revertirse desde el historial.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyPersonalValueMasking() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "personal_mask" });
    try {
      const result = await maskPersonalValues();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      const cells = result.changedCellCount === 1
        ? "1 valor"
        : `${result.changedCellCount.toLocaleString()} valores`;
      const columns = result.changedColumnCount === 1
        ? "1 columna"
        : `${result.changedColumnCount.toLocaleString()} columnas`;
      setChangeStatus({
        kind: "applied",
        message: result.changedCellCount === 0
          ? "No se encontraron valores personales no nulos que proteger."
          : `Se protegieron ${cells} en ${columns} con [REDACTED]. No se muestran nombres ni valores; el cambio puede revertirse desde el historial.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyBooleanNormalization() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "booleans" });
    try {
      const result = await normalizeBooleanValues();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      const columns = result.changedColumns.map((column) => column.name).join(", ");
      setChangeStatus({
        kind: "applied",
        message: result.changedCellCount === 0
          ? "No se encontraron alias booleanos que necesitaran normalización."
          : `${result.changedCellCount === 1 ? "Se normalizó" : "Se normalizaron"} ${plural(result.changedCellCount, "valor booleano", "valores booleanos")} en: ${columns}.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyDateParsing() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "parse_dates" });
    try {
      const result = await parseDateValues();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      const columns = result.changedColumns.map((column) => column.name).join(", ");
      setChangeStatus({
        kind: "applied",
        message: result.changedCellCount === 0
          ? "No se detectaron columnas de texto con un formato de fecha dominante y seguro."
          : `${result.changedCellCount === 1 ? "Se interpretó" : "Se interpretaron"} ${plural(result.changedCellCount, "valor de fecha", "valores de fecha")} en: ${columns}.${nullifiedCellsSentence(result.changedColumns)} Las columnas ambiguas se dejaron intactas y el cambio puede revertirse desde el historial.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyEncodingFix() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "encoding" });
    try {
      const result = await fixEncodingValues();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      const detail = result.changedCellCount === 1
        ? "1 celda"
        : `${result.changedCellCount.toLocaleString()} celdas`;
      setChangeStatus({
        kind: "applied",
        message: result.changedCellCount === 0
          ? "No se detectaron valores con doble codificación UTF-8."
          : `Se corrigió doble codificación UTF-8 en ${detail}.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyNumericCast() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "cast_numeric" });
    try {
      const result = await castNumericValues();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      const columns = result.changedColumns.map((column) => column.name).join(", ");
      setChangeStatus({
        kind: "applied",
        message: result.changedCellCount === 0
          ? "No se detectaron columnas de texto numéricas seguras para convertir."
          : `${result.changedCellCount === 1 ? "Se convirtió" : "Se convirtieron"} ${plural(result.changedCellCount, "valor numérico", "valores numéricos")} en: ${columns}.${nullifiedCellsSentence(result.changedColumns)} Los identificadores y códigos con ceros iniciales se conservaron; el cambio puede revertirse desde el historial.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyInvalidTypeCleanup() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "invalid_types" });
    try {
      const result = await nullifyInvalidTypeValues();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      const detail = result.changedCellCount === 1
        ? "1 valor"
        : `${result.changedCellCount.toLocaleString()} valores`;
      setChangeStatus({
        kind: "applied",
        message: result.changedCellCount === 0
          ? "No se encontraron valores incompatibles con un tipo sugerido con confianza."
          : result.changedCellCount === 1
            ? `Se apartó ${detail} incompatible como nulo; el cambio puede revertirse desde el historial.`
            : `Se apartaron ${detail} incompatibles como nulos; el cambio puede revertirse desde el historial.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyMissingValueImputation() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "impute" });
    try {
      const result = await imputeMissingValues();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      const columns = result.changedColumns.map((column) => column.name).join(", ");
      setChangeStatus({
        kind: "applied",
        message: result.changedCellCount === 0
          ? "No se encontraron nulos imputables con una señal conservadora."
          : `${result.changedCellCount === 1 ? "Se imputó" : "Se imputaron"} ${plural(result.changedCellCount, "valor nulo", "valores nulos")} en: ${columns}.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyOutlierImputation() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "outlier_impute" });
    try {
      const result = await imputeOutlierValues();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      const columns = result.changedColumns.map((column) => column.name).join(", ");
      setChangeStatus({
        kind: "applied",
        message: result.changedCellCount === 0
          ? "No se detectaron valores atípicos que necesitaran reemplazo."
          : `${result.changedCellCount === 1 ? "Se reemplazó" : "Se reemplazaron"} ${plural(result.changedCellCount, "valor atípico", "valores atípicos")} por la mediana en: ${columns}. El cambio puede revertirse desde el historial.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
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
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      const columns = result.changedColumns.map((column) => column.name).join(", ");
      setChangeStatus({
        kind: "applied",
        message: action === "cap"
          ? result.changedCellCount === 0
            ? "No se detectaron valores atípicos que necesitaran limitación."
            : `${result.changedCellCount === 1 ? "Se limitó" : "Se limitaron"} ${plural(result.changedCellCount, "valor atípico", "valores atípicos")} a los límites IQR en: ${columns}. El cambio puede revertirse desde el historial.`
          : result.affectedRowCount === 0
            ? "No se detectaron filas atípicas para eliminar."
            : `${result.affectedRowCount === 1 ? "Se eliminó" : "Se eliminaron"} ${plural(result.affectedRowCount, "fila atípica", "filas atípicas")} según los límites IQR. El cambio puede revertirse desde el historial.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyCategoricalImputation() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "categorical_impute" });
    try {
      const result = await imputeCategoricalValues();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      const columns = result.changedColumns.map((column) => column.name).join(", ");
      setChangeStatus({
        kind: "applied",
        message: result.changedCellCount === 0
          ? "No se encontraron nulos textuales para completar como Desconocido."
          : `${result.changedCellCount === 1 ? "Se completó" : "Se completaron"} ${plural(result.changedCellCount, "nulo textual", "nulos textuales")} como Desconocido en: ${columns}. El cambio puede revertirse desde el historial.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyRowAudit() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "audit" });
    try {
      const result = await enableRowAudit();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      const enabled = result.dataset.columns.some((column) => isRowAuditColumn(column.name));
      setChangeStatus({
        kind: "applied",
        message: enabled
          ? "La trazabilidad por fila está activa; los cambios futuros se anotarán en _cambios."
          : "La trazabilidad por fila no produjo cambios.",
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  async function applyColumnNormalization() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "columns" });
    try {
      const result = await normalizeColumnNames();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      setChangeStatus({
        kind: "applied",
        message:
          result.renamedColumnCount === 0
            ? "Los nombres de las columnas ya estaban normalizados."
            : result.renamedColumnCount === 1
              ? "Se normalizó 1 nombre de columna."
              : `${result.renamedColumnCount === 1 ? "Se normalizó" : "Se normalizaron"} ${plural(result.renamedColumnCount, "nombre de columna", "nombres de columnas")}.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
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
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      const cells = result.changedCellCount === 1
        ? "1 celda"
        : `${result.changedCellCount.toLocaleString()} celdas`;
      const rows = result.affectedRowCount === 1
        ? "1 fila"
        : `${result.affectedRowCount.toLocaleString()} filas`;
      const detail = `${cells} en ${rows}`;
      setChangeStatus({
        kind: "applied",
        message:
          result.changedCellCount === 0
            ? "No se encontraron valores que necesitaran esta corrección."
            : action === "trim"
              ? `Se recortaron espacios en ${detail}.`
              : `Se normalizó texto en ${detail}.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
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
      const result = await applySafeCorrections(options);
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
        : `${result.changedCellCount.toLocaleString()} celdas actualizadas`;
      const renamedColumns = result.renamedColumnCount === 1
        ? "1 columna renombrada"
        : `${result.renamedColumnCount.toLocaleString()} columnas renombradas`;
      const changes = [
        (options.trimText || options.normalizeSentinels) && result.changedCellCount > 0 ? changedCells : null,
        options.normalizeColumnNames && result.renamedColumnCount > 0 ? renamedColumns : null,
        options.removeDuplicates && result.removedRowCount > 0
          ? `${result.removedRowCount === 1 ? "se retiró" : "se retiraron"} ${plural(result.removedRowCount, "fila duplicada exacta", "filas duplicadas exactas")}`
          : null,
        result.typedColumnCount > 0
          ? `${result.typedColumnCount.toLocaleString()} ${result.typedColumnCount === 1 ? "columna convertida" : "columnas convertidas"} a número`
          : null,
        result.datedColumnCount > 0
          ? `${result.datedColumnCount.toLocaleString()} ${result.datedColumnCount === 1 ? "columna convertida" : "columnas convertidas"} a fecha`
          : null,
        options.imputeMissing && imputedCellCount > 0
          ? `se rellenaron ${imputedCellCount.toLocaleString()} ${imputedCellCount === 1 ? "valor vacío" : "valores vacíos"}`
          : null,
      ].filter((change): change is string => change !== null);
      setChangeStatus({
        kind: "applied",
        message: changed
          ? "Plan aplicado: " + changes.join(" y ") + "."
          : "El dataset ya cumplía las correcciones seleccionadas.",
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
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      const total = result.renamedColumnCount + result.convertedColumnCount +
        result.parsedDateColumnCount + result.removedRowCount + result.calculatedColumnCount +
        result.replacedCellCount + result.droppedColumnCount + result.splitColumnCount +
        result.mergedColumnCount + result.droppedSourceColumnCount + result.adjustedOutlierCellCount +
        result.outlierRemovedRowCount + result.collapsedRowCount + result.aggregatedColumnCount +
        result.normalizedContactCellCount + result.extractedColumnCount;
      setChangeStatus({
        kind: "applied",
        message: total === 0
          ? "La receta no produjo cambios en el dataset."
          : `Receta aplicada: ${plural(result.renamedColumnCount, "renombre", "renombres")}, ${plural(result.convertedColumnCount, "conversión", "conversiones")}, ${plural(result.parsedDateColumnCount, "fecha interpretada", "fechas interpretadas")}, ${plural(result.removedRowCount, "fila filtrada", "filas filtradas")}, ${plural(result.outlierRemovedRowCount, "fila atípica eliminada", "filas atípicas eliminadas")}, ${plural(result.calculatedColumnCount, "columna calculada", "columnas calculadas")}, ${plural(result.replacedCellCount, "celda reemplazada", "celdas reemplazadas")}, ${plural(result.splitColumnCount, "columna dividida", "columnas divididas")}, ${plural(result.mergedColumnCount, "columna combinada", "columnas combinadas")}, ${plural(result.droppedColumnCount + result.droppedSourceColumnCount, "columna descartada", "columnas descartadas")}, ${plural(result.adjustedOutlierCellCount, "valor atípico ajustado", "valores atípicos ajustados")}, ${plural(result.normalizedContactCellCount, "contacto normalizado", "contactos normalizados")} en ${plural(result.normalizedContactColumnCount, "columna", "columnas")}, ${plural(result.extractedColumnCount, "columna extraída", "columnas extraídas")} y resumen de ${plural(result.groupCount, "grupo", "grupos")} con ${plural(result.aggregatedColumnCount, "agregación", "agregaciones")}.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
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
      setChangeStatus({ kind: "applied", message: result.message });
    } catch (error: unknown) {
      setChangeStatus(changeFailureStatus(error));
    }
  }

  return {
    changeStatus,
    historyStatus,
    cancelCurrent,
    resetChangeStatus,
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
