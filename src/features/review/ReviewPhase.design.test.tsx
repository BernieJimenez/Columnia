import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { DatasetPreview, DatasetProfile } from "../../bridge";
import { createReadyDatasetStatus } from "../load/loadModel";
import { ReviewPhase } from "./ReviewPhase";

const dataset: DatasetPreview = {
  fileName: "calidad.csv",
  fileSizeBytes: 64,
  rowCount: 4,
  columnCount: 1,
  columns: [{ name: "monto", dataType: "String" }],
  rows: [["10"], [null], ["30"], ["40"]],
};

const profile: DatasetProfile = {
  rowCount: 4,
  duplicateRowCount: 0,
  nearDuplicateRowCount: 0,
  duplicatePercentage: 0,
  columns: [{
    name: "monto",
    dataType: "String",
    nullCount: 1,
    completenessPercentage: 75,
    uniqueCount: 3,
    minimum: "10",
    maximum: "40",
    mean: null,
    emptyCount: 0,
    minimumLength: 2,
    maximumLength: 2,
    averageLength: 2,
    suggestedType: "integer",
    typeMatchPercentage: 75,
    invalidTypeCount: 1,
    sentinelCount: 0,
    encodingIssueCount: 0,
    privacySignal: null,
    standardDeviation: null,
    firstQuartile: null,
    median: null,
    thirdQuartile: null,
    outlierCount: null,
    histogram: null,
  }],
};

describe("ReviewPhase design", () => {
  it("prioriza el resumen y permite continuar a Preparar sin recorrer el detalle", () => {
    const onContinueToPrepare = vi.fn();

    render(
      <ReviewPhase
        datasetStatus={createReadyDatasetStatus(dataset)}
        profileStatus={{ kind: "ready", profile }}
        reviewTab="diagnosis"
        onTabChange={() => undefined}
        onPageChange={() => undefined}
        onCancelProfile={() => undefined}
        onContinueToPrepare={onContinueToPrepare}
        comparison={{
          status: { kind: "idle" },
          keyColumns: [],
          onKeyColumnsChange: () => undefined,
          onCompare: () => undefined,
          onClear: () => undefined,
          onConsolidate: () => undefined,
          onResolveConflicts: () => undefined,
          onConflictPageChange: () => undefined,
          joinStatus: { kind: "idle" },
          joinType: "inner",
          onJoinTypeChange: () => undefined,
          onJoin: () => undefined,
        }}
      />,
    );

    expect(screen.getByRole("heading", { name: "Encontramos 2 cosas para arreglar" })).toBeInTheDocument();
    const issues = screen.getByLabelText("Resumen de calidad del dataset");
    expect(within(issues).getByText("Valores vacíos en 1 columna").parentElement).toHaveTextContent("1");
    expect(within(issues).getByText("Valores con tipo incompatible")).toBeInTheDocument();
    expect(within(issues).queryByText("Filas duplicadas")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Resumen del dataset")).not.toBeInTheDocument();
    expect(screen.getByText("Explorar análisis detallado").closest("details")).not.toHaveAttribute("open");
    expect(screen.getByText("Configuración del análisis").closest("details")).not.toHaveAttribute("open");

    expect(screen.getByRole("list", { name: "Prioridades de revisión" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Ver cambios propuestos" }));
    // Without a signal focus, Preparar opens on the proposal, not the advanced tools.
    expect(onContinueToPrepare).toHaveBeenCalledWith();
  });
});
