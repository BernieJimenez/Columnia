import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { DatasetPreview, DatasetProfile } from "../../bridge";
import { createReadyDatasetStatus } from "../load/loadModel";
import { ReviewPhase } from "./ReviewPhase";
import { byImpact } from "./proposalImpact";
import { buildPrepareProposal, defaultProposalSelection, proposalItemTitle } from "../prepare/proposalModel";

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
    dateOrder: null,
    dateHasTime: null,
    untrimmedCount: null,
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

    // Revisar announces exactly what Preparar will propose for this profile.
    const proposal = buildPrepareProposal(profile, dataset);
    const selection = defaultProposalSelection(proposal);
    expect(proposal.length).toBeGreaterThan(0);
    expect(screen.getByRole("heading", {
      name: `${proposal.length} ${proposal.length === 1 ? "cosa se puede corregir" : "cosas se pueden corregir"} con la propuesta`,
    })).toBeInTheDocument();
    const issues = screen.getByLabelText("Resumen de calidad del dataset");
    // Most serious first; every item names what it does to an analysis.
    const items = within(issues).getAllByRole("listitem");
    expect(items.map((item) => item.querySelector(".quality-issue__title")?.textContent)).toEqual(
      byImpact(proposal).map((item) => proposalItemTitle(item, selection)),
    );
    for (const item of items) expect(item.querySelector(".quality-issue__impact")?.textContent).toBeTruthy();
    expect(within(issues).queryByText(/duplicad/)).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Resumen del dataset")).not.toBeInTheDocument();
    expect(screen.getByText("Más análisis y herramientas").closest("details")).not.toHaveAttribute("open");
    expect(screen.getByRole("heading", { name: "Configuración del análisis" })).toBeInTheDocument();

    // One list on top; what the proposal does not fix goes below, with its own actions.
    expect(screen.queryByRole("list", { name: "Prioridades de revisión" })).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Vista rápida de calidad")).not.toBeInTheDocument();
    const uncorrected = screen.getByRole("list", { name: "Lo que no se corrige solo" });
    expect(uncorrected).toHaveTextContent("1 nulo en 1 columna");
    expect(uncorrected).toHaveTextContent("1 valor incompatible con el tipo sugerido en 1 columna");
    // The links to Preparar are tertiary (text), so the one primary stays «Ver cambios propuestos».
    expect(screen.getByRole("button", { name: "Ver nulos en Preparar" })).toHaveClass("quality-uncorrected__action");
    expect(screen.getByRole("button", { name: "Ver nulos en Preparar" })).not.toHaveClass("secondary-action");
    fireEvent.click(screen.getByRole("button", { name: "Ver nulos en Preparar" }));
    expect(onContinueToPrepare).toHaveBeenLastCalledWith("missingValues");
    fireEvent.click(screen.getByRole("button", { name: "Ver cambios propuestos" }));
    // Without a signal focus, Preparar opens on the proposal, not the advanced tools.
    expect(onContinueToPrepare).toHaveBeenCalledWith();
  });
});
