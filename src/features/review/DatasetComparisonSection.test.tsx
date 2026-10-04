import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as bridge from "../../bridge";
import type { DatasetComparison } from "../../bridge";
import type { ComparisonStatus } from "./compareModel";
import { DatasetComparisonSection } from "./DatasetComparisonSection";
import type { JoinStatus, ReviewMutationStatus } from "./joinModel";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

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

const withConflicts = {
  ...comparison,
  conflictingKeyCount: 60,
  canConsolidate: false,
  conflicts: [
    { key: ["7"], cells: [{ column: "importe", current: "10", compared: "12" }] },
  ],
  conflictOffset: 50,
  conflictsTruncated: false,
} as unknown as DatasetComparison;

type Overrides = Partial<{
  status: ComparisonStatus;
  joinStatus: JoinStatus;
  reviewMutationStatus: ReviewMutationStatus;
  reviewMutationCancellationPending: boolean;
  comparisonCancellationPending: boolean;
  onConflictPageChange: (offset: number) => void | Promise<void>;
}>;

function renderSection(overrides: Overrides = {}) {
  const props = {
    status: { kind: "ready", comparison } as ComparisonStatus,
    joinStatus: { kind: "idle" } as JoinStatus,
    reviewMutationStatus: { kind: "idle" } as ReviewMutationStatus,
    reviewMutationCancellationPending: false,
    comparisonCancellationPending: false,
    onConflictPageChange: vi.fn() as (offset: number) => void | Promise<void>,
    onResolveConflicts: vi.fn(),
    onCancelReviewMutation: vi.fn(),
    onConsolidate: vi.fn(),
    onKeyColumnsChange: vi.fn(),
    ...overrides,
  };
  render(
    <DatasetComparisonSection
      status={props.status}
      datasetColumns={[
        { name: "id", dataType: "Int64" },
        { name: "importe", dataType: "String" },
      ]}
      datasetRevision={1}
      keyColumns={["id"]}
      onKeyColumnsChange={props.onKeyColumnsChange}
      onCompare={vi.fn()}
      onCancelComparison={vi.fn()}
      comparisonCancellationPending={props.comparisonCancellationPending}
      onClear={vi.fn()}
      onConsolidate={props.onConsolidate}
      onResolveConflicts={props.onResolveConflicts}
      onConflictPageChange={props.onConflictPageChange}
      joinStatus={props.joinStatus}
      reviewMutationStatus={props.reviewMutationStatus}
      reviewMutationCancellationPending={props.reviewMutationCancellationPending}
      joinType="inner"
      onJoinTypeChange={vi.fn()}
      onJoin={vi.fn()}
      onCancelReviewMutation={props.onCancelReviewMutation}
    />,
  );
  return props;
}

const keyTypeMessage = "La columna clave 'id' es un número entero en el dataset activo y texto en el comparado. Conviértela al mismo tipo en Preparar (por ejemplo, a texto) y vuelve a comparar.";

