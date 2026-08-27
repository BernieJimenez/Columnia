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
    <button type="button" onClick={controller.applyNearDuplicateRemoval}>Parecidos</button>
    <button type="button" onClick={controller.applyConstantColumnRemoval}>Constantes</button>
    <button type="button" onClick={controller.applyEmptyColumnRemoval}>Vacías</button>
    <button type="button" onClick={controller.applyHighNullColumnRemoval}>Alta nulidad</button>
    <button type="button" onClick={controller.applyIdentifierColumnRemoval}>Identificadores</button>
    <button type="button" onClick={controller.applyPersonalColumnRemoval}>Personales</button>
    <button type="button" onClick={controller.applySentinelNormalization}>Centinelas</button>
    <button type="button" onClick={controller.applyBooleanNormalization}>Booleanos</button>
    <button type="button" onClick={controller.applyMissingValueImputation}>Imputar</button>
    <button type="button" onClick={controller.applyRowAudit}>Auditoría</button>
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

  it("publica la eliminación de parecidos y refresca el historial", async () => {
    vi.spyOn(bridge, "removeNearDuplicates").mockResolvedValue({ dataset, affectedRowCount: 1 });
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(history);
    const onDatasetChanged = vi.fn();
    const onProfileInvalidated = vi.fn();
    const onDeliveryInvalidated = vi.fn();
    render(<ControllerHarness {...{ onDatasetChanged, onProfileInvalidated, onDeliveryInvalidated }} />);

    fireEvent.click(screen.getByRole("button", { name: "Parecidos" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(
      "Se eliminaron 1 filas duplicadas parecidas.",
    ));
    expect(bridge.removeNearDuplicates).toHaveBeenCalledOnce();
    expect(onDatasetChanged).toHaveBeenCalledWith(dataset);
    expect(onProfileInvalidated).toHaveBeenCalledOnce();
    expect(onDeliveryInvalidated).toHaveBeenCalledOnce();
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

  it("publica la eliminación de columnas con alta nulidad", async () => {
    vi.spyOn(bridge, "removeHighNullColumns").mockResolvedValue({
      dataset,
      removedColumnCount: 1,
      removedColumns: ["comentarios"],
    });
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(history);
    const onDatasetChanged = vi.fn();
    const onProfileInvalidated = vi.fn();
    const onDeliveryInvalidated = vi.fn();
    render(<ControllerHarness {...{ onDatasetChanged, onProfileInvalidated, onDeliveryInvalidated }} />);

    fireEvent.click(screen.getByRole("button", { name: "Alta nulidad" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(
      "Se eliminaron 1 columnas con alta nulidad: comentarios.",
    ));
    expect(onDatasetChanged).toHaveBeenCalledWith(dataset);
    expect(onProfileInvalidated).toHaveBeenCalledOnce();
    expect(onDeliveryInvalidated).toHaveBeenCalledOnce();
  });

  it("publica la eliminación de columnas identificadoras y refresca historial", async () => {
    vi.spyOn(bridge, "removeIdentifierColumns").mockResolvedValue({
      dataset,
      removedColumnCount: 1,
      removedColumns: ["customer_id"],
    });
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(history);
    const onDatasetChanged = vi.fn();
    const onProfileInvalidated = vi.fn();
    const onDeliveryInvalidated = vi.fn();
    render(<ControllerHarness {...{ onDatasetChanged, onProfileInvalidated, onDeliveryInvalidated }} />);

    fireEvent.click(screen.getByRole("button", { name: "Identificadores" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(
      "Se retiraron 1 columnas identificadoras: customer_id.",
    ));
    expect(bridge.removeIdentifierColumns).toHaveBeenCalledOnce();
    expect(onDatasetChanged).toHaveBeenCalledWith(dataset);
    expect(onProfileInvalidated).toHaveBeenCalledOnce();
    expect(onDeliveryInvalidated).toHaveBeenCalledOnce();
  });

  it("publica la eliminación de columnas personales sin exponer sus nombres", async () => {
    vi.spyOn(bridge, "removePersonalColumns").mockResolvedValue({
      dataset,
      removedColumnCount: 3,
      removedColumns: ["email", "phone", "nombre"],
    });
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(history);
    const onDatasetChanged = vi.fn();
    const onProfileInvalidated = vi.fn();
    const onDeliveryInvalidated = vi.fn();
    render(<ControllerHarness {...{ onDatasetChanged, onProfileInvalidated, onDeliveryInvalidated }} />);

    fireEvent.click(screen.getByRole("button", { name: "Personales" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(
      "Se retiraron 3 columnas de datos personales.",
    ));
    expect(screen.getByRole("status")).not.toHaveTextContent("email");
    expect(bridge.removePersonalColumns).toHaveBeenCalledOnce();
    expect(onDatasetChanged).toHaveBeenCalledWith(dataset);
    expect(onProfileInvalidated).toHaveBeenCalledOnce();
    expect(onDeliveryInvalidated).toHaveBeenCalledOnce();
  });

  it("convierte centinelas a nulos y publica el impacto por columna", async () => {
    vi.spyOn(bridge, "normalizeSentinelValues").mockResolvedValue({
      dataset,
      affectedRowCount: 2,
      changedCellCount: 3,
      changedColumns: [{ name: "estado", changedCellCount: 3 }],
    });
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(history);
    const onDatasetChanged = vi.fn();
    const onProfileInvalidated = vi.fn();
    const onDeliveryInvalidated = vi.fn();
    render(<ControllerHarness {...{ onDatasetChanged, onProfileInvalidated, onDeliveryInvalidated }} />);

    fireEvent.click(screen.getByRole("button", { name: "Centinelas" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(
      "Se convirtieron 3 valores centinela a nulos en: estado.",
    ));
    expect(onDatasetChanged).toHaveBeenCalledWith(dataset);
    expect(onProfileInvalidated).toHaveBeenCalledOnce();
    expect(onDeliveryInvalidated).toHaveBeenCalledOnce();
  });

  it("normaliza alias booleanos y publica el impacto por columna", async () => {
    vi.spyOn(bridge, "normalizeBooleanValues").mockResolvedValue({
      dataset,
      affectedRowCount: 2,
      changedCellCount: 2,
      changedColumns: [{ name: "activo", changedCellCount: 2 }],
    });
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(history);
    const onDatasetChanged = vi.fn();
    const onProfileInvalidated = vi.fn();
    const onDeliveryInvalidated = vi.fn();
    render(<ControllerHarness {...{ onDatasetChanged, onProfileInvalidated, onDeliveryInvalidated }} />);

    fireEvent.click(screen.getByRole("button", { name: "Booleanos" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(
      "Se normalizaron 2 valores booleanos en: activo.",
    ));
    expect(onDatasetChanged).toHaveBeenCalledWith(dataset);
    expect(onProfileInvalidated).toHaveBeenCalledOnce();
    expect(onDeliveryInvalidated).toHaveBeenCalledOnce();
  });

  it("imputa nulos de forma conservadora y publica el impacto", async () => {
    vi.spyOn(bridge, "imputeMissingValues").mockResolvedValue({
      dataset,
      affectedRowCount: 2,
      changedCellCount: 2,
      changedColumns: [{ name: "estado", changedCellCount: 2 }],
    });
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(history);
    const onDatasetChanged = vi.fn();
    const onProfileInvalidated = vi.fn();
    const onDeliveryInvalidated = vi.fn();
    render(<ControllerHarness {...{ onDatasetChanged, onProfileInvalidated, onDeliveryInvalidated }} />);

    fireEvent.click(screen.getByRole("button", { name: "Imputar" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(
      "Se imputaron 2 valores nulos en: estado.",
    ));
    expect(onDatasetChanged).toHaveBeenCalledWith(dataset);
    expect(onProfileInvalidated).toHaveBeenCalledOnce();
    expect(onDeliveryInvalidated).toHaveBeenCalledOnce();
  });

  it("activa la trazabilidad por fila y refresca historial", async () => {
    const auditedDataset = {
      ...dataset,
      columns: [...dataset.columns, { name: "_cambios", dataType: "String" }],
    };
    vi.spyOn(bridge, "enableRowAudit").mockResolvedValue({
      dataset: auditedDataset,
      affectedRowCount: 0,
    });
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(history);
    const onDatasetChanged = vi.fn();
    const onProfileInvalidated = vi.fn();
    const onDeliveryInvalidated = vi.fn();
    render(<ControllerHarness {...{ onDatasetChanged, onProfileInvalidated, onDeliveryInvalidated }} />);

    fireEvent.click(screen.getByRole("button", { name: "Auditoría" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(
      "La trazabilidad por fila está activa; los cambios futuros se anotarán en _cambios.",
    ));
    expect(onDatasetChanged).toHaveBeenCalledWith(auditedDataset);
    expect(onProfileInvalidated).toHaveBeenCalledOnce();
    expect(onDeliveryInvalidated).toHaveBeenCalledOnce();
  });
});
