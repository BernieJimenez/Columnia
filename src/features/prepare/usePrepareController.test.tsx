import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as bridge from "../../bridge";
import type { DatasetPreview, HistoryState } from "../../bridge";
import { EMPTY_HISTORY } from "./prepareModel";
import { usePrepareController } from "./usePrepareController";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const dataset: DatasetPreview = {
  fileName: "datos.csv",
  fileSizeBytes: 10,
  rowCount: 2,
  columnCount: 1,
  columns: [{ name: "nombre", dataType: "String" }],
  rows: [["Ana"], ["Ana"]],
};

const history: HistoryState = {
  ...EMPTY_HISTORY,
  canUndo: true,
  entryCount: 2,
  currentIndex: 1,
  entries: [
    { index: 0, label: "Dataset cargado", isCurrent: false },
    { index: 1, label: "Eliminar duplicados", isCurrent: true },
  ],
};

function ControllerHarness({
  onDatasetChanged,
  onProfileInvalidated,
  onDeliveryInvalidated,
}: {
  onDatasetChanged: (next: DatasetPreview) => void;
  onProfileInvalidated: () => void;
  onDeliveryInvalidated: () => void;
}) {
  const controller = usePrepareController({
    activeDataset: dataset,
    onDatasetChanged,
    onProfileInvalidated,
    onDeliveryInvalidated,
  });
  return <>
    <button type="button" onClick={controller.applyDuplicateRemoval}>Duplicar</button>
    <button type="button" onClick={controller.applyConstantColumnRemoval}>Constantes</button>
    <button type="button" onClick={controller.applyEmptyColumnRemoval}>Vacías</button>
    <button type="button" onClick={controller.undoChange}>Deshacer controlador</button>
    <output>{controller.changeStatus.kind === "applied" ? controller.changeStatus.message : controller.changeStatus.kind}</output>
    <span data-testid="history-index">{controller.historyStatus.currentIndex}</span>
  </>;
}

describe("usePrepareController", () => {
  it("publica la corrección, invalida perfil/entrega y refresca historial", async () => {
    vi.spyOn(bridge, "removeDuplicates").mockResolvedValue({ dataset, affectedRowCount: 1 });
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(history);
    const onDatasetChanged = vi.fn();
    const onProfileInvalidated = vi.fn();
    const onDeliveryInvalidated = vi.fn();
    render(<ControllerHarness {...{ onDatasetChanged, onProfileInvalidated, onDeliveryInvalidated }} />);

    fireEvent.click(screen.getByRole("button", { name: "Duplicar" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Se eliminaron 1 filas duplicadas adicionales."));
    expect(onDatasetChanged).toHaveBeenCalledWith(dataset);
    expect(onProfileInvalidated).toHaveBeenCalledOnce();
    expect(onDeliveryInvalidated).toHaveBeenCalledOnce();
    expect(screen.getByTestId("history-index")).toHaveTextContent("1");
  });

  it("restaura dataset e historial al deshacer e invalida resultados derivados", async () => {
    vi.spyOn(bridge, "undoLastChange").mockResolvedValue({
      dataset,
      history: EMPTY_HISTORY,
      message: "Se deshizo el último cambio.",
    });
    const onDatasetChanged = vi.fn();
    const onProfileInvalidated = vi.fn();
    const onDeliveryInvalidated = vi.fn();
    render(<ControllerHarness {...{ onDatasetChanged, onProfileInvalidated, onDeliveryInvalidated }} />);

    fireEvent.click(screen.getByRole("button", { name: "Deshacer controlador" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Se deshizo el último cambio."));
    expect(bridge.undoLastChange).toHaveBeenCalledOnce();
    expect(onDatasetChanged).toHaveBeenCalledWith(dataset);
    expect(onProfileInvalidated).toHaveBeenCalledOnce();
    expect(onDeliveryInvalidated).toHaveBeenCalledOnce();
  });

  it("publica la eliminación de columnas constantes y sus nombres", async () => {
    vi.spyOn(bridge, "removeConstantColumns").mockResolvedValue({
      dataset,
      removedColumnCount: 2,
      removedColumns: ["pais", "segmento"],
    });
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(history);
    const onDatasetChanged = vi.fn();
    const onProfileInvalidated = vi.fn();
    const onDeliveryInvalidated = vi.fn();
    render(<ControllerHarness {...{ onDatasetChanged, onProfileInvalidated, onDeliveryInvalidated }} />);

    fireEvent.click(screen.getByRole("button", { name: "Constantes" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(
      "Se eliminaron 2 columnas constantes: pais, segmento.",
    ));
    expect(onDatasetChanged).toHaveBeenCalledWith(dataset);
    expect(onProfileInvalidated).toHaveBeenCalledOnce();
    expect(onDeliveryInvalidated).toHaveBeenCalledOnce();
  });

  it("publica la eliminación de columnas completamente vacías", async () => {
    vi.spyOn(bridge, "removeEmptyColumns").mockResolvedValue({
      dataset,
      removedColumnCount: 1,
      removedColumns: ["notas"],
    });
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(history);
    const onDatasetChanged = vi.fn();
    const onProfileInvalidated = vi.fn();
    const onDeliveryInvalidated = vi.fn();
    render(<ControllerHarness {...{ onDatasetChanged, onProfileInvalidated, onDeliveryInvalidated }} />);

    fireEvent.click(screen.getByRole("button", { name: "Vacías" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(
      "Se eliminaron 1 columnas completamente vacías: notas.",
    ));
    expect(onDatasetChanged).toHaveBeenCalledWith(dataset);
    expect(onProfileInvalidated).toHaveBeenCalledOnce();
    expect(onDeliveryInvalidated).toHaveBeenCalledOnce();
  });
});
