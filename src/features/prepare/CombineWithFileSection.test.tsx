import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { DatasetComparison, HistoryState } from "../../bridge";
import {
  makeReviewComparison,
  testComparison as comparison,
  testComparisonWithConflicts as withConflicts,
} from "../review/comparisonTestSupport";
import type { ReviewComparison } from "../review/useReviewController";
import { CombineWithFileSection } from "./CombineWithFileSection";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const history = (snapshotsEnabled: boolean): HistoryState => ({
  canUndo: false,
  canRedo: false,
  currentIndex: 0,
  entryCount: 1,
  entries: [],
  snapshotsEnabled,
  degradedReason: null,
  maxEntries: 20,
  diskBytes: 0,
  diskBudgetBytes: 1,
});

function renderSection(overrides: Partial<ReviewComparison> = {}, { snapshotsEnabled = true, busy = false } = {}) {
  const props = makeReviewComparison(overrides);
  render(
    <CombineWithFileSection
      comparison={props}
      datasetColumns={[
        { name: "id", dataType: "Int64" },
        { name: "importe", dataType: "String" },
      ]}
      datasetRevision={1}
      historyStatus={history(snapshotsEnabled)}
      busy={busy}
    />,
  );
  return props;
}

const joinable: DatasetComparison = {
  ...comparison,
  currentRowCount: 10,
  matchedKeyCount: 7,
  currentOnlyKeyCount: 3,
  comparedOnlyKeyCount: 2,
  sharedColumns: ["id"],
  currentOnlyColumns: ["importe"],
  comparedOnlyColumns: ["segmento", "zona"],
};

