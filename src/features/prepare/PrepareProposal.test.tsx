import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { SafeCorrectionOptions, SafeCorrectionsPreview } from "../../bridge";
import { PrepareProposal } from "./PrepareProposal";
import type { ProposalItem } from "./proposalModel";

const items: ProposalItem[] = [
  { id: "duplicates", title: "Quitar 1 fila duplicada", hint: "Se conserva la primera.", examples: [] },
  {
    id: "impute",
    title: "Rellenar 3 valores vacíos en 1 columna",
    hint: "Solo huecos pequeños.",
    columns: [{ name: "categoria", missing: 3, sentinels: 0 }],
    examples: [{ column: "categoria", before: null, after: "valor más frecuente · 3 celdas" }],
  },
];

function renderProposal(onPreview: (options: SafeCorrectionOptions) => Promise<SafeCorrectionsPreview>) {
  const onApply = vi.fn();
  render(
    <PrepareProposal
      items={items}
      columnCount={2}
      busy={false}
      canUndo={false}
      result={null}
      onApply={onApply}
      onPreview={onPreview}
      onUndo={vi.fn()}
      onDismissResult={vi.fn()}
    />,
  );
  return onApply;
}

afterEach(cleanup);

describe("PrepareProposal fill preview (RV17)", () => {
  it("announces the exact value and cells the engine will fill after the whole chain", async () => {
    let resolve!: (preview: SafeCorrectionsPreview) => void;
    const onPreview = vi.fn(() => new Promise<SafeCorrectionsPreview>((done) => { resolve = done; }));
    const onApply = renderProposal(onPreview);

    expect(onPreview).not.toHaveBeenCalled();
    fireEvent.click(screen.getByLabelText("Rellenar 3 valores vacíos en 1 columna"));

    expect(onPreview).toHaveBeenCalledWith(expect.objectContaining({
      removeDuplicates: true,
      imputeMissing: true,
      imputeColumns: ["categoria"],
    }));
    expect(screen.getByLabelText("Rellenar valores vacíos: calculando cuántos…")).toBeChecked();
    const apply = screen.getByRole("button", { name: "Aplicar 2 cambios" });
    expect(apply).toBeDisabled();

    // The duplicate row held one of the gaps.
    resolve({ removedRowCount: 1, imputedCellCount: 2, imputations: [{ column: "categoria", value: "A", cellCount: 2 }] });
    expect(await screen.findByLabelText("Rellenar 2 valores vacíos en 1 columna")).toBeChecked();
    expect(apply).toBeEnabled();

    fireEvent.click(screen.getByRole("button", { name: "Ver antes y después" }));
    expect(screen.getByText("A · 2 celdas")).toBeInTheDocument();

    fireEvent.click(apply);
    expect(onApply).toHaveBeenCalledWith(expect.objectContaining({ imputeColumns: ["categoria"] }));
  });

  it("recomputes when another change of the chain is unchecked", async () => {
    const onPreview = vi.fn((options: SafeCorrectionOptions) => Promise.resolve({
      removedRowCount: options.removeDuplicates ? 1 : 0,
      imputedCellCount: options.removeDuplicates ? 2 : 3,
      imputations: [{ column: "categoria", value: "A", cellCount: options.removeDuplicates ? 2 : 3 }],
    }));
    renderProposal(onPreview);

    fireEvent.click(screen.getByLabelText("Rellenar 3 valores vacíos en 1 columna"));
    await screen.findByLabelText("Rellenar 2 valores vacíos en 1 columna");
    fireEvent.click(screen.getByLabelText("Quitar 1 fila duplicada"));

    expect(await screen.findByLabelText("Rellenar 3 valores vacíos en 1 columna")).toBeChecked();
    expect(onPreview).toHaveBeenLastCalledWith(expect.objectContaining({ removeDuplicates: false }));
  });

  it("falls back to the profile estimate when the simulation fails", async () => {
    renderProposal(vi.fn(() => Promise.reject(new Error("sin memoria"))));

    fireEvent.click(screen.getByLabelText("Rellenar 3 valores vacíos en 1 columna"));

    expect(await screen.findByLabelText("Rellenar 3 valores vacíos en 1 columna")).toBeChecked();
    expect(screen.getByRole("button", { name: "Aplicar 2 cambios" })).toBeEnabled();
  });
});

describe("PrepareProposal ambiguous dates (RV18)", () => {
  const dateItems: ProposalItem[] = [
    {
      id: "dates",
      title: "Convertir 1 columna a fecha",
      hint: "Todos sus valores son fechas.",
      dateColumns: [{ name: "alta", order: null, sample: "01/02/2024" }],
      examples: [],
    },
  ];

  it("asks once how to read the dates and sends the answer", () => {
    const onApply = vi.fn();
    render(
      <PrepareProposal items={dateItems} columnCount={1} busy={false} canUndo={false} result={null}
        onApply={onApply} onUndo={vi.fn()} onDismissResult={vi.fn()} />,
    );

    const apply = screen.getByRole("button", { name: "Aplicar 0 cambios" });
    expect(apply).toBeDisabled();
    const question = screen.getByRole("group", { name: "¿Cómo se lee «01/02/2024»?" });
    fireEvent.click(within(question).getByLabelText("Día/mes: 2024-02-01"));

    expect(screen.getByLabelText("Convertir 1 columna a fecha")).toBeChecked();
    fireEvent.click(screen.getByRole("button", { name: "Ver antes y después" }));
    expect(screen.getByText("2024-02-01", { selector: "strong" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Aplicar 1 cambio" }));
    expect(onApply).toHaveBeenCalledWith(expect.objectContaining({
      dateColumns: [{ column: "alta", order: "dmy" }],
    }));
  });
});
