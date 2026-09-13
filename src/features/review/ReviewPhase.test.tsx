import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type {
  DatasetPreview,
  DatasetProfile,
  DatasetQueryResult,
  TemporalAggregationSeries,
} from "../../bridge";
import * as bridge from "../../bridge";
import { DatasetPreviewPanel, ReviewPhase } from "./ReviewPhase";
import { createReadyDatasetStatus } from "../load/loadModel";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const dataset: DatasetPreview = {
  fileName: "datos.csv",
  fileSizeBytes: 100,
  rowCount: 120,
  columnCount: 2,
  columns: [
    { name: "id", dataType: "String" },
    { name: "nota", dataType: "String" },
  ],
  rows: [["51", null], ["52", "ok"]],
};

const profile: DatasetProfile = {
  rowCount: 120,
  duplicateRowCount: 3,
  nearDuplicateRowCount: 0,
  duplicatePercentage: 2.5,
  numericCorrelations: {
    columns: ["id", "nombre"],
    pairs: [
      { firstColumn: "id", secondColumn: "nombre", coefficient: -0.42, sampleCount: 114 },
    ],
    sampledRowCount: 120,
    truncated: false,
  },
  categoricalGroupSummaries: [
    {
      column: "estado",
      distinctCount: 3,
      truncated: true,
      groups: [
        { label: "Activo", rowCount: 72, percentage: 60, isOther: false },
        { label: "Resto", rowCount: 48, percentage: 40, isOther: true },
      ],
    },
  ],
  columns: [
    {
      name: "id",
      dataType: "Int64",
      nullCount: 0,
      completenessPercentage: 100,
      uniqueCount: 120,
      minimum: "1",
      maximum: "120",
      mean: 60.5,
      emptyCount: null,
      minimumLength: null,
      maximumLength: null,
      averageLength: null,
      suggestedType: null,
      typeMatchPercentage: null,
      invalidTypeCount: null,
      sentinelCount: null,
      encodingIssueCount: null,
      privacySignal: null,
      standardDeviation: 34.6,
      firstQuartile: 30,
      median: 60,
      thirdQuartile: 90,
      outlierCount: 2,
      histogram: [
        { lower: 1, upper: 30.75, count: 20 },
        { lower: 30.75, upper: 60.5, count: 35 },
        { lower: 60.5, upper: 90.25, count: 40 },
        { lower: 90.25, upper: 120, count: 25 },
      ],
    },
    {
      name: "nombre",
      dataType: "String",
      nullCount: 6,
      completenessPercentage: 95,
      uniqueCount: 110,
      minimum: "Ana",
      maximum: "Zoe",
      mean: null,
      emptyCount: 2,
      minimumLength: 3,
      maximumLength: 12,
      averageLength: 6.4,
      suggestedType: null,
      typeMatchPercentage: 95,
      invalidTypeCount: 6,
      sentinelCount: null,
      encodingIssueCount: null,
      privacySignal: null,
      standardDeviation: null,
      firstQuartile: null,
      median: null,
      thirdQuartile: null,
      outlierCount: null,
      histogram: null,
    },
  ],
};

const temporalProfile: DatasetProfile = {
  ...profile,
  temporalSeries: [
    {
      column: "fecha",
      granularity: "month",
      parsedRowCount: 108,
      unparsedRowCount: 12,
      truncated: false,
      periods: [
        { period: "2024-01", rowCount: 24, percentage: 22.2 },
        { period: "2024-02", rowCount: 36, percentage: 33.3 },
        { period: "2024-03", rowCount: 48, percentage: 44.4 },
      ],
    },
  ],
  columns: [
    ...profile.columns,
    {
      ...profile.columns[0],
      name: "fecha",
      dataType: "Date",
      nullCount: 12,
      completenessPercentage: 90,
      uniqueCount: 100,
      minimum: "2024-01-01",
      maximum: "2024-12-31",
      mean: null,
      emptyCount: null,
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
    },
    {
      ...profile.columns[0],
      name: "actualizado_en",
      dataType: "Datetime(time_unit='ms', time_zone='UTC')",
      nullCount: 0,
      completenessPercentage: 100,
      uniqueCount: 120,
      minimum: "2024-01-01T08:00:00+00:00",
      maximum: "2024-12-31T18:30:00+00:00",
      mean: null,
      emptyCount: null,
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
    },
  ],
};

const dailyTemporalProfile: DatasetProfile = {
  ...temporalProfile,
  temporalSeries: [
    {
      column: "fecha",
      granularity: "day",
      parsedRowCount: 4,
      unparsedRowCount: 0,
      truncated: false,
      periods: [
        { period: "2024-04-01", rowCount: 1, percentage: 25 },
        { period: "2024-04-02", rowCount: 0, percentage: 0 },
        { period: "2024-04-03", rowCount: 2, percentage: 50 },
        { period: "2024-04-04", rowCount: 0, percentage: 0 },
        { period: "2024-04-05", rowCount: 1, percentage: 25 },
      ],
    },
  ],
};

const emptyDailyTemporalProfile: DatasetProfile = {
  ...dailyTemporalProfile,
  temporalSeries: [
    {
      ...dailyTemporalProfile.temporalSeries![0],
      parsedRowCount: 0,
      unparsedRowCount: 120,
      periods: [],
    },
  ],
};

const inferredTemporalProfile: DatasetProfile = {
  ...temporalProfile,
  columns: temporalProfile.columns.map((column) =>
    column.name === "fecha"
      ? {
          ...column,
          dataType: "str",
          minimum: "2024-01-15",
          maximum: "2024-03-28",
          suggestedType: "date",
          typeMatchPercentage: 100,
          invalidTypeCount: 0,
        }
      : column,
  ),
};

