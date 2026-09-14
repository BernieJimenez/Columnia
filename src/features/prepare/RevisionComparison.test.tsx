import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as bridge from "../../bridge";
import type { HistoryState, SnapshotRevisionComparison } from "../../bridge";
import { RevisionComparison } from "./RevisionComparison";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function history(overrides: Partial<HistoryState> = {}): HistoryState {
  return {
    canUndo: true,
    canRedo: false,
    currentIndex: 1,
    entryCount: 2,
    entries: [
      { id: "rev-before-0001", index: 0, label: "Dataset original", isCurrent: false },
      { id: "rev-after-0002", index: 1, label: "Eliminar nulos", isCurrent: true },
    ],
    snapshotsEnabled: true,
    degradedReason: null,
    maxEntries: 12,
    diskBytes: 200,
    diskBudgetBytes: 1024,
    ...overrides,
  };
}

const result: SnapshotRevisionComparison = {
  beforeSnapshotId: "rev-before-0001",
  afterSnapshotId: "rev-after-0002",
  beforeLabel: "Dataset original",
  afterLabel: "Eliminar nulos",
  before: { rowCount: 4, columnCount: 2, nullCount: 2, invalidTypeCount: 1, duplicateRowCount: 0 },
  after: { rowCount: 3, columnCount: 2, nullCount: 0, invalidTypeCount: 0, duplicateRowCount: 0 },
  deltas: { rowCount: -1, columnCount: 0, nullCount: -2, invalidTypeCount: -1, duplicateRowCount: 0 },
  columns: [
    {
      name: "monto",
      comparable: true,
      reason: null,
      before: { dataType: "String", nullCount: 2, invalidTypeCount: 1 },
      after: { dataType: "Float64", nullCount: 0, invalidTypeCount: 0 },
      typeChanged: true,
      nullCountDelta: -2,
      invalidTypeCountDelta: -1,
    },
  ],
  quality: {
    configuredRuleCount: 1,
    comparableRuleCount: 1,
    nonComparableRuleCount: 0,
    improvedRuleCount: 1,
    degradedRuleCount: 0,
    beforePassedRuleCount: 0,
    afterPassedRuleCount: 1,
    rules: [{
      ruleIndex: 1,
      kind: "numeric_range",
      column: "monto",
      comparable: true,
      reason: null,
      beforeInvalidCount: 2,
      afterInvalidCount: 0,
      beforeInvalidPercentage: 50,
      afterInvalidPercentage: 0,
      beforePassed: false,
      afterPassed: true,
    }],
  },
};

describe("RevisionComparison", () => {
  it("compara IDs explícitos, muestra solo métricas agregadas y aplica las mismas reglas", async () => {
    const compare = vi.spyOn(bridge, "compareHistorySnapshots").mockResolvedValue(result);
    const rules = [{ column: "monto", kind: "numeric_range" as const, min: 0, maxInvalid: 0 }];
    render(<RevisionComparison historyStatus={history()} qualityRules={rules} datasetRevision={4} busy={false} />);

    expect(screen.getByLabelText("Antes")).toHaveValue("rev-before-0001");
    expect(screen.getByLabelText("Después")).toHaveValue("rev-after-0002");
    fireEvent.click(screen.getByRole("button", { name: "Comparar agregados" }));

    await screen.findByLabelText("Resultado agregado de revisiones");
    expect(compare).toHaveBeenCalledWith(
      "rev-before-0001",
      "rev-after-0002",
      rules,
      expect.any(Function),
    );
    expect(screen.getByText("Filas", { exact: true }).parentElement).toHaveTextContent("4 → 3 (-1)");
    fireEvent.click(screen.getByText(/Tipos y nulos por columna/));
    expect(screen.getByText(/String → Float64/)).toBeInTheDocument();
    fireEvent.click(screen.getByText(/Reglas de calidad/));
    expect(screen.getByText(/Regla 1 · numeric_range/)).toBeInTheDocument();
  });

  it("explica el historial desactivado o degradado y no ofrece comparar", () => {
    render(<RevisionComparison
      historyStatus={history({
        snapshotsEnabled: false,
        degradedReason: "El historial se degradó por falta de espacio.",
        entries: [{ id: null, index: 0, label: "Actual", isCurrent: true }],
      })}
      qualityRules={[]}
      datasetRevision={1}
      busy={false}
    />);
    expect(screen.getByRole("note")).toHaveTextContent("El historial se degradó por falta de espacio.");
    expect(screen.queryByRole("button", { name: "Comparar agregados" })).not.toBeInTheDocument();
  });

  it("bloquea seleccionar la misma revisión", () => {
    render(<RevisionComparison historyStatus={history()} qualityRules={[]} datasetRevision={1} busy={false} />);
    fireEvent.change(screen.getByLabelText("Después"), { target: { value: "rev-before-0001" } });
    expect(screen.getByRole("button", { name: "Comparar agregados" })).toBeDisabled();
  });

  it("oculta el resultado anterior al cambiar una de las revisiones seleccionadas", async () => {
    vi.spyOn(bridge, "compareHistorySnapshots").mockResolvedValue(result);
    render(<RevisionComparison historyStatus={history()} qualityRules={[]} datasetRevision={1} busy={false} />);
    fireEvent.click(screen.getByRole("button", { name: "Comparar agregados" }));
    await screen.findByLabelText("Resultado agregado de revisiones");

    fireEvent.change(screen.getByLabelText("Antes"), { target: { value: "rev-after-0002" } });
    expect(screen.queryByLabelText("Resultado agregado de revisiones")).not.toBeInTheDocument();
  });

  it("descarta una respuesta que llega después de cambiar el dataset", async () => {
    let resolveComparison!: (comparison: SnapshotRevisionComparison) => void;
    const compare = vi.spyOn(bridge, "compareHistorySnapshots").mockReturnValue(
      new Promise((resolve) => { resolveComparison = resolve; }),
    );
    const props = { historyStatus: history(), qualityRules: [], datasetRevision: 2, busy: false };
    const view = render(<RevisionComparison {...props} />);
    fireEvent.click(screen.getByRole("button", { name: "Comparar agregados" }));
    expect(compare).toHaveBeenCalledOnce();

    view.rerender(<RevisionComparison {...props} datasetRevision={3} />);
    resolveComparison(result);
    await waitFor(() => expect(screen.queryByLabelText("Resultado agregado de revisiones")).not.toBeInTheDocument());
  });

  it("vuelve a invalidar el resultado cuando desaparece un ID seleccionado", async () => {
    const compare = vi.spyOn(bridge, "compareHistorySnapshots").mockResolvedValue(result);
    const view = render(<RevisionComparison historyStatus={history()} qualityRules={[]} datasetRevision={4} busy={false} />);
    fireEvent.click(screen.getByRole("button", { name: "Comparar agregados" }));
    await screen.findByLabelText("Resultado agregado de revisiones");

    view.rerender(<RevisionComparison
      historyStatus={history({
        entryCount: 1,
        currentIndex: 0,
        entries: [{ id: "rev-new-0003", index: 0, label: "Nueva rama", isCurrent: true }],
      })}
      qualityRules={[]}
      datasetRevision={5}
      busy={false}
    />);
    await waitFor(() => expect(screen.queryByLabelText("Resultado agregado de revisiones")).not.toBeInTheDocument());
    expect(compare).toHaveBeenCalledOnce();
  });
});
