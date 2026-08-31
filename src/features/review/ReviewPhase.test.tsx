import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { DatasetPreview, DatasetProfile, DatasetQueryResult } from "../../bridge";
import * as bridge from "../../bridge";
import { DataPreview, ReviewPhase } from "./ReviewPhase";
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

describe("ReviewPhase", () => {
  it("conserva tabpanel ARIA y perfil bajo demanda", () => {
    const onAnalyzeQuality = vi.fn();
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
      />,
    );

    expect(screen.getByRole("tabpanel", { name: "Diagnóstico" })).toHaveAttribute(
      "aria-labelledby",
      "review-diagnosis-tab",
    );
    fireEvent.click(screen.getByRole("button", { name: "Analizar calidad" }));
    expect(onAnalyzeQuality).toHaveBeenCalledOnce();
  });

  it("expone la cobertura agregada de una sesión DataPrep sin prometer la muestra original", () => {
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
        importedSessionAnalysis={{
          analysisSampled: true,
          analysisSampleRowCount: 120,
          analysisTotalRowCount: 1_000,
        }}
      />,
    );

    const notice = screen.getByRole("status", { name: "Cobertura del análisis importado" });
    expect(notice).toHaveTextContent("Análisis importado desde DataPrep");
    expect(notice).toHaveTextContent("cobertura muestreado · 120 de 1,000 filas");
    expect(notice).toHaveTextContent("no restaura filas, valores ni resultados originales");
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

  it("muestra visualizaciones accesibles con valores equivalentes al perfil", () => {
    render(
      <ReviewPhase
        datasetStatus={createReadyDatasetStatus(dataset)}
        profileStatus={{ kind: "ready", profile }}
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
    expect(
      screen.getByRole("region", { name: "Matriz de correlaciones numéricas" }),
    ).toHaveTextContent("-0.42");
    expect(screen.getByRole("region", { name: "Perfil de calidad por columna" })).toHaveTextContent(
      "95.0%",
    );
  });

  it("muestra rango y cobertura temporal sin exponer celdas", () => {
    render(
      <ReviewPhase
        datasetStatus={createReadyDatasetStatus(dataset)}
        profileStatus={{ kind: "ready", profile: temporalProfile }}
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
    expect(table).toHaveTextContent("Fecha");
    expect(table).toHaveTextContent("2024-01-01");
    expect(table).toHaveTextContent("2024-12-31");
    expect(table).toHaveTextContent("108 de 120");
    expect(table).toHaveTextContent("90.0%");
    expect(table).toHaveTextContent("actualizado_en");
    expect(table).toHaveTextContent("Fecha y hora");
    expect(table).toHaveTextContent("2024-12-31 18:30:00 UTC");
    expect(screen.getByRole("heading", { name: "Tendencia temporal · fecha" })).toBeInTheDocument();
    const trendTable = screen.getByRole("table", { name: "Tendencia temporal para fecha" });
    expect(trendTable).toHaveTextContent("2024-02");
    expect(trendTable).toHaveTextContent("36");
    expect(trendTable).toHaveTextContent("33.3%");
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
      <DataPreview
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
