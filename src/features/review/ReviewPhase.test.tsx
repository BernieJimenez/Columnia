import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { DatasetPreview, DatasetProfile } from "../../bridge";
import * as bridge from "../../bridge";
import { DataPreview, ReviewPhase } from "./ReviewPhase";
import { createReadyDatasetStatus } from "../load/loadModel";

afterEach(cleanup);

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
      typeMatchPercentage: null,
      invalidTypeCount: null,
      sentinelCount: null,
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
      />,
    );

    fireEvent.change(screen.getByRole("textbox", { name: "Consulta SQL de solo lectura" }), {
      target: { value: "SELECT id FROM dataset LIMIT 1" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Ejecutar consulta" }));

    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("2 filas disponibles"));
    expect(screen.getByRole("region", { name: "Resultado de consulta SQL" })).toHaveTextContent("id");
    expect(bridge.queryDataset).toHaveBeenCalledWith("SELECT id FROM dataset LIMIT 1");
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
    expect(screen.getByRole("heading", { name: "Distribución numérica" })).toBeInTheDocument();
    expect(screen.getByRole("list", { name: "Distribución numérica por columna" })).toHaveTextContent(
      "Q1 30 · Mediana 60 · Q3 90",
    );
    expect(screen.getByRole("heading", { name: "Histograma numérico" })).toBeInTheDocument();
    expect(screen.getByRole("table", { name: "Tabla de frecuencias para id" })).toHaveTextContent(
      "20",
    );
    expect(screen.getByRole("region", { name: "Perfil de calidad por columna" })).toHaveTextContent(
      "95.0%",
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
