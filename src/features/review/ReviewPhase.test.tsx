import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { DatasetPreview } from "../../bridge";
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
      />,
    );

    expect(screen.getByRole("tabpanel", { name: "Diagnóstico" })).toHaveAttribute(
      "aria-labelledby",
      "review-diagnosis-tab",
    );
    fireEvent.click(screen.getByRole("button", { name: "Analizar calidad" }));
    expect(onAnalyzeQuality).toHaveBeenCalledOnce();
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