describe("CombineWithFileSection (Preparar)", () => {
  it("sin comparación deja elegir el archivo aquí, sin acción primaria", () => {
    const props = renderSection({ status: { kind: "idle" } });
    expect(screen.getByRole("region", { name: "Combinar con otro archivo" })).toBeInTheDocument();
    // The summary of the details names the section; no heading repeats it.
    expect(screen.queryByRole("heading", { name: "Combinar con otro archivo" })).not.toBeInTheDocument();
    const compare = screen.getByRole("button", { name: "Comparar con otro archivo" });
    expect(compare).not.toHaveClass("primary-action");
    fireEvent.click(compare);
    expect(props.onCompare).toHaveBeenCalledOnce();
    expect(screen.queryByRole("button", { name: /^Unir/ })).not.toBeInTheDocument();
  });

  it("reutiliza la comparación hecha y resume la unión antes de hacerla", () => {
    const props = renderSection({ status: { kind: "ready", comparison: joinable } });
    expect(screen.getByText(/Comparado con/)).toHaveTextContent("Comparado con ventas-2.csv por id: 2 filas nuevas, 3 filas que faltan y 0 conflictos.");
    expect(screen.getByRole("radio", { name: /Solo coincidencias \(inner\)/ })).toBeChecked();
    expect(screen.getByRole("radio", { name: /Todas tus filas \(left\)/ })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: /Todas las filas de ambos \(full\)/ })).toBeInTheDocument();
    const join = within(screen.getByRole("region", { name: "Unir por clave" }));
    expect(join.getByText(/^Unir con ventas-2.csv por id/)).toHaveTextContent(
      "Unir con ventas-2.csv por id, solo coincidencias (inner): quedarán 7 filas y 4 columnas.",
    );
    // What any combination sets aside is said once for the section, not per action.
    expect(join.queryByText(/^El contrato de Entregar/)).not.toBeInTheDocument();
    expect(screen.getAllByText(/^El contrato de Entregar, la receta de Transformaciones/)).toHaveLength(1);
    expect(screen.getByText(/^El contrato de Entregar/).closest("dl")).toHaveTextContent("Al combinar se descartan");
    expect(screen.getByText(/«Deshacer» vuelve a las filas de antes y recupera lo descartado/)).toBeInTheDocument();
    expect(join.getByRole("button", { name: "Unir con ventas-2.csv" })).toHaveClass("secondary-action");
    fireEvent.click(join.getByRole("radio", { name: /\(full\)/ }));
    expect(props.onJoinTypeChange).toHaveBeenCalledWith("full");
    fireEvent.click(join.getByRole("button", { name: "Unir con ventas-2.csv" }));
    expect(props.onJoin).toHaveBeenCalledWith("inner");
  });

  it("calcula las filas de left y full, y no promete un número con claves repetidas", () => {
    renderSection({ status: { kind: "ready", comparison: joinable }, joinType: "full" });
    expect(screen.getByText(/^Unir con ventas-2.csv por id/)).toHaveTextContent("quedarán 12 filas y 4 columnas");
    cleanup();
    renderSection({ status: { kind: "ready", comparison: joinable }, joinType: "left" });
    expect(screen.getByText(/^Unir con ventas-2.csv por id/)).toHaveTextContent("quedarán 10 filas y 4 columnas");
    cleanup();
    renderSection({ status: { kind: "ready", comparison: { ...joinable, duplicateKeyCount: 1, canConsolidate: false } } });
    expect(screen.getByText(/^Unir con ventas-2.csv por id/)).toHaveTextContent("hay claves repetidas, así que el número de filas se sabrá al unir");
  });

  it("dice que no se puede deshacer cuando el historial está desactivado", () => {
    renderSection({ status: { kind: "ready", comparison: joinable } }, { snapshotsEnabled: false });
    expect(screen.getAllByText(/El historial está desactivado: no se podrá deshacer ni recuperar lo descartado. Guarda el proyecto antes/)).toHaveLength(1);
    expect(screen.queryByText(/«Deshacer» vuelve/)).not.toBeInTheDocument();
  });

  it("consolida con resumen previo y explica cuándo no se puede", () => {
    const props = renderSection({ status: { kind: "ready", comparison: joinable } });
    const consolidate = within(screen.getByRole("region", { name: "Añadir las filas nuevas" }));
    expect(consolidate.getByText(/^Se añaden al final/)).toHaveTextContent(
      "Se añaden al final las 2 filas de ventas-2.csv cuya clave no está en tu dataset: quedarán 12 filas y 2 columnas.",
    );
    const add = consolidate.getByRole("button", { name: "Añadir 2 filas nuevas" });
    expect(add).toHaveClass("secondary-action");
    fireEvent.click(add);
    expect(props.onConsolidate).toHaveBeenCalledOnce();
    cleanup();
    renderSection({ status: { kind: "ready", comparison: { ...comparison, keyColumns: [], canConsolidate: false } } });
    expect(screen.getByRole("note")).toHaveTextContent("las mismas columnas, en el mismo orden");
    expect(screen.queryByRole("button", { name: /^Añadir/ })).not.toBeInTheDocument();
    expect(screen.getByText(/compara de nuevo eligiendo al menos una columna clave/)).toBeInTheDocument();
  });

  it("una fila por celda en conflicto, la exclusión solo si se marca y el botón al pie", () => {
    const props = renderSection({ status: { kind: "ready", comparison: withConflicts } });
    const conflict = within(screen.getByRole("group", { name: /^Conflicto 51 · clave 7/ }));
    const cell = within(conflict.getByRole("radiogroup", { name: "importe" }));
    expect(cell.getByRole("radio", { name: /Tu dataset: 10/ })).toBeInTheDocument();
    expect(screen.queryByText(/se quitará de tu dataset/)).not.toBeInTheDocument();
    const resolve = screen.getByRole("button", { name: "Resolver conflictos" });
    expect(resolve).not.toHaveAttribute("aria-disabled");
    // The button follows the decisions.
    const section = screen.getByRole("region", { name: "Resolver conflictos" });
    expect(section.lastElementChild).toContainElement(resolve);

    fireEvent.click(resolve);
    expect(props.onResolveConflicts).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent(/Faltan decisiones/);

    fireEvent.click(conflict.getByRole("checkbox", { name: "Quitar esta fila del resultado" }));
    expect(screen.getByText(/La fila con esta clave se quitará de tu dataset/)).toBeInTheDocument();
    expect(cell.getByRole("radio", { name: /Otro archivo: 12/ })).toBeDisabled();
    expect(screen.getByText(/^Cada celda en conflicto/)).toHaveTextContent("y se quita 1 fila: quedarán 2 filas");
    fireEvent.click(conflict.getByRole("checkbox", { name: "Quitar esta fila del resultado" }));
    fireEvent.click(cell.getByRole("radio", { name: /Otro archivo: 12/ }));
    fireEvent.click(resolve);
    expect(props.onResolveConflicts).toHaveBeenCalledWith([
      { action: "useSource", conflictIndex: 50, column: "importe", source: "compared" },
    ]);
  });

  it("una acción resuelve los conflictos de todas las páginas (UX-20)", () => {
    const page = {
      ...withConflicts,
      conflictingKeyCount: 4,
      conflictOffset: 0,
      conflictsTruncated: true,
      conflicts: [
        { key: ["1"], cells: [{ column: "importe", current: "10", compared: "12" }, { column: "estado", current: "a", compared: "b" }] },
        { key: ["2"], cells: [{ column: "importe", current: "20", compared: "22" }] },
      ],
    } as DatasetComparison;
    const props = renderSection({ status: { kind: "ready", comparison: page } });
    const conflict = (number: number) => within(screen.getByRole("group", { name: new RegExp(`^Conflicto ${number} `) }));

    fireEvent.click(conflict(2).getByRole("checkbox", { name: "Quitar esta fila del resultado" }));
    fireEvent.click(screen.getByRole("button", { name: "Usar el otro archivo en esta página" }));
    expect(conflict(1).getByRole("radio", { name: /Otro archivo: 12/ })).toBeChecked();
    expect(conflict(2).getByRole("radio", { name: /Otro archivo: 22/ })).not.toBeChecked();
    fireEvent.click(within(conflict(1).getByRole("radiogroup", { name: "estado" })).getByRole("radio", { name: /Tu dataset/ }));

    const resolve = screen.getByRole("button", { name: "Resolver conflictos" });
    fireEvent.click(resolve);
    expect(screen.getByRole("alert")).toHaveTextContent(/Quedan conflictos en las páginas siguientes/);

    const keepRest = screen.getByRole("button", { name: "Conservar tu dataset en los demás conflictos" });
    fireEvent.click(keepRest);
    expect(keepRest).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByText(/usarán el valor de tu dataset en todas sus columnas/)).toBeInTheDocument();
    fireEvent.click(resolve);
    expect(props.onResolveConflicts).toHaveBeenCalledWith([
      { action: "exclude", conflictIndex: 1 },
      { action: "useSource", conflictIndex: 0, column: "importe", source: "compared" },
      { action: "useSource", conflictIndex: 0, column: "estado", source: "current" },
      { action: "useSource", conflictIndex: 2, source: "current" },
      { action: "useSource", conflictIndex: 3, source: "current" },
    ]);
  });

  it("rotula la acción en curso, cancela desde el mismo botón y bloquea las demás", () => {
    const props = renderSection({
      status: { kind: "ready", comparison: joinable },
      mutationStatus: { kind: "running", mutation: "consolidate", cancellation: "available" },
    });
    expect(screen.getByRole("status")).toHaveTextContent("Añadiendo las filas nuevas");
    fireEvent.click(screen.getByRole("button", { name: "Cancelar consolidación" }));
    expect(props.onCancelMutation).toHaveBeenCalledOnce();
    const waiting = screen.getByRole("button", { name: "Esperando consolidación…" });
    expect(waiting).toHaveAttribute("aria-disabled", "true");
    fireEvent.click(waiting);
    expect(props.onJoin).not.toHaveBeenCalled();
    cleanup();
    renderSection({
      status: { kind: "ready", comparison: joinable },
      mutationStatus: { kind: "finalizing", mutation: "join" },
    });
    expect(screen.getByRole("button", { name: "Finalizando unión…" })).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByRole("status")).toHaveTextContent("se está publicando el resultado");
    cleanup();
    renderSection({
      status: { kind: "ready", comparison: joinable },
      mutationStatus: { kind: "running", mutation: "join", cancellation: "requested" },
    });
    expect(screen.getByRole("button", { name: "Cancelando unión…" })).toHaveAttribute("aria-disabled", "true");
  });

  it("un solo aviso de error y una sola región de estado", () => {
    renderSection({ status: { kind: "ready", comparison: withConflicts }, joinStatus: { kind: "error", message: "sin claves" } });
    expect(screen.getAllByRole("alert")).toHaveLength(1);
    expect(screen.getByRole("alert")).toHaveTextContent("No se pudo unir: sin claves");
    expect(screen.getAllByRole("status")).toHaveLength(1);
    cleanup();
    renderSection({ status: { kind: "ready", comparison: withConflicts }, mutationStatus: { kind: "error", mutation: "resolveConflicts", message: "obsoleto" } });
    expect(screen.getByRole("alert")).toHaveTextContent("No se pudieron resolver los conflictos: obsoleto");
    cleanup();
    renderSection({ status: { kind: "ready", comparison: joinable }, mutationStatus: { kind: "error", mutation: "consolidate", message: "esquemas" } });
    expect(screen.getByRole("alert")).toHaveTextContent("No se pudieron añadir las filas: esquemas");
  });

  it("dice por qué se descartó la comparación tras un cambio de Preparar", () => {
    renderSection({
      status: { kind: "idle" },
      notice: "La comparación con ventas-2.csv se descartó porque cambió la columna clave id. Compara de nuevo para combinar.",
    });
    expect(screen.getByRole("note")).toHaveTextContent("se descartó porque cambió la columna clave id");
  });

  it("usa las clases de acción de la app y ninguna primaria", () => {
    renderSection({ status: { kind: "ready", comparison: withConflicts } });
    const section = screen.getByRole("region", { name: "Combinar con otro archivo" });
    expect(section.querySelector(".primary-action")).toBeNull();
    const bulk = within(screen.getByRole("group", { name: "Decidir varios conflictos a la vez" }));
    for (const button of bulk.getAllByRole("button")) expect(button).toHaveClass("secondary-action");
    for (const button of within(section).getAllByRole("button").filter((item) => item.closest(".conflict-resolution__pager") === null)) {
      expect(button.className).toMatch(/secondary-action/);
    }
  });

  it("no actúa mientras otro cambio de Preparar trabaja", () => {
    const props = renderSection({ status: { kind: "ready", comparison: joinable } }, { busy: true });
    const join = screen.getByRole("button", { name: "Unir con ventas-2.csv" });
    expect(join).toHaveAttribute("aria-disabled", "true");
    fireEvent.click(join);
    expect(props.onJoin).not.toHaveBeenCalled();
    const discard = screen.getByRole("button", { name: "Descartar comparación" });
    expect(discard).toHaveAttribute("aria-disabled", "true");
    fireEvent.click(discard);
    expect(props.onClear).not.toHaveBeenCalled();
  });
});
