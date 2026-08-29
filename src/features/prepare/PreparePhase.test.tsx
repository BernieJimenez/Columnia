import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as bridge from "../../bridge";
import type { DatasetPreview, DatasetProfile, HistoryState, LoadedRecipe, TransformRecipe } from "../../bridge";
import { PreparePhase } from "./PreparePhase";
import { TransformRecipeEditor } from "./TransformRecipeEditor";
import { EMPTY_HISTORY } from "./prepareModel";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const dataset: DatasetPreview = {
  fileName: "clientes.csv",
  fileSizeBytes: 128,
  rowCount: 2,
  columnCount: 2,
  columns: [
    { name: "nombre", dataType: "String" },
    { name: "total", dataType: "Int64" },
  ],
  rows: [[" Ana ", "10"], ["Luis", "20"]],
};

const emptyRecipe: TransformRecipe = {
  renames: [], casts: [], dateParses: [], filters: [], calculatedColumn: null,
  findReplace: null, keepColumns: null, splitColumn: null, mergeColumns: null,
  outlierTreatments: [], groupSummary: null, contactNormalizations: [], textExtractions: [],
};

const cleaningSignalsProfile: DatasetProfile = {
  rowCount: 5,
  duplicateRowCount: 1,
  nearDuplicateRowCount: 1,
  duplicatePercentage: 20,
  columns: [{
    name: "email",
    dataType: "String",
    nullCount: 1,
    completenessPercentage: 50,
    uniqueCount: 1,
    minimum: "ana@example.com",
    maximum: "ana@example.com",
    mean: null,
    emptyCount: 0,
    minimumLength: 15,
    maximumLength: 15,
    averageLength: 15,
    suggestedType: "boolean",
    typeMatchPercentage: 100,
    invalidTypeCount: 1,
    sentinelCount: 1,
    encodingIssueCount: 2,
    privacySignal: "email",
    standardDeviation: null,
    firstQuartile: null,
    median: null,
    thirdQuartile: null,
    outlierCount: null,
    histogram: null,
  }, {
    name: "fecha_alta",
    dataType: "String",
    nullCount: 0,
    completenessPercentage: 100,
    uniqueCount: 5,
    minimum: "2025-01-01",
    maximum: "2025-01-05",
    mean: null,
    emptyCount: 0,
    minimumLength: 10,
    maximumLength: 10,
    averageLength: 10,
    suggestedType: "date",
    typeMatchPercentage: 100,
    invalidTypeCount: 0,
    sentinelCount: 0,
    encodingIssueCount: 0,
    privacySignal: null,
    standardDeviation: null,
    firstQuartile: null,
    median: null,
    thirdQuartile: null,
    outlierCount: null,
    histogram: null,
  }, {
    name: "empty_column",
    dataType: "String",
    nullCount: 5,
    completenessPercentage: 0,
    uniqueCount: 0,
    minimum: null,
    maximum: null,
    mean: null,
    emptyCount: 0,
    minimumLength: null,
    maximumLength: null,
    averageLength: null,
    suggestedType: null,
    typeMatchPercentage: null,
    invalidTypeCount: null,
    sentinelCount: null,
    encodingIssueCount: null,
    privacySignal: null,
    standardDeviation: null,
    firstQuartile: null,
    median: null,
    thirdQuartile: null,
    outlierCount: null,
    histogram: null,
  }, {
    name: "high_null",
    dataType: "String",
    nullCount: 4,
    completenessPercentage: 20,
    uniqueCount: 1,
    minimum: "ok",
    maximum: "ok",
    mean: null,
    emptyCount: 0,
    minimumLength: 2,
    maximumLength: 2,
    averageLength: 2,
    suggestedType: null,
    typeMatchPercentage: null,
    invalidTypeCount: null,
    sentinelCount: null,
    encodingIssueCount: null,
    privacySignal: null,
    standardDeviation: null,
    firstQuartile: null,
    median: null,
    thirdQuartile: null,
    outlierCount: null,
    histogram: null,
  }, {
    name: "customer_id",
    dataType: "String",
    nullCount: 0,
    completenessPercentage: 100,
    uniqueCount: 5,
    minimum: null,
    maximum: null,
    mean: null,
    emptyCount: 0,
    minimumLength: 1,
    maximumLength: 2,
    averageLength: 1.5,
    suggestedType: null,
    typeMatchPercentage: null,
    invalidTypeCount: null,
    sentinelCount: 0,
    encodingIssueCount: 0,
    privacySignal: "identifier",
    standardDeviation: null,
    firstQuartile: null,
    median: null,
    thirdQuartile: null,
    outlierCount: null,
    histogram: null,
  }, {
    name: "amount",
    dataType: "Int64",
    nullCount: 0,
    completenessPercentage: 100,
    uniqueCount: 5,
    minimum: "1",
    maximum: "100",
    mean: 22,
    emptyCount: 0,
    minimumLength: null,
    maximumLength: null,
    averageLength: null,
    suggestedType: null,
    typeMatchPercentage: null,
    invalidTypeCount: null,
    sentinelCount: null,
    encodingIssueCount: null,
    privacySignal: null,
    standardDeviation: 40,
    firstQuartile: 2,
    median: 3,
    thirdQuartile: 4,
    outlierCount: 1,
    histogram: null,
  }],
};

