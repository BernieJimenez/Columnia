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
  activeDataset = dataset,
}: {
  onDatasetChanged: (next: DatasetPreview) => void;
  onProfileInvalidated: () => void;
  onDeliveryInvalidated: () => void;
  activeDataset?: DatasetPreview | null;
}) {
  const controller = usePrepareController({
    activeDataset,
    onDatasetChanged,
    onProfileInvalidated,
    onDeliveryInvalidated,
  });
  return <>
    <button type="button" onClick={controller.applyDuplicateRemoval}>Duplicar</button>
    <button type="button" onClick={controller.applyNearDuplicateRemoval}>Parecidos</button>
    <button type="button" onClick={controller.applyEmptyRowRemoval}>Filas vacías</button>
    <button type="button" onClick={controller.applyConstantColumnRemoval}>Constantes</button>
    <button type="button" onClick={controller.applyEmptyColumnRemoval}>Vacías</button>
    <button type="button" onClick={controller.applyHighNullColumnRemoval}>Alta nulidad</button>
    <button type="button" onClick={controller.applyIdentifierColumnRemoval}>Identificadores</button>
    <button type="button" onClick={controller.applyPersonalColumnRemoval}>Personales</button>
    <button type="button" onClick={controller.applyPersonalValueMasking}>Proteger personales</button>
    <button type="button" onClick={controller.applySentinelNormalization}>Centinelas</button>
    <button type="button" onClick={controller.applyDateParsing}>Fechas</button>
    <button type="button" onClick={controller.applyBooleanNormalization}>Booleanos</button>
    <button type="button" onClick={controller.applyEncodingFix}>Codificación</button>
    <button type="button" onClick={controller.applyInvalidTypeCleanup}>Tipos incompatibles</button>
    <button type="button" onClick={controller.applyMissingValueImputation}>Imputar</button>
    <button type="button" onClick={controller.applyCategoricalImputation}>Categorías</button>
    <button type="button" onClick={controller.applyOutlierImputation}>Outliers</button>
    <button type="button" onClick={controller.applyOutlierCapping}>Capear</button>
    <button type="button" onClick={controller.applyOutlierRemoval}>Eliminar atípicos</button>
    <button type="button" onClick={controller.applyRowAudit}>Auditoría</button>
    <button type="button" onClick={controller.applyColumnNormalization}>Columnas</button>
    <button type="button" onClick={() => controller.trimText()}>Recortar</button>
    <button type="button" onClick={() => controller.normalizeText(["nombre"], true)}>Texto</button>
    <button type="button" onClick={controller.applyRecommendedCorrections}>Recomendadas</button>
    <button type="button" onClick={() => controller.applyStructuralTransforms({
      renames: [], casts: [], dateParses: [], filters: [], calculatedColumn: null,
      findReplace: null, keepColumns: null, splitColumn: null, mergeColumns: null,
      outlierTreatments: [], groupSummary: null, contactNormalizations: [], textExtractions: [],
    })}>Receta</button>
    <button type="button" onClick={controller.undoChange}>Deshacer controlador</button>
    <button type="button" onClick={controller.redoChange}>Rehacer controlador</button>
    <button type="button" onClick={controller.resetChangeStatus}>Limpiar estado</button>
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

  it("rehace un cambio y actualiza el estado del historial", async () => {
    vi.spyOn(bridge, "redoLastChange").mockResolvedValue({
      dataset,
      history: { ...history, canUndo: true, canRedo: false, currentIndex: 2 },
      message: "Se rehizo el último cambio.",
    });
    const callbacks = {
      onDatasetChanged: vi.fn(),
      onProfileInvalidated: vi.fn(),
      onDeliveryInvalidated: vi.fn(),
    };
    render(<ControllerHarness {...callbacks} />);

    fireEvent.click(screen.getByRole("button", { name: "Rehacer controlador" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Se rehizo el último cambio."));
    expect(bridge.redoLastChange).toHaveBeenCalledOnce();
    expect(callbacks.onDatasetChanged).toHaveBeenCalledWith(dataset);
    expect(callbacks.onProfileInvalidated).toHaveBeenCalledOnce();
    expect(callbacks.onDeliveryInvalidated).toHaveBeenCalledOnce();
    expect(screen.getByTestId("history-index")).toHaveTextContent("2");
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

  it("publica la máscara de valores personales con impacto agregado y reversión", async () => {
    vi.spyOn(bridge, "maskPersonalValues").mockResolvedValue({
      dataset,
      changedCellCount: 4,
      changedColumnCount: 2,
    });
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(history);
    const onDatasetChanged = vi.fn();
    const onProfileInvalidated = vi.fn();
    const onDeliveryInvalidated = vi.fn();
    render(<ControllerHarness {...{ onDatasetChanged, onProfileInvalidated, onDeliveryInvalidated }} />);

    fireEvent.click(screen.getByRole("button", { name: "Proteger personales" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(
      "Se protegieron 4 valores en 2 columnas con [REDACTED]. No se muestran nombres ni valores; el cambio puede revertirse desde el historial.",
    ));
    expect(bridge.maskPersonalValues).toHaveBeenCalledOnce();
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

  it("interpreta fechas detectadas y publica el impacto por columna", async () => {
    vi.spyOn(bridge, "parseDateValues").mockResolvedValue({
      dataset,
      affectedRowCount: 2,
      changedCellCount: 2,
      changedColumns: [{ name: "fecha", changedCellCount: 2 }],
    });
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(history);
    const onDatasetChanged = vi.fn();
    const onProfileInvalidated = vi.fn();
    const onDeliveryInvalidated = vi.fn();
    render(<ControllerHarness {...{ onDatasetChanged, onProfileInvalidated, onDeliveryInvalidated }} />);

    fireEvent.click(screen.getByRole("button", { name: "Fechas" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(
      "Se interpretaron 2 valores de fecha en: fecha. Las columnas ambiguas se dejaron intactas y el cambio puede revertirse desde el historial.",
    ));
    expect(bridge.parseDateValues).toHaveBeenCalledOnce();
    expect(onDatasetChanged).toHaveBeenCalledWith(dataset);
    expect(onProfileInvalidated).toHaveBeenCalledOnce();
    expect(onDeliveryInvalidated).toHaveBeenCalledOnce();
  });

  it("corrige doble codificación UTF-8 y publica el impacto", async () => {
    vi.spyOn(bridge, "fixEncodingValues").mockResolvedValue({
      dataset,
      affectedRowCount: 2,
      changedCellCount: 2,
      changedColumns: [{ name: "city", changedCellCount: 2 }],
    });
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(history);
    const onDatasetChanged = vi.fn();
    const onProfileInvalidated = vi.fn();
    const onDeliveryInvalidated = vi.fn();
    render(<ControllerHarness {...{ onDatasetChanged, onProfileInvalidated, onDeliveryInvalidated }} />);

    fireEvent.click(screen.getByRole("button", { name: "Codificación" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(
      "Se corrigió doble codificación UTF-8 en 2 celdas.",
    ));
    expect(onDatasetChanged).toHaveBeenCalledWith(dataset);
    expect(onProfileInvalidated).toHaveBeenCalledOnce();
    expect(onDeliveryInvalidated).toHaveBeenCalledOnce();
  });

  it("aparta valores incompatibles como nulos y publica el impacto", async () => {
    vi.spyOn(bridge, "nullifyInvalidTypeValues").mockResolvedValue({
      dataset,
      affectedRowCount: 1,
      changedCellCount: 1,
      changedColumns: [{ name: "fecha", changedCellCount: 1 }],
    });
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(history);
    const onDatasetChanged = vi.fn();
    const onProfileInvalidated = vi.fn();
    const onDeliveryInvalidated = vi.fn();
    render(<ControllerHarness {...{ onDatasetChanged, onProfileInvalidated, onDeliveryInvalidated }} />);

    fireEvent.click(screen.getByRole("button", { name: "Tipos incompatibles" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(
      "Se apartó 1 valor incompatible como nulo",
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

  it("imputa outliers por mediana y publica el impacto reversible", async () => {
    vi.spyOn(bridge, "imputeOutlierValues").mockResolvedValue({
      dataset,
      affectedRowCount: 1,
      changedCellCount: 1,
      changedColumns: [{ name: "total", changedCellCount: 1 }],
    });
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(history);
    const onDatasetChanged = vi.fn();
    const onProfileInvalidated = vi.fn();
    const onDeliveryInvalidated = vi.fn();
    render(<ControllerHarness {...{ onDatasetChanged, onProfileInvalidated, onDeliveryInvalidated }} />);

    fireEvent.click(screen.getByRole("button", { name: "Outliers" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(
      "Se reemplazaron 1 outliers por la mediana en: total. El cambio puede revertirse desde el historial.",
    ));
    expect(bridge.imputeOutlierValues).toHaveBeenCalledOnce();
    expect(onDatasetChanged).toHaveBeenCalledWith(dataset);
    expect(onProfileInvalidated).toHaveBeenCalledOnce();
    expect(onDeliveryInvalidated).toHaveBeenCalledOnce();
  });

  it("limita outliers con IQR y publica el impacto reversible", async () => {
    vi.spyOn(bridge, "capOutlierValues").mockResolvedValue({
      dataset,
      affectedRowCount: 1,
      changedCellCount: 1,
      changedColumns: [{ name: "total", changedCellCount: 1 }],
    });
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(history);
    const callbacks = {
      onDatasetChanged: vi.fn(),
      onProfileInvalidated: vi.fn(),
      onDeliveryInvalidated: vi.fn(),
    };
    render(<ControllerHarness {...callbacks} />);

    fireEvent.click(screen.getByRole("button", { name: "Capear" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(
      "Se limitaron 1 outliers a los límites IQR en: total. El cambio puede revertirse desde el historial.",
    ));
    expect(bridge.capOutlierValues).toHaveBeenCalledOnce();
    expect(callbacks.onProfileInvalidated).toHaveBeenCalledOnce();
    expect(callbacks.onDeliveryInvalidated).toHaveBeenCalledOnce();
  });

  it("elimina filas atípicas con IQR y publica el impacto reversible", async () => {
    vi.spyOn(bridge, "dropOutlierValues").mockResolvedValue({
      dataset,
      affectedRowCount: 2,
      changedCellCount: 2,
      changedColumns: [{ name: "total", changedCellCount: 2 }],
    });
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(history);
    const callbacks = {
      onDatasetChanged: vi.fn(),
      onProfileInvalidated: vi.fn(),
      onDeliveryInvalidated: vi.fn(),
    };
    render(<ControllerHarness {...callbacks} />);

    fireEvent.click(screen.getByRole("button", { name: "Eliminar atípicos" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(
      "Se eliminaron 2 filas atípicas según los límites IQR. El cambio puede revertirse desde el historial.",
    ));
    expect(bridge.dropOutlierValues).toHaveBeenCalledOnce();
    expect(callbacks.onProfileInvalidated).toHaveBeenCalledOnce();
    expect(callbacks.onDeliveryInvalidated).toHaveBeenCalledOnce();
  });

  it("completa nulos textuales como Desconocido y publica el impacto reversible", async () => {
    vi.spyOn(bridge, "imputeCategoricalValues").mockResolvedValue({
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

    fireEvent.click(screen.getByRole("button", { name: "Categorías" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(
      "Se completaron 2 nulos textuales como Desconocido en: estado. El cambio puede revertirse desde el historial.",
    ));
    expect(bridge.imputeCategoricalValues).toHaveBeenCalledOnce();
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

  it("cubre las rutas sin cambios de las correcciones y transformaciones", async () => {
    vi.spyOn(bridge, "getHistoryState").mockResolvedValue(history);
    vi.spyOn(bridge, "removeDuplicates").mockResolvedValue({ dataset, affectedRowCount: 0 });
    vi.spyOn(bridge, "removeNearDuplicates").mockResolvedValue({ dataset, affectedRowCount: 0 });
    vi.spyOn(bridge, "removeEmptyRows").mockResolvedValue({ dataset, affectedRowCount: 0 });
    vi.spyOn(bridge, "removeConstantColumns").mockResolvedValue({ dataset, removedColumnCount: 0, removedColumns: [] });
    vi.spyOn(bridge, "removeEmptyColumns").mockResolvedValue({ dataset, removedColumnCount: 0, removedColumns: [] });
    vi.spyOn(bridge, "removeHighNullColumns").mockResolvedValue({ dataset, removedColumnCount: 0, removedColumns: [] });
    vi.spyOn(bridge, "removeIdentifierColumns").mockResolvedValue({ dataset, removedColumnCount: 0, removedColumns: [] });
    vi.spyOn(bridge, "removePersonalColumns").mockResolvedValue({ dataset, removedColumnCount: 0, removedColumns: [] });
    vi.spyOn(bridge, "normalizeSentinelValues").mockResolvedValue({
      dataset, affectedRowCount: 0, changedCellCount: 0, changedColumns: [],
    });
    vi.spyOn(bridge, "normalizeBooleanValues").mockResolvedValue({
      dataset, affectedRowCount: 0, changedCellCount: 0, changedColumns: [],
    });
    vi.spyOn(bridge, "imputeMissingValues").mockResolvedValue({
      dataset, affectedRowCount: 0, changedCellCount: 0, changedColumns: [],
    });
    vi.spyOn(bridge, "enableRowAudit").mockResolvedValue({ dataset, affectedRowCount: 0 });
    vi.spyOn(bridge, "normalizeColumnNames").mockResolvedValue({ dataset, renamedColumnCount: 0, renames: [] });
    vi.spyOn(bridge, "trimTextValues").mockResolvedValue({
      dataset, affectedRowCount: 0, changedCellCount: 0, changedColumns: [],
    });
    vi.spyOn(bridge, "normalizeTextValues").mockResolvedValue({
      dataset, affectedRowCount: 0, changedCellCount: 0, changedColumns: [],
    });
    vi.spyOn(bridge, "applySafeCorrections").mockResolvedValue({
      dataset, changedCellCount: 0, affectedRowCount: 0, renamedColumnCount: 0, renames: [],
    });
    vi.spyOn(bridge, "applyTransformRecipe").mockResolvedValue({
      dataset, changed: false, renamedColumnCount: 0, convertedColumnCount: 0,
      parsedDateColumnCount: 0, removedRowCount: 0, calculatedColumnCount: 0,
      replacedCellCount: 0, droppedColumnCount: 0, splitColumnCount: 0, mergedColumnCount: 0,
      droppedSourceColumnCount: 0, adjustedOutlierCellCount: 0, outlierRemovedRowCount: 0,
      outlierColumnCount: 0, groupCount: 0, aggregatedColumnCount: 0, collapsedRowCount: 0,
      normalizedContactCellCount: 0, normalizedContactColumnCount: 0, extractedColumnCount: 0,
    });
    const callbacks = {
      onDatasetChanged: vi.fn(), onProfileInvalidated: vi.fn(), onDeliveryInvalidated: vi.fn(),
    };
    render(<ControllerHarness {...callbacks} />);

    fireEvent.click(screen.getByRole("button", { name: "Duplicar" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Se eliminaron 0 filas duplicadas"));
    fireEvent.click(screen.getByRole("button", { name: "Parecidos" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("No se detectaron duplicados parecidos"));
    fireEvent.click(screen.getByRole("button", { name: "Filas vacías" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("No se detectaron filas"));
    fireEvent.click(screen.getByRole("button", { name: "Constantes" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("No se eliminaron columnas constantes"));
    fireEvent.click(screen.getByRole("button", { name: "Vacías" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("No se eliminaron columnas vacías"));
    fireEvent.click(screen.getByRole("button", { name: "Alta nulidad" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("No se detectaron columnas con al menos 80%"));
    fireEvent.click(screen.getByRole("button", { name: "Identificadores" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("No se detectaron columnas identificadoras"));
    fireEvent.click(screen.getByRole("button", { name: "Personales" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("No se detectaron columnas de datos personales"));
    fireEvent.click(screen.getByRole("button", { name: "Centinelas" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("No se detectaron valores centinela"));
    fireEvent.click(screen.getByRole("button", { name: "Booleanos" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("No se encontraron alias booleanos"));
    fireEvent.click(screen.getByRole("button", { name: "Imputar" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("No se encontraron nulos imputables"));
    fireEvent.click(screen.getByRole("button", { name: "Auditoría" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("no produjo cambios"));
    fireEvent.click(screen.getByRole("button", { name: "Columnas" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("ya estaban normalizados"));
    fireEvent.click(screen.getByRole("button", { name: "Recortar" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("No se encontraron valores"));
    fireEvent.click(screen.getByRole("button", { name: "Texto" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("No se encontraron valores"));
    fireEvent.click(screen.getByRole("button", { name: "Recomendadas" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("ya cumplía"));
    fireEvent.click(screen.getByRole("button", { name: "Receta" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("no produjo cambios"));
  });

  it("maneja errores, refresco de historial fallido y dataset ausente sin ejecutar IPC", async () => {
    const removeRows = vi.spyOn(bridge, "removeEmptyRows").mockRejectedValue(new Error("fallo controlado"));
    vi.spyOn(bridge, "getHistoryState").mockRejectedValue(new Error("historial no disponible"));
    const callbacks = {
      onDatasetChanged: vi.fn(), onProfileInvalidated: vi.fn(), onDeliveryInvalidated: vi.fn(),
    };
    render(<ControllerHarness {...callbacks} activeDataset={null} />);
    fireEvent.click(screen.getByRole("button", { name: "Filas vacías" }));
    expect(removeRows).not.toHaveBeenCalled();

    cleanup();
    render(<ControllerHarness {...callbacks} />);
    fireEvent.click(screen.getByRole("button", { name: "Filas vacías" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("error"));
    fireEvent.click(screen.getByRole("button", { name: "Limpiar estado" }));
    expect(screen.getByRole("status")).toHaveTextContent("idle");
  });

  it("ignora todas las mutaciones y cambios de historial sin dataset activo", () => {
    const callbacks = {
      onDatasetChanged: vi.fn(),
      onProfileInvalidated: vi.fn(),
      onDeliveryInvalidated: vi.fn(),
    };
    render(<ControllerHarness {...callbacks} activeDataset={null} />);

    for (const name of [
      "Duplicar",
      "Parecidos",
      "Filas vacías",
      "Constantes",
      "Vacías",
      "Alta nulidad",
      "Identificadores",
      "Personales",
      "Centinelas",
      "Booleanos",
      "Imputar",
      "Auditoría",
      "Columnas",
      "Recortar",
      "Texto",
      "Recomendadas",
      "Receta",
      "Deshacer controlador",
      "Rehacer controlador",
    ]) {
      fireEvent.click(screen.getByRole("button", { name }));
    }

    expect(callbacks.onDatasetChanged).not.toHaveBeenCalled();
    expect(callbacks.onProfileInvalidated).not.toHaveBeenCalled();
    expect(callbacks.onDeliveryInvalidated).not.toHaveBeenCalled();
  });
});