const numericTemporalProfile: DatasetProfile = {
  ...temporalProfile,
  columns: [
    ...temporalProfile.columns,
    {
      ...profile.columns[0],
      name: "ventas",
      nullCount: 5,
      completenessPercentage: 95,
      uniqueCount: 90,
      minimum: "0",
      maximum: "1000",
      mean: 420,
      outlierCount: 0,
    },
  ],
};

const multipleTemporalProfile: DatasetProfile = {
  ...numericTemporalProfile,
  temporalSeries: [
    ...numericTemporalProfile.temporalSeries!,
    {
      ...numericTemporalProfile.temporalSeries![0],
      column: "fecha_entrega",
    },
  ],
  columns: [
    ...numericTemporalProfile.columns,
    {
      ...numericTemporalProfile.columns.find((column) => column.name === "fecha")!,
      name: "fecha_entrega",
    },
  ],
};

const temporalMeanSeries: TemporalAggregationSeries = {
  dateColumn: "fecha",
  valueColumn: "ventas",
  aggregation: "mean",
  granularity: "month",
  parsedRowCount: 108,
  unparsedRowCount: 12,
  truncated: false,
  periods: [
    { period: "2024-01", rowCount: 24, valueCount: 3, value: 25 },
    { period: "2024-02", rowCount: 36, valueCount: 0, value: null },
    { period: "2024-03", rowCount: 48, valueCount: 2, value: 40 },
  ],
};

function temporalTrendElement(
  profileForTest: DatasetProfile = numericTemporalProfile,
  datasetRevision = 17,
) {
  return (
    <ReviewPhase
      datasetStatus={createReadyDatasetStatus(dataset)}
      profileStatus={{ kind: "ready", profile: profileForTest }}
      reviewTab="diagnosis"
      onTabChange={() => undefined}
      onPageChange={() => undefined}
      onAnalyzeQuality={() => undefined}
      onCancelProfile={() => undefined}
      comparisonStatus={{ kind: "idle" }}
      datasetColumns={dataset.columns}
      comparisonKeyColumns={[]}
      onComparisonKeyColumnsChange={() => undefined}
      onCompare={() => undefined}
      onClearComparison={() => undefined}
      onConsolidate={() => undefined}
      onResolveConflicts={() => undefined}
      onConflictPageChange={() => undefined}
      joinStatus={{ kind: "idle" }}
      joinType="inner"
      onJoinTypeChange={() => undefined}
      onJoin={() => undefined}
      datasetRevision={datasetRevision}
    />
  );
}

function renderTemporalTrend(
  profileForTest: DatasetProfile = numericTemporalProfile,
  datasetRevision = 17,
) {
  return render(temporalTrendElement(profileForTest, datasetRevision));
}