describe("PreparePhase", () => {
  it("conserva tabs ARIA y selección explícita para normalizar texto", () => {
    const onNormalizeText = vi.fn();
    render(<PreparePhase
      dataset={dataset}
      profileStatus={{ kind: "idle" }}
      changeStatus={{ kind: "idle" }}
      historyStatus={EMPTY_HISTORY}
      recipeDraft={null}
      recipeSession={0}
      onAnalyzeQuality={() => undefined}
      onCancelProfile={() => undefined}
      onRemoveDuplicates={() => undefined}
      onRemoveEmptyRows={() => undefined}
      onRemoveConstantColumns={() => undefined}
    onRemoveEmptyColumns={() => undefined}
      onRemoveHighNullColumns={() => undefined}
      onNormalizeSentinels={() => undefined}
      onNormalizeBooleans={() => undefined}
      onImputeMissingValues={() => undefined}
      onEnableRowAudit={() => undefined}
      onNormalizeColumns={() => undefined}
      onApplyRecommended={() => undefined}
      onTrimText={() => undefined}
      onNormalizeText={onNormalizeText}
      onApplyTransforms={() => undefined}
      onRecipeDraftChange={() => undefined}
      onUndo={() => undefined}
      onRedo={() => undefined}
    />);

    const normalize = screen.getByRole("button", { name: "Normalizar texto seleccionado" });
    expect(normalize).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox", { name: "nombre" }));
    expect(normalize).toBeEnabled();
    fireEvent.click(normalize);
    expect(onNormalizeText).toHaveBeenCalledWith(["nombre"], true);

    const correctionsTab = screen.getByRole("tab", { name: "Correcciones" });
    fireEvent.keyDown(correctionsTab, { key: "ArrowRight" });
    expect(screen.getByRole("tab", { name: "Transformaciones" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("heading", { name: "Preparar estructura y tipos" })).toBeInTheDocument();
  });

  it("respeta disponibilidad y callbacks de Deshacer/Rehacer", () => {
    const onUndo = vi.fn();
    const onRedo = vi.fn();
    const history: HistoryState = {
      ...EMPTY_HISTORY,
      canUndo: true,
      entryCount: 1,
      entries: [{ index: 0, label: "Normalizar texto", isCurrent: true }],
    };
    render(<PreparePhase
      dataset={dataset} profileStatus={{ kind: "idle" }} changeStatus={{ kind: "idle" }}
      historyStatus={history} onAnalyzeQuality={() => undefined} onCancelProfile={() => undefined}
      recipeDraft={null} recipeSession={0}
      onRemoveDuplicates={() => undefined} onRemoveConstantColumns={() => undefined} onRemoveEmptyColumns={() => undefined} onRemoveHighNullColumns={() => undefined} onNormalizeSentinels={() => undefined} onNormalizeBooleans={() => undefined} onImputeMissingValues={() => undefined} onEnableRowAudit={() => undefined} onNormalizeColumns={() => undefined}
      onRemoveEmptyRows={() => undefined}
      onApplyRecommended={() => undefined} onTrimText={() => undefined}
      onNormalizeText={() => undefined} onApplyTransforms={() => undefined}
      onRecipeDraftChange={() => undefined}
      onUndo={onUndo} onRedo={onRedo}
    />);

    fireEvent.click(screen.getByRole("button", { name: "Deshacer" }));
    expect(onUndo).toHaveBeenCalledOnce();
    expect(screen.getByRole("button", { name: "Rehacer" })).toBeDisabled();
    expect(screen.getByText(/Versión actual: Normalizar texto/)).toBeInTheDocument();
  });

  it("expone señales agregadas de limpieza y privacidad sin mostrar celdas", () => {
    const onRemoveConstantColumns = vi.fn();
    const onRemoveEmptyColumns = vi.fn();
    const onRemoveHighNullColumns = vi.fn();
    const onNormalizeSentinels = vi.fn();
    const onNormalizeBooleans = vi.fn();
    const onParseDates = vi.fn();
    const onFixEncoding = vi.fn();
    const onImputeMissingValues = vi.fn();
    const onImputeCategoricalValues = vi.fn();
    const onImputeOutliers = vi.fn();
    const onCapOutliers = vi.fn();
    const onDropOutliers = vi.fn();
    const onEnableRowAudit = vi.fn();
    render(<PreparePhase
      dataset={dataset}
      profileStatus={{ kind: "ready", profile: cleaningSignalsProfile }}
      changeStatus={{ kind: "idle" }}
      historyStatus={EMPTY_HISTORY}
      recipeDraft={null}
      recipeSession={0}
      onAnalyzeQuality={() => undefined}
      onCancelProfile={() => undefined}
      onRemoveDuplicates={() => undefined}
      onRemoveEmptyRows={() => undefined}
      onRemoveConstantColumns={onRemoveConstantColumns}
      onRemoveEmptyColumns={onRemoveEmptyColumns}
      onRemoveHighNullColumns={onRemoveHighNullColumns}
      onNormalizeSentinels={onNormalizeSentinels}
      onNormalizeBooleans={onNormalizeBooleans}
      onParseDates={onParseDates}
      onFixEncoding={onFixEncoding}
      onImputeMissingValues={onImputeMissingValues}
      onImputeCategoricalValues={onImputeCategoricalValues}
      onImputeOutliers={onImputeOutliers}
      onCapOutliers={onCapOutliers}
      onDropOutliers={onDropOutliers}
      onEnableRowAudit={onEnableRowAudit}
      onNormalizeColumns={() => undefined}
      onApplyRecommended={() => undefined}
      onTrimText={() => undefined}
      onNormalizeText={() => undefined}
      onApplyTransforms={() => undefined}
      onRecipeDraftChange={() => undefined}
      onUndo={() => undefined}
      onRedo={() => undefined}
    />);

    expect(screen.getByRole("heading", { name: "Señales para revisar" })).toBeInTheDocument();
    expect(screen.getByRole("list")).toHaveTextContent("1 filas adicionales");
    expect(screen.getByRole("list")).toHaveTextContent("Posible dato personal: 1 columna detectada por categoría agregada: correo electrónico (1)");
    expect(screen.getByRole("button", { name: "Revisar identificadores detectados" })).toBeInTheDocument();
    expect(screen.getByRole("list")).toHaveTextContent("Duplicados parecidos: 1");
    expect(screen.getByRole("list")).toHaveTextContent("Tipos sugeridos:");
    expect(screen.getByRole("list")).toHaveTextContent("Fechas detectadas: fecha_alta coincide con un formato de fecha cerrado.");
    fireEvent.click(screen.getByRole("button", { name: "Eliminar columnas constantes" }));
    expect(onRemoveConstantColumns).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Eliminar columnas vacías" }));
    expect(onRemoveEmptyColumns).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Eliminar columnas con alta nulidad" }));
    expect(onRemoveHighNullColumns).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Convertir centinelas a nulos" }));
    expect(onNormalizeSentinels).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Corregir codificación" }));
    expect(onFixEncoding).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Normalizar booleanos" }));
    expect(onNormalizeBooleans).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Interpretar fechas detectadas" }));
    expect(onParseDates).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Intentar imputación conservadora" }));
    expect(onImputeMissingValues).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Completar categorías desconocidas" }));
    expect(onImputeCategoricalValues).toHaveBeenCalledOnce();
    expect(screen.getByRole("list")).toHaveTextContent("Valores atípicos: amount (1) supera los límites IQR de 1.5.");
    fireEvent.click(screen.getByRole("button", { name: "Imputar outliers con mediana" }));
    expect(onImputeOutliers).toHaveBeenCalledOnce();
    expect(screen.getByRole("button", { name: "Limitar outliers con IQR" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Eliminar filas atípicas" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Limitar outliers con IQR" }));
    expect(screen.getByRole("alertdialog")).toHaveTextContent("Limitar valores atípicos");
    fireEvent.click(screen.getByRole("button", { name: "Limitar outliers" }));
    expect(onCapOutliers).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Eliminar filas atípicas" }));
    expect(screen.getByRole("alertdialog")).toHaveTextContent("Se eliminará cualquier fila");
    fireEvent.click(within(screen.getByRole("alertdialog")).getByRole("button", { name: "Eliminar filas atípicas" }));
    expect(onDropOutliers).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Activar trazabilidad" }));
    expect(onEnableRowAudit).toHaveBeenCalledOnce();
  });

  it("confirma el impacto de duplicados parecidos sin exponer valores y permite cancelar", () => {
    const onRemoveNearDuplicates = vi.fn();
    render(<PreparePhase
      dataset={dataset}
      profileStatus={{ kind: "ready", profile: cleaningSignalsProfile }}
      changeStatus={{ kind: "idle" }}
      historyStatus={EMPTY_HISTORY}
      recipeDraft={null}
      recipeSession={0}
      onAnalyzeQuality={() => undefined}
      onCancelProfile={() => undefined}
      onRemoveDuplicates={() => undefined}
      onRemoveNearDuplicates={onRemoveNearDuplicates}
      onRemoveEmptyRows={() => undefined}
      onRemoveConstantColumns={() => undefined}
      onRemoveEmptyColumns={() => undefined}
      onRemoveHighNullColumns={() => undefined}
      onNormalizeSentinels={() => undefined}
      onNormalizeBooleans={() => undefined}
      onImputeMissingValues={() => undefined}
      onEnableRowAudit={() => undefined}
      onNormalizeColumns={() => undefined}
      onApplyRecommended={() => undefined}
      onTrimText={() => undefined}
      onNormalizeText={() => undefined}
      onApplyTransforms={() => undefined}
      onRecipeDraftChange={() => undefined}
      onUndo={() => undefined}
      onRedo={() => undefined}
    />);

    fireEvent.click(screen.getByRole("button", { name: "Revisar y eliminar parecidos" }));
    const dialog = screen.getByRole("alertdialog", { name: "Eliminar duplicados parecidos" });
    expect(dialog).toHaveTextContent("1 filas");
    expect(dialog).not.toHaveTextContent("Ana");
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(onRemoveNearDuplicates).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Revisar y eliminar parecidos" }));
    fireEvent.click(screen.getByRole("button", { name: "Eliminar duplicados parecidos" }));
    expect(onRemoveNearDuplicates).toHaveBeenCalledOnce();
  });

  it("confirma el retiro de identificadores sin exponer celdas y permite cancelarlo", () => {
    const onRemoveIdentifierColumns = vi.fn();
    render(<PreparePhase
      dataset={dataset}
      profileStatus={{ kind: "ready", profile: cleaningSignalsProfile }}
      changeStatus={{ kind: "idle" }}
      historyStatus={EMPTY_HISTORY}
      recipeDraft={null}
      recipeSession={0}
      onAnalyzeQuality={() => undefined}
      onCancelProfile={() => undefined}
      onRemoveDuplicates={() => undefined}
      onRemoveNearDuplicates={() => undefined}
      onRemoveEmptyRows={() => undefined}
      onRemoveConstantColumns={() => undefined}
      onRemoveEmptyColumns={() => undefined}
      onRemoveHighNullColumns={() => undefined}
      onRemoveIdentifierColumns={onRemoveIdentifierColumns}
      onNormalizeSentinels={() => undefined}
      onNormalizeBooleans={() => undefined}
      onImputeMissingValues={() => undefined}
      onEnableRowAudit={() => undefined}
      onNormalizeColumns={() => undefined}
      onApplyRecommended={() => undefined}
      onTrimText={() => undefined}
      onNormalizeText={() => undefined}
      onApplyTransforms={() => undefined}
      onRecipeDraftChange={() => undefined}
      onUndo={() => undefined}
      onRedo={() => undefined}
    />);

    fireEvent.click(screen.getByRole("button", { name: "Revisar identificadores detectados" }));
    const dialog = screen.getByRole("alertdialog", { name: "Retirar identificadores detectados" });
    expect(dialog).toHaveTextContent("1 columna identificadora");
    expect(dialog).toHaveTextContent("customer_id");
    expect(dialog).not.toHaveTextContent("Ana");
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(onRemoveIdentifierColumns).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Revisar identificadores detectados" }));
    fireEvent.click(screen.getByRole("button", { name: "Retirar identificadores" }));
    expect(onRemoveIdentifierColumns).toHaveBeenCalledOnce();
  });

  it("confirma el retiro de PII por categorías agregadas sin exponer nombres ni valores", () => {
    const onRemovePersonalColumns = vi.fn();
    const onMaskPersonalValues = vi.fn();
    render(<PreparePhase
      dataset={dataset}
      profileStatus={{ kind: "ready", profile: cleaningSignalsProfile }}
      changeStatus={{ kind: "idle" }}
      historyStatus={EMPTY_HISTORY}
      recipeDraft={null}
      recipeSession={0}
      onAnalyzeQuality={() => undefined}
      onCancelProfile={() => undefined}
      onRemoveDuplicates={() => undefined}
      onRemoveNearDuplicates={() => undefined}
      onRemoveEmptyRows={() => undefined}
      onRemoveConstantColumns={() => undefined}
      onRemoveEmptyColumns={() => undefined}
      onRemoveHighNullColumns={() => undefined}
      onRemoveIdentifierColumns={() => undefined}
      onRemovePersonalColumns={onRemovePersonalColumns}
      onMaskPersonalValues={onMaskPersonalValues}
      onNormalizeSentinels={() => undefined}
      onNormalizeBooleans={() => undefined}
      onImputeMissingValues={() => undefined}
      onEnableRowAudit={() => undefined}
      onNormalizeColumns={() => undefined}
      onApplyRecommended={() => undefined}
      onTrimText={() => undefined}
      onNormalizeText={() => undefined}
      onApplyTransforms={() => undefined}
      onRecipeDraftChange={() => undefined}
      onUndo={() => undefined}
      onRedo={() => undefined}
    />);

    fireEvent.click(screen.getByRole("button", { name: "Revisar datos personales detectados" }));
    const dialog = screen.getByRole("alertdialog", { name: "Retirar datos personales detectados" });
    expect(dialog).toHaveTextContent("1 columna personal");
    expect(dialog).toHaveTextContent("correo electrónico (1)");
    expect(dialog).not.toHaveTextContent("email");
    expect(dialog).not.toHaveTextContent("ana@example.com");
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(onRemovePersonalColumns).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Revisar datos personales detectados" }));
    fireEvent.click(screen.getByRole("button", { name: "Retirar datos personales" }));
    expect(onRemovePersonalColumns).toHaveBeenCalledOnce();

    fireEvent.click(screen.getByRole("button", { name: "Proteger valores personales detectados" }));
    const maskDialog = screen.getByRole("alertdialog", { name: "Proteger datos personales detectados" });
    expect(maskDialog).toHaveTextContent("1 columna personal");
    expect(maskDialog).toHaveTextContent("[REDACTED]");
    expect(maskDialog).not.toHaveTextContent("email");
    expect(maskDialog).not.toHaveTextContent("ana@example.com");
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(onMaskPersonalValues).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Proteger valores personales detectados" }));
    fireEvent.click(screen.getByRole("button", { name: "Proteger valores personales" }));
    expect(onMaskPersonalValues).toHaveBeenCalledOnce();
  });

  it("confirma apartar valores incompatibles sin exponer celdas", () => {
    const onNullifyInvalidTypes = vi.fn();
    render(<PreparePhase
      dataset={dataset}
      profileStatus={{ kind: "ready", profile: cleaningSignalsProfile }}
      changeStatus={{ kind: "idle" }}
      historyStatus={EMPTY_HISTORY}
      recipeDraft={null}
      recipeSession={0}
      onAnalyzeQuality={() => undefined}
      onCancelProfile={() => undefined}
      onRemoveDuplicates={() => undefined}
      onRemoveNearDuplicates={() => undefined}
      onRemoveEmptyRows={() => undefined}
      onRemoveConstantColumns={() => undefined}
      onRemoveEmptyColumns={() => undefined}
      onRemoveHighNullColumns={() => undefined}
      onRemoveIdentifierColumns={() => undefined}
      onRemovePersonalColumns={() => undefined}
      onNormalizeSentinels={() => undefined}
      onNormalizeBooleans={() => undefined}
      onFixEncoding={() => undefined}
      onNullifyInvalidTypes={onNullifyInvalidTypes}
      onImputeMissingValues={() => undefined}
      onEnableRowAudit={() => undefined}
      onNormalizeColumns={() => undefined}
      onApplyRecommended={() => undefined}
      onTrimText={() => undefined}
      onNormalizeText={() => undefined}
      onApplyTransforms={() => undefined}
      onRecipeDraftChange={() => undefined}
      onUndo={() => undefined}
      onRedo={() => undefined}
    />);

    fireEvent.click(screen.getByRole("button", { name: "Revisar tipos incompatibles" }));
    const dialog = screen.getByRole("alertdialog", { name: "Apartar valores incompatibles" });
    expect(dialog).toHaveTextContent("1 columna");
    expect(dialog).not.toHaveTextContent("Ana");
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(onNullifyInvalidTypes).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Revisar tipos incompatibles" }));
    fireEvent.click(screen.getByRole("button", { name: "Apartar valores incompatibles" }));
    expect(onNullifyInvalidTypes).toHaveBeenCalledOnce();
  });

  it("ejecuta callbacks opcionales y cierra confirmaciones con Escape", () => {
    render(<PreparePhase
      dataset={dataset}
      profileStatus={{ kind: "ready", profile: cleaningSignalsProfile }}
      changeStatus={{ kind: "idle" }}
      historyStatus={EMPTY_HISTORY}
      recipeDraft={null}
      recipeSession={0}
      onAnalyzeQuality={() => undefined}
      onCancelProfile={() => undefined}
      onRemoveDuplicates={() => undefined}
      onRemoveEmptyRows={() => undefined}
      onRemoveConstantColumns={() => undefined}
      onRemoveEmptyColumns={() => undefined}
      onRemoveHighNullColumns={() => undefined}
      onNormalizeSentinels={() => undefined}
      onNormalizeBooleans={() => undefined}
      onImputeMissingValues={() => undefined}
      onEnableRowAudit={() => undefined}
      onNormalizeColumns={() => undefined}
      onApplyRecommended={() => undefined}
      onTrimText={() => undefined}
      onNormalizeText={() => undefined}
      onApplyTransforms={() => undefined}
      onRecipeDraftChange={() => undefined}
      onUndo={() => undefined}
      onRedo={() => undefined}
    />);

    fireEvent.click(screen.getByRole("button", { name: "Revisar y eliminar parecidos" }));
    fireEvent.keyDown(screen.getByRole("alertdialog", { name: "Eliminar duplicados parecidos" }), { key: "Escape" });
    fireEvent.click(screen.getByRole("button", { name: "Revisar y eliminar parecidos" }));
    fireEvent.click(screen.getByRole("button", { name: "Eliminar duplicados parecidos" }));

    fireEvent.click(screen.getByRole("button", { name: "Revisar identificadores detectados" }));
    fireEvent.keyDown(screen.getByRole("alertdialog", { name: "Retirar identificadores detectados" }), { key: "Escape" });
    fireEvent.click(screen.getByRole("button", { name: "Revisar identificadores detectados" }));
    fireEvent.click(screen.getByRole("button", { name: "Retirar identificadores" }));

    fireEvent.click(screen.getByRole("button", { name: "Revisar datos personales detectados" }));
    fireEvent.keyDown(screen.getByRole("alertdialog", { name: "Retirar datos personales detectados" }), { key: "Escape" });
    fireEvent.click(screen.getByRole("button", { name: "Revisar datos personales detectados" }));
    fireEvent.click(screen.getByRole("button", { name: "Retirar datos personales" }));

    fireEvent.click(screen.getByRole("button", { name: "Proteger valores personales detectados" }));
    fireEvent.keyDown(screen.getByRole("alertdialog", { name: "Proteger datos personales detectados" }), { key: "Escape" });
    fireEvent.click(screen.getByRole("button", { name: "Proteger valores personales detectados" }));
    fireEvent.click(screen.getByRole("button", { name: "Proteger valores personales" }));
  });

  it("cubre estados de análisis, navegación de tabs y limpieza de texto seleccionada", () => {
    const callbacks = {
      onAnalyzeQuality: vi.fn(),
      onCancelProfile: vi.fn(),
      onRemoveDuplicates: vi.fn(),
      onRemoveNearDuplicates: vi.fn(),
      onRemoveEmptyRows: vi.fn(),
      onRemoveConstantColumns: vi.fn(),
      onRemoveEmptyColumns: vi.fn(),
      onRemoveHighNullColumns: vi.fn(),
      onRemoveIdentifierColumns: vi.fn(),
      onRemovePersonalColumns: vi.fn(),
      onNormalizeSentinels: vi.fn(),
      onNormalizeBooleans: vi.fn(),
      onFixEncoding: vi.fn(),
      onImputeMissingValues: vi.fn(),
      onEnableRowAudit: vi.fn(),
      onNormalizeColumns: vi.fn(),
      onApplyRecommended: vi.fn(),
      onTrimText: vi.fn(),
      onNormalizeText: vi.fn(),
      onApplyTransforms: vi.fn(),
      onRecipeDraftChange: vi.fn(),
      onUndo: vi.fn(),
      onRedo: vi.fn(),
    };
    render(<PreparePhase
      dataset={dataset}
      profileStatus={{ kind: "idle" }}
      changeStatus={{ kind: "idle" }}
      historyStatus={{ ...EMPTY_HISTORY, canRedo: true }}
      recipeDraft={null}
      recipeSession={0}
      {...callbacks}
    />);

    fireEvent.click(screen.getByRole("button", { name: "Analizar antes de preparar" }));
    expect(callbacks.onAnalyzeQuality).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("tab", { name: "Transformaciones" }));
    fireEvent.keyDown(screen.getByRole("tab", { name: "Transformaciones" }), { key: "ArrowLeft" });
    expect(screen.getByRole("tab", { name: "Correcciones" })).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(screen.getByRole("tab", { name: "Correcciones" }), { key: "ArrowRight" });
    expect(screen.getByRole("tab", { name: "Transformaciones" })).toHaveAttribute("aria-selected", "true");

    cleanup();
    render(<PreparePhase
      dataset={dataset}
      profileStatus={{
        kind: "loading",
        progress: { operation: "profile", stage: "Columnas", percent: 20 },
        cancelRequested: false,
      }}
      changeStatus={{ kind: "idle" }}
      historyStatus={EMPTY_HISTORY}
      recipeDraft={null}
      recipeSession={0}
      {...callbacks}
    />);
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(callbacks.onCancelProfile).toHaveBeenCalledOnce();

    cleanup();
    render(<PreparePhase
      dataset={dataset}
      profileStatus={{
        kind: "loading",
        progress: { operation: "profile", stage: "Columnas", percent: 20 },
        cancelRequested: true,
      }}
      changeStatus={{ kind: "idle" }}
      historyStatus={EMPTY_HISTORY}
      recipeDraft={null}
      recipeSession={0}
      {...callbacks}
    />);
    expect(screen.getByRole("button", { name: "Cancelando…" })).toBeDisabled();

    cleanup();
    render(<PreparePhase
      dataset={dataset}
      profileStatus={{ kind: "ready", profile: cleaningSignalsProfile }}
      changeStatus={{ kind: "idle" }}
      historyStatus={EMPTY_HISTORY}
      recipeDraft={null}
      recipeSession={0}
      {...callbacks}
    />);
    fireEvent.click(screen.getByText("Correcciones avanzadas"));
    fireEvent.click(screen.getByLabelText("nombre"));
    fireEvent.click(screen.getByLabelText("nombre"));
    fireEvent.click(screen.getByLabelText("nombre"));
    fireEvent.click(screen.getByLabelText("Eliminar acentos"));
    fireEvent.click(screen.getByRole("button", { name: "Normalizar texto seleccionado" }));
    expect(callbacks.onNormalizeText).toHaveBeenCalledWith(["nombre"], false);
    fireEvent.click(screen.getByRole("button", { name: "Recortar espacios" }));
    expect(callbacks.onTrimText).toHaveBeenCalledOnce();

    cleanup();
    render(<PreparePhase
      dataset={dataset}
      profileStatus={{ kind: "error", message: "perfil no disponible" }}
      changeStatus={{ kind: "idle" }}
      historyStatus={EMPTY_HISTORY}
      recipeDraft={null}
      recipeSession={0}
      {...callbacks}
    />);
    expect(screen.getByRole("alert")).toHaveTextContent("perfil no disponible");
  });
});

describe("TransformRecipeEditor", () => {
  it("inicializa un borrador restaurado y emite cambios en una sola dirección", async () => {
    const onDraftChange = vi.fn();
    const restored: LoadedRecipe = {
      version: 1,
      name: "Limpieza persistida",
      savedAt: "2026-08-21T00:00:00Z",
      recipe: { ...emptyRecipe, renames: [{ from: "nombre", to: "cliente" }] },
    };
    render(<TransformRecipeEditor
      dataset={dataset}
      busy={false}
      initialDraft={restored}
      onApply={() => undefined}
      onDraftChange={onDraftChange}
    />);

    expect(screen.getByRole("textbox", { name: "Nombre de la receta" })).toHaveValue("Limpieza persistida");
    expect(screen.getByRole("combobox", { name: "Columna para renombrar 1" })).toHaveValue("nombre");
    expect(screen.getByRole("textbox", { name: "Nuevo nombre 1" })).toHaveValue("cliente");
    expect(onDraftChange).not.toHaveBeenCalled();

    fireEvent.change(screen.getByRole("textbox", { name: "Nuevo nombre 1" }), { target: { value: "persona" } });
    await waitFor(() => expect(onDraftChange).toHaveBeenCalledWith({
      ...restored,
      recipe: { ...emptyRecipe, renames: [{ from: "nombre", to: "persona" }] },
    }));
    expect(onDraftChange).toHaveBeenCalledTimes(1);
  });

  it("conserva el último borrador válido durante una edición transitoria inválida", async () => {
    const onDraftChange = vi.fn();
    const restored: LoadedRecipe = {
      version: 1,
      name: "Limpieza persistida",
      savedAt: "2026-08-21T00:00:00Z",
      recipe: { ...emptyRecipe, renames: [{ from: "nombre", to: "cliente" }] },
    };
    render(<TransformRecipeEditor
      dataset={dataset}
      busy={false}
      initialDraft={restored}
      onApply={() => undefined}
      onDraftChange={onDraftChange}
    />);

    fireEvent.change(screen.getByRole("textbox", { name: "Nuevo nombre 1" }), {
      target: { value: "" },
    });
    await waitFor(() => expect(screen.getByRole("textbox", { name: "Nuevo nombre 1" })).toHaveValue(""));
    expect(onDraftChange).not.toHaveBeenCalled();

    fireEvent.change(screen.getByRole("textbox", { name: "Nuevo nombre 1" }), {
      target: { value: "persona" },
    });
    await waitFor(() => expect(onDraftChange).toHaveBeenCalledWith({
      ...restored,
      recipe: { ...emptyRecipe, renames: [{ from: "nombre", to: "persona" }] },
    }));
    expect(onDraftChange).toHaveBeenCalledTimes(1);
  });

  it("guarda el borrador validado con el nombre visible", async () => {
    const saved: LoadedRecipe = {
      version: 1,
      name: "Mi receta",
      savedAt: "2026-08-21T00:00:00Z",
      recipe: { ...emptyRecipe, renames: [{ from: "nombre", to: "cliente" }] },
    };
    const save = vi.spyOn(bridge, "saveTransformRecipe").mockResolvedValue(saved);
    render(<TransformRecipeEditor dataset={dataset} busy={false} initialDraft={null} onApply={() => undefined} onDraftChange={() => undefined} />);
    fireEvent.change(screen.getByLabelText("Columna para renombrar 1"), { target: { value: "nombre" } });
    fireEvent.change(screen.getByLabelText("Nuevo nombre 1"), { target: { value: "cliente" } });
    fireEvent.click(screen.getByRole("button", { name: "Guardar receta" }));

    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Receta guardada: Mi receta"));
    expect(save).toHaveBeenCalledWith(expect.objectContaining({
      renames: [{ from: "nombre", to: "cliente" }],
    }), "Mi receta");
  });

  it("expone la imputación de outliers en recetas y solicita confirmación", () => {
    const onApply = vi.fn();
    const initialDraft: LoadedRecipe = {
      version: 1,
      name: "Imputación IQR",
      savedAt: "2026-08-29T00:00:00Z",
      recipe: {
        ...emptyRecipe,
        outlierTreatments: [{ column: "total", action: "impute" }],
      },
    };
    render(<TransformRecipeEditor
      dataset={dataset}
      busy={false}
      initialDraft={initialDraft}
      onApply={onApply}
      onDraftChange={() => undefined}
    />);

    expect(screen.getByRole("combobox", { name: "Acción de outliers 1" })).toHaveValue("impute");
    fireEvent.click(screen.getByRole("button", { name: "Aplicar receta" }));
    expect(screen.getByRole("alertdialog")).toHaveTextContent("reemplazarán valores atípicos por la mediana");
    fireEvent.click(screen.getByRole("button", { name: "Confirmar y aplicar" }));
    expect(onApply).toHaveBeenCalledWith(expect.objectContaining({
      outlierTreatments: [{ column: "total", action: "impute" }],
    }));
  });

  it("interpone el alertdialog antes de aplicar filtros destructivos", () => {
    const onApply = vi.fn();
    render(<TransformRecipeEditor dataset={dataset} busy={false} initialDraft={null} onApply={onApply} onDraftChange={() => undefined} />);
    fireEvent.click(screen.getByRole("button", { name: "+ Añadir filtro AND" }));
    fireEvent.change(screen.getByLabelText("Columna del filtro 1"), { target: { value: "nombre" } });
    fireEvent.change(screen.getByLabelText("Valor del filtro 1"), { target: { value: "Ana" } });
    fireEvent.click(screen.getByRole("button", { name: "Aplicar receta" }));

    expect(onApply).not.toHaveBeenCalled();
    expect(screen.getByRole("alertdialog")).toHaveTextContent("1 filtros unidos por AND");
    fireEvent.click(screen.getByRole("button", { name: "Confirmar y aplicar" }));
    expect(onApply).toHaveBeenCalledWith(expect.objectContaining({
      filters: [{ column: "nombre", operator: "eq", value: "Ana" }],
    }));
  });

  it("confirma antes de reemplazar un borrador al cargar receta", async () => {
    const loaded: LoadedRecipe = {
      version: 1,
      name: "Receta cargada",
      savedAt: "2026-08-21T00:00:00Z",
      recipe: { ...emptyRecipe, renames: [{ from: "nombre", to: "cliente" }] },
    };
    vi.spyOn(bridge, "pickTransformRecipe").mockResolvedValue(loaded);
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    render(<TransformRecipeEditor dataset={dataset} busy={false} initialDraft={null} onApply={() => undefined} onDraftChange={() => undefined} />);
    fireEvent.change(screen.getByLabelText("Columna para renombrar 1"), { target: { value: "nombre" } });
    fireEvent.change(screen.getByLabelText("Nuevo nombre 1"), { target: { value: "borrador" } });
    fireEvent.click(screen.getByRole("button", { name: "Cargar receta" }));

    await waitFor(() => expect(confirm).toHaveBeenCalledOnce());
    expect(screen.getByLabelText("Nuevo nombre 1")).toHaveValue("borrador");
    expect(screen.queryByText(/Receta cargada:/)).not.toBeInTheDocument();
  });

  it("muestra el informe de compatibilidad y las opciones de entrega migradas", async () => {
    const loaded: LoadedRecipe = {
      version: 1,
      name: "Pipeline DataPrep",
      savedAt: "2026-08-24T00:00:00Z",
      recipe: { ...emptyRecipe, renames: [{ from: "nombre", to: "cliente" }] },
      exportOptions: { formats: ["csv", "excel"], selectedColumns: ["cliente"], privacyMode: "mask" },
      migrationReport: {
        artifactSha256: "a".repeat(64),
        sourceFormat: "dataprep",
        sourceVersion: 3,
        convertedItems: 4,
        omittedItems: 2,
        warningCount: 2,
        convertedOperations: ["renames", "export.formats"],
        omittedOperations: ["export.report_format"],
        warnings: [{ path: "export.report_format", severity: "omitted", message: "Revisión manual." }],
        manualActions: ["Abrir C:\\Users\\Ana\\pipeline.json y revisar valor 'secreto'."],
        session: {
          hasSourceReference: true,
          hasSnapshotReference: true,
          sheetName: "Datos",
          stageLabel: "Transformación",
          appliedOperationCount: 1,
          qualityRuleCount: 2,
          analysisCheckCount: 3,
        },
      },
    };
    vi.spyOn(bridge, "pickTransformRecipe").mockResolvedValue(loaded);
    render(<TransformRecipeEditor dataset={dataset} busy={false} initialDraft={null} onApply={() => undefined} onDraftChange={() => undefined} />);
    fireEvent.click(screen.getByRole("button", { name: "Cargar receta" }));

    const report = await waitFor(() => screen.getByLabelText("Informe de migración de receta"));
    expect(report).toHaveTextContent("Convertidos");
    expect(report).toHaveTextContent("4");
    expect(report).toHaveTextContent("Entrega importada: CSV, Excel");
    expect(report).toHaveTextContent("Acciones manuales");
    expect(report).toHaveTextContent("Contexto de sesión");
    expect(report).toHaveTextContent("Etapa de trabajo");
    expect(report).toHaveTextContent("Hoja de origen");
    expect(report).toHaveTextContent("Operaciones aplicadas");
    expect(report).toHaveTextContent("Reglas de calidad");
    expect(report).toHaveTextContent("Comprobaciones");
    expect(report).toHaveTextContent("Se detectaron referencias de origen o snapshot");
    expect(report).not.toHaveTextContent("Transformación");
    expect(report).not.toHaveTextContent("Datos");
    expect(report).not.toHaveTextContent("a".repeat(64));
    expect(report).not.toHaveTextContent("export.report_format");
    expect(report).not.toHaveTextContent("C:\\Users\\Ana\\pipeline.json");
    expect(report).not.toHaveTextContent("secreto");
  });
});