describe("DatasetComparisonSection", () => {
  it("no deja cambiar la clave bajo un resultado visible y explica cómo hacerlo (UX-05)", () => {
    renderSection();
    expect(screen.getByRole("checkbox", { name: /importe/ })).toBeDisabled();
    expect(screen.getByRole("checkbox", { name: /id/ })).toBeDisabled();
    expect(screen.getByText(/Para cambiar la clave, descarta antes la comparación/)).toBeInTheDocument();
  });

  it("dice qué hoja y encabezado se leyeron del archivo comparado (FUN-41)", () => {
    renderSection({
      status: {
        kind: "ready",
        comparison: {
          ...comparison,
          comparedSourceNote: "Se comparó la hoja «Datos», la primera de 2 del libro, con la primera fila como encabezado.",
        },
      },
    });
    expect(screen.getByText(/Se comparó la hoja «Datos», la primera de 2/)).toBeInTheDocument();
    expect(screen.getAllByText("Ninguna")).toHaveLength(2);
  });

  it("permite elegir la clave antes de comparar", () => {
    const props = renderSection({ status: { kind: "idle" } });
    fireEvent.click(screen.getByRole("checkbox", { name: /importe/ }));
    expect(props.onKeyColumnsChange).toHaveBeenCalledWith(["id", "importe"]);
  });

  it("guía cuando los tipos de la clave no coinciden (UX-06)", () => {
    renderSection({ status: { kind: "error", message: keyTypeMessage } });
    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent("No se pudo comparar la fuente");
    expect(alert).toHaveTextContent("Conviértela al mismo tipo en Preparar");
  });

  it.each([
    ["unión", { joinStatus: { kind: "error", message: "sin claves" } }, "No se pudieron unir los datasets: sin claves"],
    ["cancelación de la comparación", { status: { kind: "loading", cancellationError: "ocupado" } }, "No se pudo solicitar la cancelación: ocupado"],
    [
      "cancelación de una mutación",
      { reviewMutationStatus: { kind: "running", mutation: "join", cancellation: "requested", cancellationError: "tarde" } },
      "No se pudo solicitar la cancelación: tarde",
    ],
    ["consolidación", { reviewMutationStatus: { kind: "error", mutation: "consolidate", message: "esquemas" } }, "No se pudieron consolidar las filas: esquemas"],
    ["resolución", { reviewMutationStatus: { kind: "error", mutation: "resolveConflicts", message: "obsoleto" } }, "No se pudieron resolver los conflictos: obsoleto"],
  ])("muestra el aviso de error de %s", (_, overrides, text) => {
    renderSection(overrides as Overrides);
    expect(screen.getByRole("alert")).toHaveTextContent(text);
  });

  it("explica la cancelación pendiente mientras lee la segunda fuente", () => {
    renderSection({ status: { kind: "loading" }, comparisonCancellationPending: true });
    expect(screen.getByText(/Cancelación solicitada. Si el selector de archivos sigue abierto/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Esperando cancelación…" })).toBeDisabled();
  });

  it.each([
    [{ ...comparison, conflictingKeyCount: 2, canConsolidate: false }, "existen conflictos de valores"],
    [{ ...comparison, duplicateKeyCount: 1, canConsolidate: false }, "existen claves duplicadas"],
    [{ ...comparison, keyColumns: [], canConsolidate: false }, "mismas columnas en el mismo orden"],
  ])("explica por qué la consolidación está bloqueada (%#)", (blocked, text) => {
    renderSection({ status: { kind: "ready", comparison: blocked } });
    expect(screen.getByRole("note")).toHaveTextContent(text);
    expect(screen.getByRole("button", { name: "Consolidar filas" })).toBeDisabled();
  });

  it.each([
    [{ kind: "running", mutation: "consolidate", cancellation: "available" }, "Cancelar consolidación", false],
    [{ kind: "running", mutation: "consolidate", cancellation: "requested" }, "Cancelando consolidación…", true],
    [{ kind: "finalizing", mutation: "consolidate" }, "Finalizando consolidación…", true],
    [{ kind: "running", mutation: "join", cancellation: "available" }, "Esperando unión…", true],
    [{ kind: "running", mutation: "resolveConflicts", cancellation: "available" }, "Esperando resolución…", true],
  ])("rotula el botón de consolidar durante una mutación (%#)", (mutation, label, disabled) => {
    const props = renderSection({ reviewMutationStatus: mutation as ReviewMutationStatus });
    const button = screen.getAllByRole("button", { name: label }).at(-1)!;
    if (disabled) {
      expect(button).toBeDisabled();
    } else {
      fireEvent.click(button);
      expect(props.onCancelReviewMutation).toHaveBeenCalledOnce();
    }
  });

  it("rotula el botón de unir mientras une, finaliza o espera la cancelación", () => {
    renderSection({ reviewMutationStatus: { kind: "finalizing", mutation: "join" } });
    expect(screen.getByRole("button", { name: "Finalizando unión…" })).toBeDisabled();
    cleanup();
    renderSection({ reviewMutationStatus: { kind: "running", mutation: "join", cancellation: "requested" } });
    expect(screen.getByRole("button", { name: "Cancelando unión…" })).toBeDisabled();
    cleanup();
    renderSection({
      reviewMutationCancellationPending: true,
      reviewMutationStatus: { kind: "running", mutation: "join", cancellation: "available" },
    });
    expect(screen.getAllByRole("button", { name: "Esperando cancelación…" }).length).toBeGreaterThan(0);
  });

  it("excluye o elige el origen de cada celda y resuelve la página", () => {
    const props = renderSection({ status: { kind: "ready", comparison: withConflicts } });
    const resolve = screen.getByRole("button", { name: "Resolver conflictos" });
    expect(resolve).toBeDisabled();
    const exclude = screen.getByRole("checkbox", { name: /Excluir la fila activa de la clave 7/ });
    fireEvent.click(exclude);
    expect(screen.getByText("1 fila activa se excluirá del resultado.")).toBeInTheDocument();
    fireEvent.click(exclude);
    fireEvent.click(screen.getByRole("radio", { name: "Usar comparado en importe" }));
    fireEvent.click(resolve);
    expect(props.onResolveConflicts).toHaveBeenCalledWith([
      { action: "useSource", conflictIndex: 50, column: "importe", source: "compared" },
    ]);
  });

  it("muestra el estado de la resolución en curso", () => {
    renderSection({
      status: { kind: "ready", comparison: withConflicts },
      reviewMutationStatus: { kind: "running", mutation: "resolveConflicts", cancellation: "requested" },
    });
    expect(screen.getByRole("button", { name: "Cancelando resolución…" })).toBeDisabled();
    expect(screen.getByText(/Cancelación solicitada. Esperando a que la resolución se detenga/)).toBeInTheDocument();
    cleanup();
    renderSection({
      status: { kind: "ready", comparison: withConflicts },
      reviewMutationStatus: { kind: "finalizing", mutation: "resolveConflicts" },
    });
    expect(screen.getByText(/se está publicando el resultado validado/)).toBeInTheDocument();
  });

  it("pagina hacia atrás, permite cancelar la carga y avisa si la cancelación falla", async () => {
    let finishPage!: () => void;
    const onConflictPageChange = vi.fn(() => new Promise<void>((resolve) => { finishPage = resolve; }));
    vi.spyOn(bridge, "cancelOperation").mockRejectedValue(new Error("sin operación"));
    renderSection({ status: { kind: "ready", comparison: withConflicts }, onConflictPageChange });
    const pager = screen.getByRole("navigation", { name: "Paginación de conflictos" });
    expect(pager).toHaveTextContent("Conflictos 51–51 de 60");
    fireEvent.click(within(pager).getByRole("button", { name: "Conflictos anteriores" }));
    expect(onConflictPageChange).toHaveBeenCalledWith(0);
    fireEvent.click(await screen.findByRole("button", { name: "Cancelar carga" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("No se pudo cancelar la carga de conflictos: sin operación");
    finishPage();
    await waitFor(() => expect(screen.queryByRole("button", { name: "Cancelar carga" })).not.toBeInTheDocument());
  });
});