describe("ReviewPhase", () => {
  it("conserva tabpanel ARIA y perfil bajo demanda", () => {
    const onAnalyzeQuality = vi.fn();
    const onAnalysisSampleRowsChange = vi.fn();
    render(
      <ReviewPhase
        datasetStatus={createReadyDatasetStatus(dataset)}
        profileStatus={{ kind: "idle" }}
        reviewTab="diagnosis"
        onTabChange={() => undefined}
        onPageChange={() => undefined}
        onAnalyzeQuality={onAnalyzeQuality}
        onCancelProfile={() => undefined}
        comparisonStatus={{ kind: "idle" }}
        datasetColumns={dataset.columns}
        comparisonKeyColumns={[]}
        onComparisonKeyColumnsChange={() => undefined}
        onCompare={() => undefined}
        onClearComparison={() => undefined}
        onConsolidate={() => undefined}
        onResolveConflicts={() => undefined}
        onConflictPageChange={() => undefined}
        joinStatus={{ kind: "idle" }}
        joinType="inner"
        onJoinTypeChange={() => undefined}
        onJoin={() => undefined}
        analysisSampleRows={50_000}
        onAnalysisSampleRowsChange={onAnalysisSampleRowsChange}
      />,
    );

    expect(screen.getByRole("tabpanel", { name: "Diagnóstico" })).toHaveAttribute(
      "aria-labelledby",
      "review-diagnosis-tab",
    );
    fireEvent.click(screen.getByRole("button", { name: "Analizar calidad" }));
    expect(onAnalyzeQuality).toHaveBeenCalledOnce();
    expect(screen.getByRole("combobox", { name: "Filas de muestra para correlaciones" })).toHaveValue("50000");
    fireEvent.change(screen.getByRole("combobox", { name: "Filas de muestra para correlaciones" }), {
      target: { value: "10000" },
    });
    expect(onAnalysisSampleRowsChange).toHaveBeenCalledWith(10_000);
  });

  it("muestra diferencias de fuentes y permite consolidar un esquema compatible", () => {
    const onCompare = vi.fn();
    const onClearComparison = vi.fn();
    const onConsolidate = vi.fn();
    const onComparisonKeyColumnsChange = vi.fn();
    const onJoin = vi.fn();
    const onJoinTypeChange = vi.fn();
    render(
      <ReviewPhase
        datasetStatus={createReadyDatasetStatus(dataset)}
        profileStatus={{ kind: "idle" }}
        reviewTab="diagnosis"
        onTabChange={() => undefined}
        onPageChange={() => undefined}
        onAnalyzeQuality={() => undefined}
        onCancelProfile={() => undefined}
        comparisonStatus={{
          kind: "ready",
          comparison: {
            currentFileName: "datos.csv",
            comparedFileName: "actualizacion.csv",
            currentRowCount: 2,
            comparedRowCount: 2,
            commonRowCount: 1,
            currentOnlyRowCount: 1,
            comparedOnlyRowCount: 1,
            sharedColumns: ["id"],
            currentOnlyColumns: [],
            comparedOnlyColumns: [],
            schemaCompatible: true,
            keyColumns: ["id"],
            matchedKeyCount: 1,
            currentOnlyKeyCount: 0,
            comparedOnlyKeyCount: 0,
            conflictingKeyCount: 0,
            duplicateKeyCount: 0,
            conflicts: [],
            conflictOffset: 0,
            conflictsTruncated: false,
            canConsolidate: true,
          },
        }}
        onCompare={onCompare}
        datasetColumns={dataset.columns}
        comparisonKeyColumns={["id"]}
        onComparisonKeyColumnsChange={onComparisonKeyColumnsChange}
        onClearComparison={onClearComparison}
        onConsolidate={onConsolidate}
        onResolveConflicts={() => undefined}
        onConflictPageChange={() => undefined}
        joinStatus={{ kind: "idle" }}
        joinType="inner"
        onJoinTypeChange={onJoinTypeChange}
        onJoin={onJoin}
      />,
    );

    expect(screen.getByText("actualizacion.csv")).toBeInTheDocument();
    expect(screen.getByText("Filas compartidas")).toBeInTheDocument();
    expect(screen.getByText(/La comparación cargada también está disponible como compared/)).toBeInTheDocument();
    expect(screen.getByText(/SELECT id, segmento FROM dataset LEFT JOIN compared/)).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: /id/ })).toBeChecked();
    fireEvent.click(screen.getByRole("checkbox", { name: /id/ }));
    fireEvent.click(screen.getByRole("radio", { name: /^Left/ }));
    fireEvent.click(screen.getByRole("button", { name: "Consolidar filas" }));
    fireEvent.click(screen.getByRole("button", { name: "Descartar comparación" }));
    fireEvent.click(screen.getByRole("button", { name: "Elegir fuente y unir" }));
    expect(onConsolidate).toHaveBeenCalledOnce();
    expect(onClearComparison).toHaveBeenCalledOnce();
    expect(onComparisonKeyColumnsChange).toHaveBeenCalledWith([]);
    expect(onJoinTypeChange).toHaveBeenCalledWith("left");
    expect(onJoin).toHaveBeenCalledWith("inner");
  });

  it("ejecuta la consulta SQL segura y muestra el resultado accesible", async () => {
    const onSqlHistoryChange = vi.fn();
    vi.spyOn(bridge, "queryDataset").mockResolvedValue({
      columns: [{ name: "id", dataType: "Int64" }],
      rowCount: 2,
      offset: 0,
      rows: [["1"]],
      truncated: true,
    });
    render(
      <ReviewPhase
        datasetStatus={createReadyDatasetStatus(dataset)}
        profileStatus={{ kind: "idle" }}
        reviewTab="diagnosis"
        onTabChange={() => undefined}
        onPageChange={() => undefined}
        onAnalyzeQuality={() => undefined}
        onCancelProfile={() => undefined}
        comparisonStatus={{ kind: "idle" }}
        datasetColumns={dataset.columns}
        comparisonKeyColumns={[]}
        onComparisonKeyColumnsChange={() => undefined}
        onCompare={() => undefined}
        onClearComparison={() => undefined}
        onConsolidate={() => undefined}
        onResolveConflicts={() => undefined}
        onConflictPageChange={() => undefined}
        joinStatus={{ kind: "idle" }}
        joinType="inner"
        onJoinTypeChange={() => undefined}
        onJoin={() => undefined}
        sqlHistory={[{ id: 9, outcome: "error", durationMs: 18, rowCount: null }]}
        onSqlHistoryChange={onSqlHistoryChange}
      />,
    );

    fireEvent.change(screen.getByRole("textbox", { name: "Consulta SQL de solo lectura" }), {
      target: { value: "SELECT id FROM dataset LIMIT 1" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Ejecutar consulta" }));

    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("2 filas disponibles"));
    expect(screen.getByRole("status")).toHaveTextContent("resultado truncado por LIMIT");
    expect(screen.getByRole("region", { name: "Resultado de consulta SQL" })).toHaveTextContent("id");
    expect(screen.getByRole("heading", { name: "Actividad reciente" })).toBeInTheDocument();
    expect(screen.getByRole("list", { name: "Historial de consultas SQL" })).toHaveTextContent("Completada");
    expect(screen.getByRole("list", { name: "Historial de consultas SQL" })).toHaveTextContent("Error");
    expect(onSqlHistoryChange).toHaveBeenCalledWith([
      expect.objectContaining({
        id: 10,
        outcome: "success",
        durationMs: expect.any(Number),
        rowCount: 2,
      }),
      { id: 9, outcome: "error", durationMs: 18, rowCount: null },
    ]);
    expect(bridge.queryDataset).toHaveBeenCalledWith("SELECT id FROM dataset LIMIT 1", "polars");
  });

  it("permite cancelar una consulta y no pinta una respuesta tardía", async () => {
    let resolveQuery: (result: DatasetQueryResult) => void = () => undefined;
    const pendingQuery = new Promise<DatasetQueryResult>((resolve) => {
      resolveQuery = resolve;
    });
    vi.spyOn(bridge, "queryDataset").mockReturnValue(pendingQuery);
    vi.spyOn(bridge, "cancelOperation").mockResolvedValue(undefined);

    render(
      <ReviewPhase
        datasetStatus={createReadyDatasetStatus(dataset)}
        profileStatus={{ kind: "idle" }}
        reviewTab="diagnosis"
        onTabChange={() => undefined}
        onPageChange={() => undefined}
        onAnalyzeQuality={() => undefined}
        onCancelProfile={() => undefined}
        comparisonStatus={{ kind: "idle" }}
        datasetColumns={dataset.columns}
        comparisonKeyColumns={[]}
        onComparisonKeyColumnsChange={() => undefined}
        onCompare={() => undefined}
        onClearComparison={() => undefined}
        onConsolidate={() => undefined}
        onResolveConflicts={() => undefined}
        onConflictPageChange={() => undefined}
        joinStatus={{ kind: "idle" }}
        joinType="inner"
        onJoinTypeChange={() => undefined}
        onJoin={() => undefined}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Ejecutar consulta" }));
    expect(screen.getByRole("button", { name: "Cancelar consulta" })).toBeEnabled();

    fireEvent.click(screen.getByRole("button", { name: "Cancelar consulta" }));
    await waitFor(() => {
      expect(bridge.cancelOperation).toHaveBeenCalledWith("query");
      expect(screen.getByRole("status")).toHaveTextContent("Consulta cancelada");
    });
    expect(screen.getByRole("list", { name: "Historial de consultas SQL" })).toHaveTextContent("Cancelada");

    resolveQuery({
      columns: [{ name: "id", dataType: "Int64" }],
      rowCount: 1,
      offset: 0,
      rows: [["1"]],
      truncated: false,
    });
    await Promise.resolve();
    expect(screen.queryByRole("region", { name: "Resultado de consulta SQL" })).not.toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("No se actualizó el resultado");
  });

  it("descarta el resultado de una consulta cuando cambia la revisión del dataset", async () => {
    let resolveQuery: (result: DatasetQueryResult) => void = () => undefined;
    const pendingQuery = new Promise<DatasetQueryResult>((resolve) => {
      resolveQuery = resolve;
    });
    vi.spyOn(bridge, "queryDataset").mockReturnValue(pendingQuery);
    const nextDataset = { ...dataset, fileName: "nuevo.csv", fileSizeBytes: 101 };
    const view = render(
      <ReviewPhase
        datasetStatus={createReadyDatasetStatus(dataset)}
        datasetRevision={0}
        profileStatus={{ kind: "idle" }}
        reviewTab="diagnosis"
        onTabChange={() => undefined}
        onPageChange={() => undefined}
        onAnalyzeQuality={() => undefined}
        onCancelProfile={() => undefined}
        comparisonStatus={{ kind: "idle" }}
        datasetColumns={dataset.columns}
        comparisonKeyColumns={[]}
        onComparisonKeyColumnsChange={() => undefined}
        onCompare={() => undefined}
        onClearComparison={() => undefined}
        onConsolidate={() => undefined}
        onResolveConflicts={() => undefined}
        onConflictPageChange={() => undefined}
        joinStatus={{ kind: "idle" }}
        joinType="inner"
        onJoinTypeChange={() => undefined}
        onJoin={() => undefined}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Ejecutar consulta" }));
    view.rerender(
      <ReviewPhase
        datasetStatus={createReadyDatasetStatus(nextDataset)}
        datasetRevision={1}
        profileStatus={{ kind: "idle" }}
        reviewTab="diagnosis"
        onTabChange={() => undefined}
        onPageChange={() => undefined}
        onAnalyzeQuality={() => undefined}
        onCancelProfile={() => undefined}
        comparisonStatus={{ kind: "idle" }}
        datasetColumns={nextDataset.columns}
        comparisonKeyColumns={[]}
        onComparisonKeyColumnsChange={() => undefined}
        onCompare={() => undefined}
        onClearComparison={() => undefined}
        onConsolidate={() => undefined}
        onResolveConflicts={() => undefined}
        onConflictPageChange={() => undefined}
        joinStatus={{ kind: "idle" }}
        joinType="inner"
        onJoinTypeChange={() => undefined}
        onJoin={() => undefined}
      />,
    );
    resolveQuery({
      columns: [{ name: "id", dataType: "Int64" }],
      rowCount: 1,
      offset: 0,
      rows: [["1"]],
      truncated: false,
    });
    await Promise.resolve();
    expect(screen.queryByRole("region", { name: "Resultado de consulta SQL" })).not.toBeInTheDocument();
  });

  it("muestra visualizaciones accesibles con valores equivalentes al perfil", () => {
    const sampledProfile: DatasetProfile = {
      ...profile,
      numericCorrelations: {
        ...profile.numericCorrelations!,
        sampledRowCount: 87,
        truncated: true,
      },
    };
    render(
      <ReviewPhase
        datasetStatus={createReadyDatasetStatus(dataset)}
        profileStatus={{ kind: "ready", profile: sampledProfile }}
        reviewTab="diagnosis"
        onTabChange={() => undefined}
        onPageChange={() => undefined}
        onAnalyzeQuality={() => undefined}
        onCancelProfile={() => undefined}
        comparisonStatus={{ kind: "idle" }}
        datasetColumns={dataset.columns}
        comparisonKeyColumns={[]}
        onComparisonKeyColumnsChange={() => undefined}
        onCompare={() => undefined}
        onClearComparison={() => undefined}
        onConsolidate={() => undefined}
        onResolveConflicts={() => undefined}
        onConflictPageChange={() => undefined}
        joinStatus={{ kind: "idle" }}
        joinType="inner"
        onJoinTypeChange={() => undefined}
        onJoin={() => undefined}
      />,
    );

    expect(screen.getByRole("heading", { name: "Señales del perfil" })).toBeInTheDocument();
    expect(screen.getByRole("list", { name: "Completitud por columna" })).toHaveTextContent(
      "nombre95.0%",
    );
    expect(screen.getByRole("list", { name: "Posibles outliers por columna" })).toHaveTextContent(
      "id2",
    );
    expect(screen.getByRole("list", { name: "Patrones de nulos por columna" })).toHaveTextContent(
      "nombre6 nulos",
    );
    expect(screen.getByRole("table", { name: "Tabla de patrones de nulos" })).toHaveTextContent(
      "5.0%",
    );
    expect(screen.getByRole("list", { name: "Validación de formato por columna" })).toHaveTextContent(
      "nombre95.0%",
    );
    expect(screen.getByRole("table", { name: "Tabla de validación de formato" })).toHaveTextContent(
      "6",
    );
    expect(screen.getByText("Filas analizadas").parentElement).toHaveTextContent("120");
    expect(screen.getByRole("heading", { name: "Distribución numérica" })).toBeInTheDocument();
    expect(screen.getByRole("list", { name: "Distribución numérica por columna" })).toHaveTextContent(
      "Q1 30 · Mediana 60 · Q3 90",
    );
    expect(screen.getByRole("heading", { name: "Histograma numérico" })).toBeInTheDocument();
    expect(screen.getByRole("table", { name: "Tabla de frecuencias para id" })).toHaveTextContent(
      "20",
    );
    expect(screen.getByRole("heading", { name: "Distribución por categoría" })).toBeInTheDocument();
    expect(screen.getByRole("list", { name: "Distribución de grupos para estado" })).toHaveTextContent(
      "Activo72 filas",
    );
    expect(screen.getByRole("table", { name: "Resumen de grupos para estado" })).toHaveTextContent(
      "Resto",
    );
    expect(screen.getByRole("heading", { name: "Correlaciones numéricas" })).toBeInTheDocument();
    expect(screen.getByText(/Pearson entre pares disponibles/)).toHaveTextContent(
      "La lectura usa 87 filas y muestra las primeras 12 columnas numéricas.",
    );
    expect(
      screen.getByRole("region", { name: "Matriz de correlaciones numéricas" }),
    ).toHaveTextContent("-0.42");
    expect(screen.getByText(/el resto está agrupado para evitar ruido y preservar privacidad/i))
      .toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Perfil de calidad por columna" })).toHaveTextContent(
      "95.0%",
    );
  });

  it("muestra rango y cobertura temporal sin exponer celdas", () => {
    render(
      <ReviewPhase
        datasetStatus={createReadyDatasetStatus(dataset)}
        profileStatus={{
          kind: "ready",
          profile: {
            ...inferredTemporalProfile,
            temporalSeries: inferredTemporalProfile.temporalSeries?.map((summary) => ({
              ...summary,
              truncated: true,
            })),
          },
        }}
        reviewTab="diagnosis"
        onTabChange={() => undefined}
        onPageChange={() => undefined}
        onAnalyzeQuality={() => undefined}
        onCancelProfile={() => undefined}
        comparisonStatus={{ kind: "idle" }}
        datasetColumns={dataset.columns}
        comparisonKeyColumns={[]}
        onComparisonKeyColumnsChange={() => undefined}
        onCompare={() => undefined}
        onClearComparison={() => undefined}
        onConsolidate={() => undefined}
        onResolveConflicts={() => undefined}
        onConflictPageChange={() => undefined}
        joinStatus={{ kind: "idle" }}
        joinType="inner"
        onJoinTypeChange={() => undefined}
        onJoin={() => undefined}
      />,
    );

    expect(screen.getByRole("heading", { name: "Cobertura temporal" })).toBeInTheDocument();
    const table = screen.getByRole("table", { name: "Tabla de cobertura temporal" });
    expect(table).toHaveTextContent("fecha");
    expect(table).toHaveTextContent("Fecha detectada");
    expect(table).toHaveTextContent("2024-01-15");
    expect(table).toHaveTextContent("2024-03-28");
    expect(table).toHaveTextContent("108 de 120");
    expect(table).toHaveTextContent("90.0%");
    expect(table).toHaveTextContent("actualizado_en");
    expect(table).toHaveTextContent("Fecha y hora");
    expect(table).toHaveTextContent("2024-12-31 18:30:00 UTC");
    expect(screen.getByRole("heading", { name: "Tendencia temporal · fecha" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Perfil de calidad por columna" })).toHaveTextContent(
      "Almacenado como Texto · sugerido: Fecha",
    );
    const trendTable = screen.getByRole("table", { name: "Tendencia temporal para fecha" });
    expect(trendTable).toHaveTextContent("2024-02");
    expect(trendTable).toHaveTextContent("36");
    expect(trendTable).toHaveTextContent("33.3%");
    const temporalTrend = screen.getByRole("group", { name: "Tendencia temporal · fecha" });
    expect(temporalTrend).toHaveTextContent("Se incluyen 108 de 120 filas interpretables.");
    expect(temporalTrend).toHaveTextContent("Los periodos más antiguos se agruparon");
    expect(screen.getByRole("img", { name: /Serie temporal de fecha por filas/ })).toBeInTheDocument();
    const metric = screen.getByRole("combobox", { name: "Métrica temporal para fecha" });
    expect(metric).toHaveValue("rows");
    fireEvent.change(metric, { target: { value: "percentage" } });
    expect(screen.getByRole("img", { name: /Serie temporal de fecha por porcentaje de valores/ })).toBeInTheDocument();
  });

  it("muestra una tendencia diaria acotada con tabla equivalente", () => {
    render(
      <ReviewPhase
        datasetStatus={createReadyDatasetStatus(dataset)}
        profileStatus={{ kind: "ready", profile: dailyTemporalProfile }}
        reviewTab="diagnosis"
        onTabChange={() => undefined}
        onPageChange={() => undefined}
        onAnalyzeQuality={() => undefined}
        onCancelProfile={() => undefined}
        comparisonStatus={{ kind: "idle" }}
        datasetColumns={dataset.columns}
        comparisonKeyColumns={[]}
        onComparisonKeyColumnsChange={() => undefined}
        onCompare={() => undefined}
        onClearComparison={() => undefined}
        onConsolidate={() => undefined}
        onResolveConflicts={() => undefined}
        onConflictPageChange={() => undefined}
        joinStatus={{ kind: "idle" }}
        joinType="inner"
        onJoinTypeChange={() => undefined}
        onJoin={() => undefined}
      />,
    );

    expect(screen.getByText(/Conteo de filas por día/)).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Calendario diario para fecha" })).toBeInTheDocument();
    const calendar = screen.getByRole("list", { name: "Calendario diario para fecha" });
    expect(calendar).toHaveTextContent("1 abr");
    expect(screen.getByRole("listitem", { name: /2024-04-02: 0 filas/ })).toBeInTheDocument();
    const trendTable = screen.getByRole("table", { name: "Tendencia temporal para fecha" });
    expect(trendTable).toHaveTextContent("2024-04-02");
    expect(trendTable).toHaveTextContent("2024-04-03");
    expect(trendTable).toHaveTextContent("50.0%");
  });

  it("expone un estado vacío cuando no hay días interpretables", () => {
    render(
      <ReviewPhase
        datasetStatus={createReadyDatasetStatus(dataset)}
        profileStatus={{ kind: "ready", profile: emptyDailyTemporalProfile }}
        reviewTab="diagnosis"
        onTabChange={() => undefined}
        onPageChange={() => undefined}
        onAnalyzeQuality={() => undefined}
        onCancelProfile={() => undefined}
        comparisonStatus={{ kind: "idle" }}
        datasetColumns={dataset.columns}
        comparisonKeyColumns={[]}
        onComparisonKeyColumnsChange={() => undefined}
        onCompare={() => undefined}
        onClearComparison={() => undefined}
        onConsolidate={() => undefined}
        onResolveConflicts={() => undefined}
        onConflictPageChange={() => undefined}
        joinStatus={{ kind: "idle" }}
        joinType="inner"
        onJoinTypeChange={() => undefined}
        onJoin={() => undefined}
      />,
    );

    expect(screen.getByRole("group", { name: "Calendario diario para fecha" })).toHaveTextContent(
      "No hay días interpretables para mostrar en esta columna.",
    );
    expect(screen.getByRole("table", { name: "Tendencia temporal para fecha" })).toBeInTheDocument();
  });

  it("calcula y presenta una agregación numérica elegida sobre el dataset completo", async () => {
    const getAggregation = vi
      .spyOn(bridge, "getTemporalAggregation")
      .mockResolvedValue(temporalMeanSeries);
    renderTemporalTrend();

    const metric = screen.getByRole("combobox", { name: "Métrica temporal para fecha" });
    fireEvent.change(metric, { target: { value: "numeric" } });
    const valueColumn = screen.getByRole("combobox", { name: "Columna numérica" });
    expect(valueColumn).toHaveValue("id");
    fireEvent.change(valueColumn, { target: { value: "ventas" } });
    const aggregation = screen.getByRole("combobox", { name: "Agregación" });
    expect(aggregation).toHaveValue("sum");
    fireEvent.change(aggregation, { target: { value: "mean" } });
    fireEvent.click(screen.getByRole("button", { name: "Calcular tendencia" }));

    await waitFor(() => expect(getAggregation).toHaveBeenCalledWith("fecha", "ventas", "mean"));
    const table = await screen.findByRole("table", {
      name: "Tendencia temporal para fecha: Promedio de ventas",
    });
    expect(table).toHaveTextContent("2024-01");
    expect(table).toHaveTextContent("25");
    expect(table).toHaveTextContent("2024-02");
    expect(table).toHaveTextContent("0");
    expect(table).toHaveTextContent("—");
    expect(table).toHaveTextContent("2024-03");
    expect(table).toHaveTextContent("40");
    expect(screen.getByRole("img", { name: /Tendencia temporal: Promedio de ventas por mes/ }))
      .toBeInTheDocument();
    expect(screen.getByText(/Nulos y valores no numéricos se excluyen del cálculo/))
      .toBeInTheDocument();
    expect(document.querySelectorAll(".quality-temporal-line__point")).toHaveLength(2);
    expect(document.querySelectorAll(".quality-temporal-line__path")).toHaveLength(2);
  });

  it("serializa agregaciones entre tendencias y solo permite cancelar desde la propietaria", async () => {
    let resolveAggregation!: (series: TemporalAggregationSeries) => void;
    const pendingAggregation = new Promise<TemporalAggregationSeries>((resolve) => {
      resolveAggregation = resolve;
    });
    let resolveCancellation!: () => void;
    const pendingCancellation = new Promise<void>((resolve) => {
      resolveCancellation = resolve;
    });
    const getAggregation = vi.spyOn(bridge, "getTemporalAggregation").mockReturnValue(pendingAggregation);
    const cancel = vi.spyOn(bridge, "cancelOperation").mockReturnValue(pendingCancellation);
    renderTemporalTrend(multipleTemporalProfile);

    const owner = screen.getByRole("group", { name: "Tendencia temporal · fecha" });
    const nonOwner = screen.getByRole("group", { name: "Tendencia temporal · fecha_entrega" });
    fireEvent.change(within(owner).getByRole("combobox", { name: "Métrica temporal para fecha" }), {
      target: { value: "numeric" },
    });
    fireEvent.change(within(nonOwner).getByRole("combobox", { name: "Métrica temporal para fecha_entrega" }), {
      target: { value: "numeric" },
    });
    fireEvent.change(within(owner).getByRole("combobox", { name: "Columna numérica" }), {
      target: { value: "ventas" },
    });
    fireEvent.change(within(owner).getByRole("combobox", { name: "Agregación" }), {
      target: { value: "mean" },
    });

    fireEvent.click(within(owner).getByRole("button", { name: "Calcular tendencia" }));
    expect(within(owner).getByRole("status")).toHaveTextContent(
      "Calculando una serie temporal con el dataset completo",
    );
    await waitFor(() => expect(getAggregation).toHaveBeenCalledTimes(1));

    const nonOwnerStatus = within(nonOwner).getByRole("status");
    expect(nonOwnerStatus).toHaveTextContent(/espera|en curso|activo/i);
    const nonOwnerCalculate = within(nonOwner).queryByRole("button", { name: "Calcular tendencia" });
    if (nonOwnerCalculate) expect(nonOwnerCalculate).toBeDisabled();
    expect(within(nonOwner).queryByRole("button", { name: "Cancelar cálculo" })).not.toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Cancelar cálculo" })).toHaveLength(1);

    if (nonOwnerCalculate) fireEvent.click(nonOwnerCalculate);
    expect(getAggregation).toHaveBeenCalledTimes(1);
    expect(getAggregation).toHaveBeenCalledWith("fecha", "ventas", "mean");
    expect(cancel).not.toHaveBeenCalled();

    fireEvent.click(within(owner).getByRole("button", { name: "Cancelar cálculo" }));
    await waitFor(() => expect(cancel).toHaveBeenCalledTimes(1));
    expect(cancel).toHaveBeenCalledWith("temporal");
    resolveAggregation(temporalMeanSeries);
    await waitFor(() => expect(within(owner).getByRole("table", {
      name: "Tendencia temporal para fecha: Promedio de ventas",
    })).toBeInTheDocument());
    expect(getAggregation).toHaveBeenCalledTimes(1);
    expect(within(nonOwner).queryByRole("button", { name: "Calcular tendencia" })).not.toBeInTheDocument();

    resolveCancellation();
    const nextCalculation = await within(nonOwner).findByRole("button", { name: "Calcular tendencia" });
    fireEvent.click(nextCalculation);
    await waitFor(() => expect(getAggregation).toHaveBeenCalledTimes(2));
  });

  it("descarta una agregación temporal tardía cuando cambia la revisión del dataset", async () => {
    let resolveAggregation!: (series: TemporalAggregationSeries) => void;
    const pendingAggregation = new Promise<TemporalAggregationSeries>((resolve) => {
      resolveAggregation = resolve;
    });
    vi.spyOn(bridge, "getTemporalAggregation").mockReturnValue(pendingAggregation);
    const cancel = vi.spyOn(bridge, "cancelOperation").mockResolvedValue(undefined);
    const view = renderTemporalTrend(numericTemporalProfile, 0);

    fireEvent.change(screen.getByRole("combobox", { name: "Métrica temporal para fecha" }), {
      target: { value: "numeric" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Calcular tendencia" }));
    await waitFor(() => expect(bridge.getTemporalAggregation).toHaveBeenCalledOnce());

    view.rerender(temporalTrendElement(numericTemporalProfile, 1));
    await waitFor(() => expect(cancel).toHaveBeenCalledWith("temporal"));
    expect(screen.getByRole("status")).toHaveTextContent(
      "Elige una métrica y calcula la tendencia sobre todas las filas",
    );

    resolveAggregation(temporalMeanSeries);
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Calcular tendencia" })).toBeEnabled();
    });
    expect(screen.queryByRole("table", { name: /Promedio de ventas/ })).not.toBeInTheDocument();
  });

  it("expone progreso y permite cancelar el cálculo de la serie numérica", async () => {
    let rejectAggregation!: (error: Error) => void;
    const pendingAggregation = new Promise<TemporalAggregationSeries>((_resolve, reject) => {
      rejectAggregation = reject;
    });
    vi.spyOn(bridge, "getTemporalAggregation").mockReturnValue(pendingAggregation);
    const cancel = vi.spyOn(bridge, "cancelOperation").mockImplementation(async () => {
      rejectAggregation(new Error("Operación temporal cancelada"));
    });
    renderTemporalTrend();

    fireEvent.change(screen.getByRole("combobox", { name: "Métrica temporal para fecha" }), {
      target: { value: "numeric" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Calcular tendencia" }));
    expect(screen.getByRole("status")).toHaveTextContent(
      "Calculando una serie temporal con el dataset completo",
    );

    fireEvent.click(screen.getByRole("button", { name: "Cancelar cálculo" }));
    await waitFor(() => expect(cancel).toHaveBeenCalledWith("temporal"));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(
      "Elige una métrica y calcula la tendencia sobre todas las filas",
    ));
    expect(screen.queryByRole("table", { name: /Promedio de ventas/ })).not.toBeInTheDocument();
  });

  it("muestra el error cuando falla la agregación numérica", async () => {
    vi.spyOn(bridge, "getTemporalAggregation").mockRejectedValue(new Error("falló el cálculo local"));
    renderTemporalTrend();

    fireEvent.change(screen.getByRole("combobox", { name: "Métrica temporal para fecha" }), {
      target: { value: "numeric" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Calcular tendencia" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "No se pudo calcular la tendencia numérica: falló el cálculo local",
    );
  });

  it("exige y emite una decisión explícita por conflicto", () => {
    const onResolveConflicts = vi.fn();
    render(
      <ReviewPhase
        datasetStatus={createReadyDatasetStatus(dataset)}
        profileStatus={{ kind: "idle" }}
        reviewTab="diagnosis"
        onTabChange={() => undefined}
        onPageChange={() => undefined}
        onAnalyzeQuality={() => undefined}
        onCancelProfile={() => undefined}
        comparisonStatus={{
          kind: "ready",
          comparison: {
            currentFileName: "datos.csv",
            comparedFileName: "actualizacion.csv",
            currentRowCount: 2,
            comparedRowCount: 2,
            commonRowCount: 2,
            currentOnlyRowCount: 0,
            comparedOnlyRowCount: 0,
            sharedColumns: ["id", "nota"],
            currentOnlyColumns: [],
            comparedOnlyColumns: [],
            schemaCompatible: true,
            keyColumns: ["id"],
            matchedKeyCount: 1,
            currentOnlyKeyCount: 0,
            comparedOnlyKeyCount: 0,
            conflictingKeyCount: 1,
            duplicateKeyCount: 0,
            conflicts: [{
              key: ["51"],
              cells: [{ column: "nota", current: null, compared: "ok" }],
            }],
            conflictOffset: 0,
            conflictsTruncated: false,
            canConsolidate: false,
          },
        }}
        datasetColumns={dataset.columns}
        comparisonKeyColumns={["id"]}
        onComparisonKeyColumnsChange={() => undefined}
        onCompare={() => undefined}
        onClearComparison={() => undefined}
        onConsolidate={() => undefined}
        onResolveConflicts={onResolveConflicts}
        onConflictPageChange={() => undefined}
        joinStatus={{ kind: "idle" }}
        joinType="inner"
        onJoinTypeChange={() => undefined}
        onJoin={() => undefined}
      />,
    );

    const resolveButton = screen.getByRole("button", { name: "Resolver conflictos" });
    expect(resolveButton).toBeDisabled();
    fireEvent.click(screen.getByRole("radio", { name: "Usar comparado en nota" }));
    expect(resolveButton).toBeEnabled();
    fireEvent.click(resolveButton);
    expect(onResolveConflicts).toHaveBeenCalledWith([
      { conflictIndex: 0, column: "nota", source: "compared" },
    ]);
  });

  it("pagina conflictos sin permitir saltar decisiones pendientes", () => {
    const onConflictPageChange = vi.fn();
    render(
      <ReviewPhase
        datasetStatus={createReadyDatasetStatus(dataset)}
        profileStatus={{ kind: "idle" }}
        reviewTab="diagnosis"
        onTabChange={() => undefined}
        onPageChange={() => undefined}
        onAnalyzeQuality={() => undefined}
        onCancelProfile={() => undefined}
        comparisonStatus={{
          kind: "ready",
          comparison: {
            currentFileName: "datos.csv",
            comparedFileName: "actualizacion.csv",
            currentRowCount: 2,
            comparedRowCount: 2,
            commonRowCount: 2,
            currentOnlyRowCount: 0,
            comparedOnlyRowCount: 0,
            sharedColumns: ["id", "nota"],
            currentOnlyColumns: [],
            comparedOnlyColumns: [],
            schemaCompatible: true,
            keyColumns: ["id"],
            matchedKeyCount: 1,
            currentOnlyKeyCount: 0,
            comparedOnlyKeyCount: 0,
            conflictingKeyCount: 2,
            duplicateKeyCount: 0,
            conflicts: [{
              key: ["51"],
              cells: [{ column: "nota", current: null, compared: "ok" }],
            }],
            conflictOffset: 0,
            conflictsTruncated: true,
            canConsolidate: false,
          },
        }}
        datasetColumns={dataset.columns}
        comparisonKeyColumns={["id"]}
        onComparisonKeyColumnsChange={() => undefined}
        onCompare={() => undefined}
        onClearComparison={() => undefined}
        onConsolidate={() => undefined}
        onResolveConflicts={() => undefined}
        onConflictPageChange={onConflictPageChange}
        joinStatus={{ kind: "idle" }}
        joinType="inner"
        onJoinTypeChange={() => undefined}
        onJoin={() => undefined}
      />,
    );

    const nextButton = screen.getByRole("button", { name: "Siguientes conflictos" });
    expect(nextButton).toBeDisabled();
    fireEvent.click(screen.getByRole("radio", { name: "Usar comparado en nota" }));
    expect(nextButton).toBeEnabled();
    fireEvent.click(nextButton);
    expect(onConflictPageChange).toHaveBeenCalledWith(1);
  });

  it("anuncia el rango, representa null y solicita saltos exactos de 50", () => {
    const onPageChange = vi.fn();
    render(
      <DatasetPreviewPanel
        dataset={dataset}
        pageOffset={50}
        pageLoading={false}
        onPageChange={onPageChange}
      />,
    );

    expect(screen.getByLabelText("Vista previa del dataset")).toHaveAttribute("tabindex", "0");
    expect(screen.getByText("Filas 51–52 de 120")).toHaveAttribute("aria-live", "polite");
    expect(screen.getByText("null")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Anterior" }));
    fireEvent.click(screen.getByRole("button", { name: "Siguiente" }));
    expect(onPageChange).toHaveBeenNthCalledWith(1, 0);
    expect(onPageChange).toHaveBeenNthCalledWith(2, 100);
  });
});
