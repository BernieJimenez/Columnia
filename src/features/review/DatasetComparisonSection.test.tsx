import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { DatasetComparison } from "../../bridge";
import type { ComparisonStatus } from "./compareModel";
import { DatasetComparisonSection } from "./DatasetComparisonSection";

afterEach(cleanup);

const comparison: DatasetComparison = {
  currentFileName: "ventas.csv",
  comparedFileName: "ventas-2.csv",
  currentRowCount: 3,
  comparedRowCount: 3,
  commonRowCount: 2,
  currentOnlyRowCount: 1,
  comparedOnlyRowCount: 1,
  sharedColumns: ["id", "importe"],
  currentOnlyColumns: [],
  comparedOnlyColumns: [],
  schemaCompatible: true,
  keyColumns: ["id"],
  matchedKeyCount: 3,
  currentOnlyKeyCount: 0,
  comparedOnlyKeyCount: 0,
  conflictingKeyCount: 0,
  duplicateKeyCount: 0,
  conflicts: [],
  conflictOffset: 0,
  conflictsTruncated: false,
  canConsolidate: true,
};

function renderSection(status: ComparisonStatus, onKeyColumnsChange = vi.fn()) {
  render(
    <DatasetComparisonSection
      status={status}
      datasetColumns={[
        { name: "id", dataType: "Int64" },
        { name: "importe", dataType: "String" },
      ]}
      datasetRevision={1}
      keyColumns={["id"]}
      onKeyColumnsChange={onKeyColumnsChange}
      onCompare={vi.fn()}
      onCancelComparison={vi.fn()}
      comparisonCancellationPending={false}
      onClear={vi.fn()}
      onConsolidate={vi.fn()}
      onResolveConflicts={vi.fn()}
      onConflictPageChange={vi.fn()}
      joinStatus={{ kind: "idle" }}
      reviewMutationStatus={{ kind: "idle" }}
      reviewMutationCancellationPending={false}
      joinType="inner"
      onJoinTypeChange={vi.fn()}
      onJoin={vi.fn()}
      onCancelReviewMutation={vi.fn()}
    />,
  );
  return onKeyColumnsChange;
}

describe("DatasetComparisonSection", () => {
  it("no deja cambiar la clave bajo un resultado visible y explica cómo hacerlo (UX-05)", () => {
    renderSection({ kind: "ready", comparison });
    expect(screen.getByRole("checkbox", { name: /importe/ })).toBeDisabled();
    expect(screen.getByRole("checkbox", { name: /id/ })).toBeDisabled();
    expect(screen.getByText(/Para cambiar la clave, descarta antes la comparación/)).toBeInTheDocument();
  });

  it("permite elegir la clave antes de comparar", () => {
    const onKeyColumnsChange = renderSection({ kind: "idle" });
    fireEvent.click(screen.getByRole("checkbox", { name: /importe/ }));
    expect(onKeyColumnsChange).toHaveBeenCalledWith(["id", "importe"]);
  });

  it("muestra la guía del motor cuando los tipos de la clave no coinciden (UX-06)", () => {
    renderSection({
      kind: "error",
      message:
        "La columna clave 'id' es un número entero en el dataset activo y texto en el comparado. Conviértela al mismo tipo en Preparar (por ejemplo, a texto) y vuelve a comparar.",
    });
    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent("No se pudo comparar la fuente");
    expect(alert).toHaveTextContent("Conviértela al mismo tipo en Preparar");
  });
});
