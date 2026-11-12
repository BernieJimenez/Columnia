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
  options: { numericTolerance: false, ignoreCase: false, trimSpaces: false },
};

const withConflicts = {
  ...comparison,
  conflictingKeyCount: 60,
  canConsolidate: false,
  options: { numericTolerance: false, ignoreCase: false, trimSpaces: false },
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
  options: bridge.ComparisonOptions;
  onOptionsChange: (options: bridge.ComparisonOptions) => void;
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
    onJoin: vi.fn(),
    onKeyColumnsChange: vi.fn(),
    options: { numericTolerance: false, ignoreCase: false, trimSpaces: false },
    onOptionsChange: vi.fn() as (options: bridge.ComparisonOptions) => void,
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
      options={props.options}
      onOptionsChange={props.onOptionsChange}
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
      onJoin={props.onJoin}
      onCancelReviewMutation={props.onCancelReviewMutation}
    />,
  );
  return props;
}

const keyTypeMessage = "La columna clave 'id' es un número entero en el dataset activo y texto en el comparado. Conviértela al mismo tipo en Preparar (por ejemplo, a texto) y vuelve a comparar.";

describe("DatasetComparisonSection", () => {
  it("deja elegir qué cuenta como igual y el resultado dice qué se usó (PROD-21)", () => {
    const onOptionsChange = vi.fn();
    renderSection({ status: { kind: "idle" } as ComparisonStatus, onOptionsChange });
    fireEvent.click(screen.getByRole("checkbox", { name: /Ignorar mayúsculas/ }));
    expect(onOptionsChange).toHaveBeenCalledWith({ numericTolerance: false, ignoreCase: true, trimSpaces: false });
    cleanup();

    renderSection({
      status: {
        kind: "ready",
        comparison: { ...comparison, options: { numericTolerance: true, ignoreCase: true, trimSpaces: false } },
      } as ComparisonStatus,
      options: { numericTolerance: true, ignoreCase: true, trimSpaces: false },
    });
    expect(screen.getByRole("checkbox", { name: /Tolerar el redondeo/ })).toBeDisabled();
    expect(screen.getByText(/Se comparó tolerando el redondeo de los decimales e ignorando mayúsculas/))
      .toBeInTheDocument();
  });

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
    [{ ...comparison, conflictingKeyCount: 2, canConsolidate: false, options: { numericTolerance: false, ignoreCase: false, trimSpaces: false } }, "existen conflictos de valores"],
    [{ ...comparison, duplicateKeyCount: 1, canConsolidate: false, options: { numericTolerance: false, ignoreCase: false, trimSpaces: false } }, "existen claves duplicadas"],
    [{ ...comparison, keyColumns: [], canConsolidate: false, options: { numericTolerance: false, ignoreCase: false, trimSpaces: false } }, "mismas columnas en el mismo orden"],
  ])("explica por qué la consolidación está bloqueada (%#)", (blocked, text) => {
    renderSection({ status: { kind: "ready", comparison: blocked } });
    expect(screen.getByRole("note")).toHaveTextContent(text);
    expect(screen.getByRole("button", { name: "Consolidar filas" })).toHaveAttribute("aria-disabled", "true");
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
      expect(button).toHaveAttribute("aria-disabled", "true");
    } else {
      fireEvent.click(button);
      expect(props.onCancelReviewMutation).toHaveBeenCalledOnce();
    }
  });

  it("rotula el botón de unir mientras une, finaliza o espera la cancelación", () => {
    renderSection({ reviewMutationStatus: { kind: "finalizing", mutation: "join" } });
    expect(screen.getByRole("button", { name: "Finalizando unión…" })).toHaveAttribute("aria-disabled", "true");
    cleanup();
    renderSection({ reviewMutationStatus: { kind: "running", mutation: "join", cancellation: "requested" } });
    expect(screen.getByRole("button", { name: "Cancelando unión…" })).toHaveAttribute("aria-disabled", "true");
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
    expect(resolve).toHaveAttribute("aria-disabled", "true");
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

  it("una acción resuelve los conflictos de todas las páginas (UX-20)", () => {
    const firstPage = {
      ...withConflicts,
      conflictingKeyCount: 3,
      conflictOffset: 0,
      conflictsTruncated: true,
      conflicts: [
        { key: ["1"], cells: [{ column: "importe", current: "10", compared: "12" }, { column: "estado", current: "a", compared: "b" }] },
      ],
    } as unknown as DatasetComparison;
    const props = renderSection({ status: { kind: "ready", comparison: firstPage } });
    const resolve = screen.getByRole("button", { name: "Resolver conflictos" });
    expect(resolve).toHaveAttribute("aria-disabled", "true");
    // The page action answers every cell on screen.
    fireEvent.click(screen.getByRole("button", { name: "Conservar el activo en esta página" }));
    expect(screen.getByRole("radio", { name: "Conservar activo en importe" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "Conservar activo en estado" })).toBeChecked();
    // The global action covers the conflicts of the pages not opened yet.
    fireEvent.click(screen.getByRole("button", { name: "Usar el comparado en los demás conflictos" }));
    expect(resolve).toBeEnabled();
    fireEvent.click(resolve);
    expect(props.onResolveConflicts).toHaveBeenCalledWith([
      { action: "useSource", conflictIndex: 0, column: "importe", source: "current" },
      { action: "useSource", conflictIndex: 0, column: "estado", source: "current" },
      { action: "useSource", conflictIndex: 1, source: "compared" },
      { action: "useSource", conflictIndex: 2, source: "compared" },
    ]);
  });

  it("muestra el estado de la resolución en curso", () => {
    renderSection({
      status: { kind: "ready", comparison: withConflicts },
      reviewMutationStatus: { kind: "running", mutation: "resolveConflicts", cancellation: "requested" },
    });
    expect(screen.getByRole("button", { name: "Cancelando resolución…" })).toHaveAttribute("aria-disabled", "true");
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

  it("no deja unir mientras la comparación lee la otra fuente (UX-16)", () => {
    renderSection({ status: { kind: "loading" } });
    expect(screen.getByRole("button", { name: "Elegir fuente y unir" })).toHaveAttribute("aria-disabled", "true");
  });

  it("cuenta las filas que las claves repetidas dejan sin comparar (FUN-82)", () => {
    renderSection({ status: { kind: "ready", comparison: { ...comparison, duplicateKeyCount: 2, duplicateKeyRowCount: 5, canConsolidate: false, options: { numericTolerance: false, ignoreCase: false, trimSpaces: false } } } });
    expect(screen.getByText(/5 filas tienen una clave repetida/)).toBeInTheDocument();
  });

  it("distingue un valor nulo, uno vacío y el texto «null» (TXT-12)", () => {
    renderSection({
      status: {
        kind: "ready",
        comparison: {
          ...withConflicts,
          conflictsTruncated: true,
          conflicts: [{ key: [null], cells: [
            { column: "nota", current: null, compared: "null" },
            { column: "otra", current: "", compared: "x" },
          ] }],
        } as unknown as DatasetComparison,
      },
    });
    const conflict = screen.getByRole("group", { name: /clave \(nulo\)/ });
    expect(within(conflict).getAllByText("(nulo)").length).toBeGreaterThan(0);
    expect(within(conflict).getByText("null")).toBeInTheDocument();
    expect(within(conflict).getByText("(vacío)")).toBeInTheDocument();
    // The page still lacks decisions, so it is not «completa».
    expect(screen.getByText(/Faltan decisiones en esta página/)).toBeInTheDocument();
  });
  it("dice con una sola opción, con las tres o con ninguna qué se usó al comparar (PROD-21)", () => {
    renderSection({
      status: {
        kind: "ready",
        comparison: { ...comparison, options: { numericTolerance: false, ignoreCase: false, trimSpaces: true } },
      } as ComparisonStatus,
    });
    expect(screen.getByText("Se comparó ignorando los espacios al principio y al final.")).toBeInTheDocument();
    cleanup();

    renderSection({
      status: {
        kind: "ready",
        comparison: { ...comparison, options: { numericTolerance: true, ignoreCase: true, trimSpaces: true } },
      } as ComparisonStatus,
    });
    expect(screen.getByText(
      "Se comparó tolerando el redondeo de los decimales, ignorando mayúsculas e ignorando los espacios al principio y al final.",
    )).toBeInTheDocument();
    cleanup();

    // A result from before the options existed says nothing about them.
    renderSection({
      status: { kind: "ready", comparison: { ...comparison, options: undefined } } as unknown as ComparisonStatus,
    });
    expect(screen.queryByText(/Se comparó /)).not.toBeInTheDocument();
  });

  it("dice «Ninguna» cuando los datasets no comparten columnas", () => {
    renderSection({
      status: {
        kind: "ready",
        comparison: {
          ...comparison,
          sharedColumns: [],
          currentOnlyColumns: ["id"],
          comparedOnlyColumns: ["codigo"],
          schemaCompatible: false,
          canConsolidate: false,
        },
      } as ComparisonStatus,
    });
    const columns = screen.getByLabelText("Resultado de columnas");
    expect(within(columns).getByRole("heading", { name: "Columnas compartidas (0)" }).nextElementSibling).toHaveTextContent("Ninguna");
    expect(within(columns).getByText("codigo")).toBeInTheDocument();
  });

  it("decide por página, cambia de idea y aplica la fuente elegida a los demás conflictos (UX-20)", () => {
    const page = {
      ...withConflicts,
      conflictingKeyCount: 4,
      conflictOffset: 0,
      conflictsTruncated: true,
      conflicts: [
        { key: ["1"], cells: [{ column: "importe", current: "10", compared: "12" }, { column: "estado", current: "a", compared: "b" }] },
        { key: ["2"], cells: [{ column: "importe", current: "20", compared: "22" }] },
      ],
    } as unknown as DatasetComparison;
    const props = renderSection({ status: { kind: "ready", comparison: page } });
    const conflict = (number: number) => within(screen.getByRole("group", { name: new RegExp(`^Conflicto ${number} `) }));
    const resolve = screen.getByRole("button", { name: "Resolver conflictos" });

    // «Usar el comparado en esta página» skips the key that was excluded.
    fireEvent.click(conflict(2).getByRole("checkbox", { name: /Excluir la fila activa/ }));
    expect(conflict(2).getByRole("radio", { name: "Usar comparado en importe" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Usar el comparado en esta página" }));
    expect(conflict(1).getByRole("radio", { name: "Usar comparado en importe" })).toBeChecked();
    expect(conflict(2).getByRole("radio", { name: "Usar comparado en importe" })).not.toBeChecked();

    // The user changes one cell back to the active value.
    fireEvent.click(conflict(1).getByRole("radio", { name: "Conservar activo en estado" }));
    expect(conflict(1).getByRole("radio", { name: "Conservar activo en estado" })).toBeChecked();

    // The same button turns the «demás conflictos» rule on and off, and says what it does.
    const keepRest = screen.getByRole("button", { name: "Conservar el activo en los demás conflictos" });
    fireEvent.click(keepRest);
    expect(keepRest).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByText(/usarán el valor del activo en todas sus columnas/)).toBeInTheDocument();
    fireEvent.click(keepRest);
    expect(keepRest).toHaveAttribute("aria-pressed", "false");
    expect(screen.queryByText(/usarán el valor del activo/)).not.toBeInTheDocument();
    expect(resolve).toHaveAttribute("aria-disabled", "true");

    // With the rule on, the key excluded stays excluded and the unseen ones follow the rule.
    fireEvent.click(keepRest);
    fireEvent.click(resolve);
    expect(props.onResolveConflicts).toHaveBeenCalledWith([
      { action: "exclude", conflictIndex: 1 },
      { action: "useSource", conflictIndex: 0, column: "importe", source: "compared" },
      { action: "useSource", conflictIndex: 0, column: "estado", source: "current" },
      { action: "useSource", conflictIndex: 2, source: "current" },
      { action: "useSource", conflictIndex: 3, source: "current" },
    ]);
  });

  it("una celda sin decidir toma la fuente de los demás conflictos y las exclusiones se cuentan en plural", () => {
    const page = {
      ...withConflicts,
      conflictingKeyCount: 3,
      conflictOffset: 0,
      conflictsTruncated: true,
      conflicts: [
        { key: ["1"], cells: [{ column: "importe", current: "10", compared: "12" }, { column: "estado", current: "a", compared: "b" }] },
        { key: ["2"], cells: [{ column: "importe", current: "20", compared: "22" }] },
      ],
    } as unknown as DatasetComparison;
    const props = renderSection({ status: { kind: "ready", comparison: page } });
    const conflict = (number: number) => within(screen.getByRole("group", { name: new RegExp(`^Conflicto ${number} `) }));

    fireEvent.click(conflict(1).getByRole("checkbox", { name: /Excluir la fila activa/ }));
    fireEvent.click(conflict(2).getByRole("checkbox", { name: /Excluir la fila activa/ }));
    expect(screen.getByText("2 filas activas se excluirán del resultado.")).toBeInTheDocument();
    // Un-excluding the first key makes its cells decidable again; only one is decided.
    fireEvent.click(conflict(1).getByRole("checkbox", { name: /Excluir la fila activa/ }));
    expect(screen.getByText("1 fila activa se excluirá del resultado.")).toBeInTheDocument();
    fireEvent.click(conflict(1).getByRole("radio", { name: "Conservar activo en importe" }));
    fireEvent.click(screen.getByRole("button", { name: "Usar el comparado en los demás conflictos" }));
    fireEvent.click(screen.getByRole("button", { name: "Resolver conflictos" }));
    expect(props.onResolveConflicts).toHaveBeenCalledWith([
      { action: "exclude", conflictIndex: 1 },
      { action: "useSource", conflictIndex: 0, column: "importe", source: "current" },
      { action: "useSource", conflictIndex: 0, column: "estado", source: "compared" },
      { action: "useSource", conflictIndex: 2, source: "compared" },
    ]);
  });

  it("bloquea el botón de resolver mientras otra mutación trabaja y dice cuál", () => {
    renderSection({
      status: { kind: "ready", comparison: withConflicts },
      reviewMutationStatus: { kind: "running", mutation: "join", cancellation: "available" },
    });
    expect(screen.getAllByRole("button", { name: "Esperando unión…" })).not.toHaveLength(0);
    cleanup();
    renderSection({
      status: { kind: "ready", comparison: withConflicts },
      reviewMutationStatus: { kind: "running", mutation: "consolidate", cancellation: "available" },
    });
    const waiting = screen.getAllByRole("button", { name: "Esperando consolidación…" });
    expect(waiting.length).toBeGreaterThan(0);
    for (const button of waiting) expect(button).toHaveAttribute("aria-disabled", "true");
    expect(screen.queryByRole("button", { name: "Resolver conflictos" })).not.toBeInTheDocument();
  });

  it("asocia el motivo de ocupado a los controles de la comparación solo durante una mutación (AUD-059)", () => {
    renderSection({ reviewMutationStatus: { kind: "running", mutation: "join", cancellation: "available" } as ReviewMutationStatus });
    expect(screen.getByRole("button", { name: "Descartar comparación" })).toHaveAttribute("aria-describedby", "busy-reason");
    expect(screen.getByRole("button", { name: "Esperando unión…" })).toHaveAttribute("aria-disabled", "true");
    cleanup();
    renderSection();
    expect(screen.getByRole("button", { name: "Descartar comparación" })).not.toHaveAttribute("aria-describedby");
  });

  it("los botones principales son aria-disabled, siguen en el orden de foco y no actúan bloqueados (AUD-059)", () => {
    const props = renderSection({ status: { kind: "loading" } });
    const join = screen.getByRole("button", { name: "Elegir fuente y unir" });
    expect(join).toHaveAttribute("aria-disabled", "true");
    expect(join).not.toBeDisabled();
    join.focus();
    expect(document.activeElement).toBe(join);
    fireEvent.click(join);
    expect(props.onJoin).not.toHaveBeenCalled();
    expect(join).not.toHaveAttribute("aria-describedby");
    cleanup();
    renderSection({ reviewMutationStatus: { kind: "finalizing", mutation: "join" } });
    const finalizing = screen.getByRole("button", { name: "Finalizando unión…" });
    expect(finalizing).toHaveAttribute("aria-describedby", "busy-reason");
  });});
