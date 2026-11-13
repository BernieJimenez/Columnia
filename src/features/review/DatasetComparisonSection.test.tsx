import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as bridge from "../../bridge";
import type { DatasetComparison } from "../../bridge";
import { DatasetComparisonSection } from "./DatasetComparisonSection";
import {
  makeReviewComparison,
  testComparison as comparison,
  testComparisonWithConflicts as withConflicts,
} from "./comparisonTestSupport";
import type { ReviewComparison } from "./useReviewController";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function renderSection(overrides: Partial<ReviewComparison> = {}, onCombineInPrepare: (() => void) | undefined = vi.fn()) {
  const props = makeReviewComparison(overrides);
  render(
    <DatasetComparisonSection
      comparison={props}
      datasetColumns={[
        { name: "id", dataType: "Int64" },
        { name: "importe", dataType: "String" },
      ]}
      onCombineInPrepare={onCombineInPrepare}
    />,
  );
  return { ...props, onCombineInPrepare };
}

const keyTypeMessage = "La columna clave 'id' es un número entero en el dataset activo y texto en el comparado. Conviértela al mismo tipo en Preparar (por ejemplo, a texto) y vuelve a comparar.";

describe("DatasetComparisonSection (Revisar)", () => {
  it("primer paso: clave, qué cuenta como igual y una sola acción primaria", () => {
    const props = renderSection({ status: { kind: "idle" } });
    expect(screen.getByRole("heading", { name: "Elige la clave y el otro archivo" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("checkbox", { name: /importe/ }));
    expect(props.onKeyColumnsChange).toHaveBeenCalledWith(["id", "importe"]);
    fireEvent.click(screen.getByRole("checkbox", { name: /Ignorar mayúsculas/ }));
    expect(props.onOptionsChange).toHaveBeenCalledWith({ numericTolerance: false, ignoreCase: true, trimSpaces: false });
    const compare = screen.getByRole("button", { name: "Comparar con otro archivo" });
    expect(compare).toHaveClass("primary-action");
    expect(document.querySelectorAll(".primary-action")).toHaveLength(1);
    fireEvent.click(compare);
    expect(props.onCompare).toHaveBeenCalledOnce();
  });

  it("no ofrece unir, consolidar ni resolver: Revisar no cambia el dataset", () => {
    renderSection({ status: { kind: "ready", comparison: withConflicts } });
    for (const name of [/Unir/, /Consolidar/, /Resolver conflictos/, /Elegir fuente/]) {
      expect(screen.queryByRole("button", { name })).not.toBeInTheDocument();
    }
    expect(screen.queryByRole("radio")).not.toBeInTheDocument();
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
    expect(document.querySelectorAll(".primary-action")).toHaveLength(0);
    expect(screen.getByText(/Tu dataset no ha cambiado/)).toBeInTheDocument();
  });

  it("segundo paso: el resultado con filas nuevas, que faltan y conflictos, y «Combinar en Preparar»", () => {
    const props = renderSection({
      status: {
        kind: "ready",
        comparison: { ...withConflicts, comparedOnlyKeyCount: 4, currentOnlyKeyCount: 2 },
      },
    });
    expect(screen.getByRole("heading", { name: "Resultado de la comparación" })).toBeInTheDocument();
    const counts = screen.getByLabelText("Resultado por clave");
    expect(within(counts).getByText("Filas nuevas (solo en el otro archivo)").nextElementSibling).toHaveTextContent("4");
    expect(within(counts).getByText("Filas que faltan (solo en tu dataset)").nextElementSibling).toHaveTextContent("2");
    expect(within(counts).getByText(/^Conflictos/).nextElementSibling).toHaveTextContent("60");
    expect(screen.getByText(/importe:/)).toHaveTextContent("importe: 10 en tu dataset, 12 en el otro archivo");
    fireEvent.click(screen.getByRole("button", { name: "Combinar en Preparar" }));
    expect(props.onCombineInPrepare).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Descartar comparación" }));
    expect(props.onClear).toHaveBeenCalledOnce();
  });

  it("dice qué se usó al comparar, qué hoja se leyó y cómo cambiar la clave (PROD-21, FUN-41, UX-05)", () => {
    renderSection({
      status: {
        kind: "ready",
        comparison: {
          ...comparison,
          options: { numericTolerance: true, ignoreCase: true, trimSpaces: false },
          comparedSourceNote: "Se comparó la hoja «Datos», la primera de 2 del libro, con la primera fila como encabezado.",
        },
      },
    });
    expect(screen.getByText(/Se comparó tolerando el redondeo de los decimales e ignorando mayúsculas/)).toBeInTheDocument();
    expect(screen.getByText(/Se comparó la hoja «Datos», la primera de 2/)).toBeInTheDocument();
    expect(screen.getByText(/Para cambiar la clave, descarta antes la comparación/)).toBeInTheDocument();
    expect(screen.getAllByText("Ninguna")).toHaveLength(2);
  });

  it("dice con una sola opción, con las tres o con ninguna qué se usó al comparar (PROD-21)", () => {
    renderSection({ status: { kind: "ready", comparison: { ...comparison, options: { numericTolerance: false, ignoreCase: false, trimSpaces: true } } } });
    expect(screen.getByText("Se comparó ignorando los espacios al principio y al final.")).toBeInTheDocument();
    cleanup();
    renderSection({ status: { kind: "ready", comparison: { ...comparison, options: { numericTolerance: true, ignoreCase: true, trimSpaces: true } } } });
    expect(screen.getByText(
      "Se comparó tolerando el redondeo de los decimales, ignorando mayúsculas e ignorando los espacios al principio y al final.",
    )).toBeInTheDocument();
    cleanup();
    renderSection({ status: { kind: "ready", comparison: { ...comparison, options: undefined } as unknown as DatasetComparison } });
    expect(screen.queryByText(/^Se comparó /)).not.toBeInTheDocument();
  });

  it("guía cuando los tipos de la clave no coinciden (UX-06)", () => {
    renderSection({ status: { kind: "error", message: keyTypeMessage } });
    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent("No se pudo comparar el archivo");
    expect(alert).toHaveTextContent("Conviértela al mismo tipo en Preparar");
  });

  it("explica la cancelación pendiente y avisa si falla", () => {
    renderSection({ status: { kind: "loading" }, cancellationPending: true });
    expect(screen.getByText(/Cancelación solicitada. Si el selector de archivos sigue abierto/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Esperando cancelación…" })).toBeDisabled();
    cleanup();
    renderSection({ status: { kind: "loading", cancellationError: "ocupado" } });
    expect(screen.getByRole("alert")).toHaveTextContent("No se pudo solicitar la cancelación: ocupado");
    expect(screen.getByRole("status")).toHaveTextContent("Leyendo el otro archivo…");
  });

  it("cuenta las filas que las claves repetidas dejan sin comparar (FUN-82)", () => {
    renderSection({ status: { kind: "ready", comparison: { ...comparison, duplicateKeyCount: 2, duplicateKeyRowCount: 5, canConsolidate: false } } });
    expect(screen.getByText(/5 filas tienen una clave repetida/)).toBeInTheDocument();
  });

  it("sin clave resume las filas completas", () => {
    renderSection({ status: { kind: "ready", comparison: { ...comparison, keyColumns: [] } } });
    const counts = screen.getByLabelText("Resultado por filas completas");
    expect(within(counts).getByText("Filas en los dos").nextElementSibling).toHaveTextContent("2");
    expect(screen.getByText(/por filas completas/)).toBeInTheDocument();
  });

  it("distingue un valor nulo, uno vacío y el texto «null» (TXT-12)", () => {
    renderSection({
      status: {
        kind: "ready",
        comparison: {
          ...withConflicts,
          conflicts: [{ key: [null], cells: [
            { column: "nota", current: null, compared: "null" },
            { column: "otra", current: "", compared: "x" },
          ] }],
        },
      },
    });
    expect(screen.getByText("Clave (nulo)")).toBeInTheDocument();
    expect(screen.getByText("valor ausente")).toBeInTheDocument();
    expect(screen.getByText("null")).toBeInTheDocument();
    expect(screen.getByText("(vacío)")).toBeInTheDocument();
  });

  it("pagina los conflictos sin pedir decisiones, permite cancelar la carga y avisa si falla", async () => {
    let finishPage!: () => void;
    const onConflictPageChange = vi.fn(() => new Promise<void>((resolve) => { finishPage = resolve; }));
    vi.spyOn(bridge, "cancelOperation").mockRejectedValue(new Error("sin operación"));
    renderSection({ status: { kind: "ready", comparison: { ...withConflicts, conflictOffset: 0, conflictsTruncated: true } }, onConflictPageChange });
    const pager = screen.getByRole("navigation", { name: "Paginación de conflictos" });
    expect(pager).toHaveTextContent("Conflictos 1–1 de 60");
    fireEvent.click(within(pager).getByRole("button", { name: "Siguientes conflictos" }));
    expect(onConflictPageChange).toHaveBeenCalledWith(1);
    fireEvent.click(await screen.findByRole("button", { name: "Cancelar carga" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("No se pudo cancelar la carga de conflictos: sin operación");
    finishPage();
    await waitFor(() => expect(screen.queryByRole("button", { name: "Cancelar carga" })).not.toBeInTheDocument());
  });

  it("asocia el motivo de ocupado mientras Preparar combina (AUD-059)", () => {
    renderSection({ mutationStatus: { kind: "running", mutation: "join", cancellation: "available" } });
    const discard = screen.getByRole("button", { name: "Descartar comparación" });
    expect(discard).toBeDisabled();
    expect(discard).toHaveAttribute("aria-describedby", "busy-reason");
    cleanup();
    renderSection();
    expect(screen.getByRole("button", { name: "Descartar comparación" })).not.toHaveAttribute("aria-describedby");
  });
});
