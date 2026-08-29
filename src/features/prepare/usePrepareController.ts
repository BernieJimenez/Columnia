import { useState } from "react";

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
  normalizeSentinelValues,
  normalizeBooleanValues,
  normalizeTextValues,
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
  type TransformRecipe,
} from "../../bridge";
import { EMPTY_HISTORY, type ChangeStatus } from "./prepareModel";

interface PrepareControllerOptions {
  activeDataset: DatasetPreview | null;
  onDatasetChanged: (dataset: DatasetPreview) => void;
  onProfileInvalidated: () => void;
  onDeliveryInvalidated: () => void;
}

export function usePrepareController({
  activeDataset,
  onDatasetChanged,
  onProfileInvalidated,
  onDeliveryInvalidated,
}: PrepareControllerOptions) {
  const [changeStatus, setChangeStatus] = useState<ChangeStatus>({ kind: "idle" });
  const [historyStatus, setHistoryStatus] = useState(EMPTY_HISTORY);

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
        message: `Se eliminaron ${result.affectedRowCount.toLocaleString()} filas duplicadas adicionales.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
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
          : `Se eliminaron ${result.affectedRowCount.toLocaleString()} filas duplicadas parecidas. La primera fila de cada grupo y las copias exactas se conservaron.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
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
          : `Se eliminaron ${result.affectedRowCount.toLocaleString()} filas completamente vacías.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
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
          : `Se eliminaron ${result.removedColumnCount.toLocaleString()} columnas constantes: ${result.removedColumns.join(", ")}.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
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
          : `Se eliminaron ${result.removedColumnCount.toLocaleString()} columnas completamente vacías: ${result.removedColumns.join(", ")}.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
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
          ? "No se detectaron columnas con al menos 80% de valores nulos."
          : `Se eliminaron ${result.removedColumnCount.toLocaleString()} columnas con alta nulidad: ${result.removedColumns.join(", ")}.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
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
          : `Se retiraron ${result.removedColumnCount.toLocaleString()} columnas identificadoras: ${result.removedColumns.join(", ")}. La operación puede revertirse desde el historial.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
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
          : `Se retiraron ${result.removedColumnCount.toLocaleString()} columnas de datos personales. No se muestran nombres ni valores. La operación puede revertirse desde el historial.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
    }
  }

  async function applySentinelNormalization() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "sentinels" });
    try {
      const result = await normalizeSentinelValues();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      const columns = result.changedColumns.map((column) => column.name).join(", ");
      setChangeStatus({
        kind: "applied",
        message: result.changedCellCount === 0
          ? "No se detectaron valores centinela conocidos en las columnas de texto."
          : `Se convirtieron ${result.changedCellCount.toLocaleString()} valores centinela a nulos en: ${columns}.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
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
          : `Se normalizaron ${result.changedCellCount.toLocaleString()} valores booleanos en: ${columns}.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
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
      setChangeStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
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
      setChangeStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
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
          : `Se imputaron ${result.changedCellCount.toLocaleString()} valores nulos en: ${columns}.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
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
          ? "No se detectaron outliers que necesitaran imputación."
          : `Se reemplazaron ${result.changedCellCount.toLocaleString()} outliers por la mediana en: ${columns}. El cambio puede revertirse desde el historial.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
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
            ? "No se detectaron outliers que necesitaran limitación."
            : `Se limitaron ${result.changedCellCount.toLocaleString()} outliers a los límites IQR en: ${columns}. El cambio puede revertirse desde el historial.`
          : result.affectedRowCount === 0
            ? "No se detectaron filas atípicas para eliminar."
            : `Se eliminaron ${result.affectedRowCount.toLocaleString()} filas atípicas según los límites IQR. El cambio puede revertirse desde el historial.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
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
          : `Se completaron ${result.changedCellCount.toLocaleString()} nulos textuales como Desconocido en: ${columns}. El cambio puede revertirse desde el historial.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
    }
  }

  async function applyRowAudit() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "audit" });
    try {
      const result = await enableRowAudit();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      const enabled = result.dataset.columns.some((column) => column.name === "_cambios");
      setChangeStatus({
        kind: "applied",
        message: enabled
          ? "La trazabilidad por fila está activa; los cambios futuros se anotarán en _cambios."
          : "La trazabilidad por fila no produjo cambios.",
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
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
              : `Se normalizaron ${result.renamedColumnCount.toLocaleString()} nombres de columnas.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
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
      setChangeStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
    }
  }

  async function applyRecommendedCorrections() {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "safe" });
    try {
      const result = await applySafeCorrections();
      onDatasetChanged(result.dataset);
      onProfileInvalidated();
      const changed = result.changedCellCount > 0 || result.renamedColumnCount > 0;
      const changedCells = result.changedCellCount === 1
        ? "1 celda recortada"
        : `${result.changedCellCount.toLocaleString()} celdas recortadas`;
      const renamedColumns = result.renamedColumnCount === 1
        ? "1 columna renombrada"
        : `${result.renamedColumnCount.toLocaleString()} columnas renombradas`;
      setChangeStatus({
        kind: "applied",
        message: changed
          ? `Correcciones recomendadas aplicadas: ${changedCells} y ${renamedColumns}.`
          : "El dataset ya cumplía las correcciones recomendadas.",
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
    }
  }

  async function applyStructuralTransforms(recipe: TransformRecipe) {
    if (activeDataset === null) return;
    setChangeStatus({ kind: "working", action: "transform" });
    try {
      const result = await applyTransformRecipe(recipe);
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
          : `Receta aplicada: ${result.renamedColumnCount.toLocaleString()} renombres, ${result.convertedColumnCount.toLocaleString()} conversiones, ${result.parsedDateColumnCount.toLocaleString()} fechas interpretadas, ${result.removedRowCount.toLocaleString()} filas filtradas, ${result.outlierRemovedRowCount.toLocaleString()} filas atípicas eliminadas, ${result.calculatedColumnCount.toLocaleString()} columnas calculadas, ${result.replacedCellCount.toLocaleString()} celdas reemplazadas, ${result.splitColumnCount.toLocaleString()} columnas divididas, ${result.mergedColumnCount.toLocaleString()} columnas combinadas, ${(result.droppedColumnCount + result.droppedSourceColumnCount).toLocaleString()} columnas descartadas, ${result.adjustedOutlierCellCount.toLocaleString()} outliers ajustados, ${result.normalizedContactCellCount.toLocaleString()} contactos normalizados en ${result.normalizedContactColumnCount.toLocaleString()} columnas, ${result.extractedColumnCount.toLocaleString()} columnas extraídas y resumen de ${result.groupCount.toLocaleString()} grupos con ${result.aggregatedColumnCount.toLocaleString()} agregaciones.`,
      });
      await refreshHistory();
      onDeliveryInvalidated();
    } catch (error: unknown) {
      setChangeStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
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
      setChangeStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) });
    }
  }

  return {
    changeStatus,
    historyStatus,
    resetChangeStatus,
    refreshHistory,
    applyDuplicateRemoval,
    applyNearDuplicateRemoval,
    applyEmptyRowRemoval,
    applyConstantColumnRemoval,
    applyEmptyColumnRemoval,
    applyHighNullColumnRemoval,
    applyIdentifierColumnRemoval,
    applyPersonalColumnRemoval,
    applySentinelNormalization,
    applyBooleanNormalization,
    applyEncodingFix,
    applyInvalidTypeCleanup,
    applyMissingValueImputation,
    applyCategoricalImputation,
    applyOutlierImputation,
    applyOutlierCapping: () => applyOutlierTreatment("cap"),
    applyOutlierRemoval: () => applyOutlierTreatment("drop"),
    applyRowAudit,
    applyColumnNormalization,
    applyRecommendedCorrections,
    trimText: () => applyTextChange("trim"),
    normalizeText: (columns: string[], removeAccents: boolean) =>
      applyTextChange("text", columns, removeAccents),
    applyStructuralTransforms,
    undoChange: () => changeHistory("undo"),
    redoChange: () => changeHistory("redo"),
  };
}
