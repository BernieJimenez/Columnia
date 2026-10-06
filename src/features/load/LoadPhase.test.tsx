import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { localeText } from "../../test/localeText";

import type { DatasetSourceInspection } from "../../bridge";
import type { SampleDatasetDescriptor } from "../../bridge";
import { LoadPhase } from "./LoadPhase";
import {
  completeDelimitedHeaderReview,
  completeSchemaPreview,
  delimitedHeaderInspection,
  LEGACY_ENCODING_PREFIX,
  workbookInspection,
} from "./loadModel";
import type { RecentDataset } from "./recentFilesModel";

afterEach(cleanup);

const workbook: DatasetSourceInspection = {
  selectionId: "opaque-selection",
  fileName: "libro.xlsx",
  fileSizeBytes: 2048,
  format: "excel",
  isCompressedContainer: true,
  defaultSheetId: "sheet-1",
  sheets: [
    { id: "sheet-1", name: "Enero" },
    { id: "sheet-2", name: "Febrero" },
  ],
  resourceEstimate: {
    processingPath: "inMemory",
    estimatedMaterializationRamBytes: 256 * 1024 * 1024 + 8192,
    estimatedTemporaryDiskBytes: 2048,
  },
};

const recentDataset: RecentDataset = {
  id: "recent-ventas",
  fileName: "ventas.csv",
  format: "csv",
  lastOpenedAt: 1_724_640_000_000,
};

const delimitedSource: DatasetSourceInspection = {
  ...workbook,
  selectionId: "csv-selection",
  fileName: "ventas.csv",
  format: "csv",
  fileSizeBytes: 128,
  isCompressedContainer: false,
  defaultSheetId: null,
  sheets: [],
};

const delimitedHeaderReview = {
  delimiter: ";",
  firstRow: {
    headerMode: "firstRow" as const,
    columns: [{ name: "id", dataType: "String" }],
    rows: [["1"]],
    includesFirstRow: false,
    sampleTruncated: false,
  },
  generated: {
    headerMode: "generated" as const,
    columns: [{ name: "column_1", dataType: "String" }],
    rows: [["id"], ["1"]],
    includesFirstRow: true,
    sampleTruncated: false,
  },
};

function loadPhaseProps(overrides: Partial<React.ComponentProps<typeof LoadPhase>> = {}) {
  return {
    runtime: { kind: "connected" as const },
    datasetStatus: { kind: "empty" as const },
    inspection: { kind: "idle" as const },
    recentDatasets: [],
    onSelect: () => undefined,
    onSelectRecent: () => undefined,
    onClearRecent: () => undefined,
    onRemoveRecent: () => undefined,
    onSheetAction: () => undefined,
    onCancelLoad: () => undefined,
    ...overrides,
  };
}

describe("LoadPhase", () => {
  it("mantiene el panel de tareas en su propio disclosure, fuera del de proyectos", () => {
    render(
      <LoadPhase
        {...loadPhaseProps({
          children: <p>Contenido de proyectos</p>,
          reusableTaskPanel: (
            <details aria-label="Tareas reutilizables">
              <summary>Reutilizar una tarea</summary>
              <p>Configuración local guardada</p>
            </details>
          ),
        })}
      />,
    );

    const tasksPanel = screen.getByText("Reutilizar una tarea").closest("details");
    const projectsPanel = screen.getByText("Continuar un proyecto").closest("details");
    expect(tasksPanel).not.toBeNull();
    expect(tasksPanel?.open).toBe(false);
    expect(projectsPanel).not.toContainElement(tasksPanel);
    expect(screen.getByText("Configuración local guardada")).toBeInTheDocument();
  });

  it("muestra una muestra acotada y bloquea la carga delimitada hasta revisar encabezados", () => {
    const onSheetAction = vi.fn();
    const previousDataset = {
      kind: "ready" as const,
      dataset: {
        fileName: "anterior.csv",
        fileSizeBytes: 10,
        rowCount: 1,
        columnCount: 1,
        columns: [{ name: "id", dataType: "String" }],
        rows: [["anterior"]],
      },
      pageOffset: 0,
      pageLoading: false,
    };
    const pendingInspection = delimitedHeaderInspection(delimitedSource);
    const { rerender } = render(
      <LoadPhase
        {...loadPhaseProps({ onSheetAction })}
        datasetStatus={previousDataset}
        inspection={pendingInspection}
      />,
    );

    expect(screen.getByRole("dialog", { name: "Revisar encabezados de ventas.csv" })).toBeInTheDocument();
    expect(screen.getByText("Preparando una muestra local de hasta 64 KiB…")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Revisar esquema" })).toBeDisabled();
    expect(screen.getByRole("heading", { name: "anterior.csv" })).toBeInTheDocument();

    const readyInspection = completeDelimitedHeaderReview(pendingInspection, delimitedHeaderReview);
    rerender(
      <LoadPhase
        {...loadPhaseProps({ onSheetAction })}
        datasetStatus={previousDataset}
        inspection={readyInspection}
      />,
    );
    const interpretation = screen.getByRole("region", { name: "Vista previa de la interpretación" });
    expect(within(interpretation).getByLabelText("Muestra importada")).toBeInTheDocument();
    expect(screen.getByText("Separador detectado").parentElement).toHaveTextContent("“;”");

    fireEvent.click(screen.getByRole("radio", { name: /Conservar la primera fila como datos/ }));
    expect(onSheetAction).toHaveBeenCalledWith({ kind: "header_mode_changed", headerMode: "generated" });
    fireEvent.click(screen.getByRole("button", { name: "Revisar esquema" }));
    expect(onSheetAction).toHaveBeenCalledWith({ kind: "confirmed" });
    const reviewedInspection = completeSchemaPreview(readyInspection, {
      rowCount: 2,
      columns: delimitedHeaderReview.generated.columns,
      schemaMismatch: null,
    });
    rerender(
      <LoadPhase
        {...loadPhaseProps({ onSheetAction })}
        datasetStatus={previousDataset}
        inspection={reviewedInspection}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Cargar archivo" }));
    expect(onSheetAction).toHaveBeenCalledTimes(3);
    expect(onSheetAction).toHaveBeenLastCalledWith({ kind: "confirmed" });
    expect(screen.getByRole("heading", { name: "anterior.csv" })).toBeInTheDocument();
  });

  it("permite volver a leer el archivo con otro separador o codificación (PROD-02)", () => {
    const onReinterpret = vi.fn();
    const readyInspection = completeDelimitedHeaderReview(
      delimitedHeaderInspection(delimitedSource),
      delimitedHeaderReview,
    );
    render(<LoadPhase {...loadPhaseProps({})} inspection={readyInspection} onReinterpret={onReinterpret} />);

    fireEvent.click(screen.getByText("¿Columnas o acentos mal leídos?"));
    const reread = screen.getByRole("button", { name: "Volver a leer" });
    expect(reread).toBeDisabled();
    fireEvent.change(screen.getByRole("combobox", { name: "Separador" }), { target: { value: "|" } });
    fireEvent.change(screen.getByRole("combobox", { name: "Codificación" }), { target: { value: "windows-1252" } });
    fireEvent.click(reread);
    expect(onReinterpret).toHaveBeenCalledWith("|", "windows-1252");

    fireEvent.change(screen.getByRole("combobox", { name: "Separador" }), { target: { value: "" } });
    fireEvent.click(reread);
    expect(onReinterpret).toHaveBeenLastCalledWith(null, "windows-1252");
  });

  it("expone el diálogo accesible y emite acciones nominales para la hoja", () => {
    const onSheetAction = vi.fn();
    render(
      <LoadPhase
        {...loadPhaseProps({ onSheetAction })}
        inspection={workbookInspection(workbook)}
      />,
    );

    expect(screen.getByRole("heading", { name: "Trae tus datos a un espacio de trabajo local." })).toBeInTheDocument();
    expect(screen.getByText(/los archivos grandes se leen por bloques/)).toBeInTheDocument();
    expect(screen.getByRole("dialog", { name: "Elegir hoja de libro.xlsx" })).toHaveAttribute(
      "aria-describedby",
      "sheet-description",
    );
    const summary = within(screen.getByRole("region", { name: "Resumen antes de cargar" }));
    expect(summary.getByText(/2[.,]0 KiB/)).toBeInTheDocument();
    expect(summary.getByText("Enero")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Usar la primera fila como encabezados" })).toBeChecked();
    expect(screen.getByText(/esquema se calcula .* antes de activar el dataset/)).toBeInTheDocument();
    expect(screen.getByText(/pueden ocupar bastante más memoria/)).toBeInTheDocument();
    const resourceSummary = screen.getByRole("region", { name: "Estimación de recursos" });
    expect(resourceSummary).toHaveTextContent("Aprox. 256 MiB (4× archivo + 256 MiB)");
    expect(within(resourceSummary).getByText(/Snapshot e historial inicial/)).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Hoja" })).toHaveFocus();
    fireEvent.change(screen.getByRole("combobox", { name: "Hoja" }), {
      target: { value: "sheet-2" },
    });
    fireEvent.click(screen.getByRole("radio", { name: "Generar encabezados (column_1, column_2…)" }));
    fireEvent.click(screen.getByRole("button", { name: "Revisar esquema" }));

    expect(onSheetAction).toHaveBeenNthCalledWith(1, { kind: "sheet_changed", sheetId: "sheet-2" });
    expect(onSheetAction).toHaveBeenNthCalledWith(2, {
      kind: "header_mode_changed",
      headerMode: "generated",
    });
    expect(onSheetAction).toHaveBeenNthCalledWith(3, { kind: "confirmed" });
  });

  it("conserva el dataset anterior visible mientras la carga se puede cancelar", () => {
    const onCancelLoad = vi.fn();
    render(
      <LoadPhase
        {...loadPhaseProps({ onCancelLoad })}
        datasetStatus={{
          kind: "loading",
          progress: { operation: "load", stage: "Leyendo filas", percent: 25 },
          cancelRequested: false,
          previous: {
            kind: "ready",
            dataset: {
              fileName: "anterior.csv",
              fileSizeBytes: 10,
              rowCount: 1,
              columnCount: 1,
              columns: [{ name: "id", dataType: "String" }],
              rows: [["1"]],
            },
            pageOffset: 0,
            pageLoading: false,
          },
        }}
        inspection={{ kind: "inspecting" }}
      />,
    );

    expect(screen.getByRole("heading", { name: "anterior.csv" })).toBeInTheDocument();
    expect(screen.getByLabelText("Progreso: Leyendo filas")).toHaveValue(25);
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(onCancelLoad).toHaveBeenCalledOnce();
  });

  it("cancelar una discrepancia conserva el dataset activo y descarta la selección pendiente", () => {
    const onSchemaMismatchAction = vi.fn();
    render(
      <LoadPhase
        {...loadPhaseProps({ onSchemaMismatchAction })}
        datasetStatus={{
          kind: "ready",
          dataset: {
            fileName: "anterior.csv",
            fileSizeBytes: 10,
            rowCount: 1,
            columnCount: 1,
            columns: [{ name: "id", dataType: "String" }],
            rows: [["1"]],
          },
          pageOffset: 0,
          pageLoading: false,
        }}
        inspection={{
          kind: "schema_mismatch",
          source: workbook,
          profile: {
            version: 1,
            format: "excel",
            sheetName: "Enero",
            headerMode: "firstRow",
            schema: [{ name: "id", dataType: "String" }],
          },
          mismatch: {
            code: "importProfileSchemaMismatch",
            missingColumns: ["id"],
            addedColumns: ["id_actual"],
            changedTypes: [],
          },
          sheetId: "sheet-1",
          headerMode: "firstRow",
        }}
      />,
    );

    expect(screen.getByRole("heading", { name: "anterior.csv" })).toBeInTheDocument();
    expect(screen.getByRole("alertdialog", { name: "El esquema difiere del perfil guardado" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Cancelar y conservar dataset" }));
    expect(onSchemaMismatchAction).toHaveBeenCalledWith({ kind: "cancelled" });
    expect(screen.getByRole("heading", { name: "anterior.csv" })).toBeInTheDocument();
  });

  it("ofrece volver a elegir desde el historial y permite limpiarlo", () => {
    const onSelectRecent = vi.fn();
    const onClearRecent = vi.fn();
    const onRemoveRecent = vi.fn();
    render(
      <LoadPhase
        {...loadPhaseProps({
          recentDatasets: [recentDataset],
          onSelectRecent,
          onClearRecent,
          onRemoveRecent,
        })}
      />,
    );

    expect(screen.getByRole("heading", { name: "Archivos recientes" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Elegir de nuevo" }));
    fireEvent.click(screen.getByRole("button", { name: "Quitar ventas.csv del historial" }));
    fireEvent.click(screen.getByRole("button", { name: "Limpiar historial" }));

    expect(onSelectRecent).toHaveBeenCalledWith(recentDataset);
    expect(onRemoveRecent).toHaveBeenCalledWith("recent-ventas");
    expect(onClearRecent).toHaveBeenCalledOnce();
  });

  it("ofrece datasets de ejemplo locales mediante identificadores opacos", () => {
    const onSelectSample = vi.fn();
    const samples: SampleDatasetDescriptor[] = [
      {
        id: "quality",
        name: "Clientes · señales de calidad",
        format: "csv",
        description: "Nulos e identificadores.",
      },
    ];
    render(
      <LoadPhase
        {...loadPhaseProps({ sampleDatasets: samples, onSelectSample })}
      />,
    );

    expect(screen.getByRole("heading", { name: "Explora con un dataset de ejemplo" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Clientes · señales de calidad/ }));
    expect(onSelectSample).toHaveBeenCalledWith("quality");
  });

  it("deshabilita volver a elegir fuera de Tauri, pero conserva el historial visible", () => {
    render(
      <LoadPhase
        {...loadPhaseProps({
          runtime: { kind: "browser" },
          recentDatasets: [recentDataset],
        })}
      />,
    );

    expect(screen.getByRole("button", { name: "Elegir de nuevo" })).toBeDisabled();
    expect(screen.getByText("ventas.csv")).toBeInTheDocument();
    });
  });

  it("actualiza el resumen cuando cambian la hoja y el modo de encabezados", () => {
    const initialInspection = workbookInspection(workbook);
    if (initialInspection.kind !== "sheet") throw new Error("Se esperaba seleccionar una hoja.");

    const { rerender } = render(
      <LoadPhase {...loadPhaseProps()} inspection={initialInspection} />,
    );

    rerender(
      <LoadPhase
        {...loadPhaseProps()}
        inspection={{
          ...initialInspection,
          selectedSheetId: "sheet-2",
          headerMode: "generated",
        }}
      />,
    );

    const summary = within(screen.getByRole("region", { name: "Resumen antes de cargar" }));
    expect(summary.getByText("Febrero")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Generar encabezados (column_1, column_2…)" })).toBeChecked();
  });

  it("pide confirmación nominal antes de cargar una fuente costosa", () => {
    const onResourcePreflightAction = vi.fn();
    const source: DatasetSourceInspection = {
      ...workbook,
      format: "csv",
      fileName: "clientes-grande.csv",
      fileSizeBytes: 512 * 1024 * 1024,
      sheets: [],
      defaultSheetId: null,
      resourceEstimate: {
        processingPath: "sourceBacked",
        estimatedMaterializationRamBytes: 2.25 * 1024 * 1024 * 1024,
        estimatedTemporaryDiskBytes: null,
      },
    };
    render(
      <LoadPhase
        {...loadPhaseProps({ onResourcePreflightAction })}
        inspection={{ kind: "resource_preflight", source }}
      />,
    );

    const dialog = screen.getByRole("dialog", { name: "Revisa el costo estimado de la carga" });
    expect(within(dialog).getByText(/Lectura source-backed/)).toBeInTheDocument();
    expect(dialog).toHaveTextContent(localeText("Aprox. 2.3 GiB"));
    expect(within(dialog).getByText(/No se prevé un snapshot/)).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "Continuar con la carga" }));
    expect(onResourcePreflightAction).toHaveBeenCalledWith({ kind: "confirmed" });
  });

describe("LoadPhase: ramas de error, cancelación, codificación y perfil (QA-12)", () => {
  const profile = {
    version: 1 as const,
    format: "csv" as const,
    headerMode: "firstRow" as const,
    schema: [{ name: "id", dataType: "String" }],
  };

  it("explica el cierre inesperado según el último proyecto guardado", () => {
    const onDismissPreviousExit = vi.fn();
    const view = render(<LoadPhase {...loadPhaseProps({ previousExitUnclean: true, onDismissPreviousExit })} lastSavedProject={undefined} />);
    expect(screen.getByRole("heading", { name: "La sesión anterior se cerró de forma inesperada" })).toBeInTheDocument();
    expect(screen.getByText(/Si guardaste uno, puedes recuperarlo/)).toBeInTheDocument();
    view.rerender(<LoadPhase {...loadPhaseProps({ previousExitUnclean: true, onDismissPreviousExit })} lastSavedProject={null} />);
    expect(screen.getByText(/No había ningún proyecto guardado/)).toBeInTheDocument();
    view.rerender(<LoadPhase
      {...loadPhaseProps({ previousExitUnclean: true, onDismissPreviousExit })}
      lastSavedProject={{ name: "Ventas", updatedAt: "2026-10-02T10:00:00Z" }}
    />);
    expect(screen.getByText(/Último guardado: .*\(Ventas\)/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Entendido" }));
    expect(onDismissPreviousExit).toHaveBeenCalledOnce();
  });

  it("avisa de un catálogo apartado y dice dónde quedó (ARQ-02)", () => {
    const onDismissSetAsideCatalogs = vi.fn();
    render(<LoadPhase
      {...loadPhaseProps({ onDismissSetAsideCatalogs })}
      setAsideCatalogs={["C:/datos/delivery-presets.sqlite3.unreadable-1"]}
    />);
    expect(screen.getByRole("heading", { name: "Se apartó un catálogo que no se podía abrir" })).toBeInTheDocument();
    expect(screen.getByText("C:/datos/delivery-presets.sqlite3.unreadable-1")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Entendido" }));
    expect(onDismissSetAsideCatalogs).toHaveBeenCalledOnce();
  });

  it("deja cancelar la inspección de un libro y reintentar una cancelación fallida", () => {
    const onCancelWorkbookInspection = vi.fn();
    const onRetrySelectionCancellation = vi.fn();
    const view = render(<LoadPhase
      {...loadPhaseProps({ onCancelWorkbookInspection, onRetrySelectionCancellation })}
      inspection={{ kind: "workbook_inspecting", source: workbook }}
    />);
    fireEvent.click(screen.getByRole("button", { name: "Cancelar inspección" }));
    expect(onCancelWorkbookInspection).toHaveBeenCalledOnce();
    view.rerender(<LoadPhase
      {...loadPhaseProps({ onCancelWorkbookInspection, onRetrySelectionCancellation })}
      inspection={{ kind: "workbook_inspecting", source: workbook }}
      workbookInspectionCancellationPending
    />);
    expect(screen.getByRole("button", { name: "Cancelando inspección…" })).toBeDisabled();

    view.rerender(<LoadPhase {...loadPhaseProps()} inspection={{ kind: "selection_cancelling", source: workbook }} />);
    expect(screen.getByText(/Cancelando la selección de/)).toBeInTheDocument();

    view.rerender(<LoadPhase
      {...loadPhaseProps({ onRetrySelectionCancellation })}
      inspection={{ kind: "selection_cancellation_failed", source: workbook, message: "La lectura sigue activa.", cancelPending: false, discardPending: false }}
    />);
    expect(screen.getByRole("alert")).toHaveTextContent("La lectura sigue activa.");
    fireEvent.click(screen.getByRole("button", { name: "Reintentar cancelación" }));
    expect(onRetrySelectionCancellation).toHaveBeenCalledOnce();
  });

  it("muestra los errores de carga y de importación", () => {
    const view = render(<LoadPhase {...loadPhaseProps()} datasetStatus={{ kind: "error", message: "archivo dañado" }} />);
    expect(screen.getByRole("alert")).toHaveTextContent("No se pudo cargar el archivo: archivo dañado");
    view.rerender(<LoadPhase {...loadPhaseProps()} inspection={{ kind: "error", message: "formato desconocido" }} />);
    expect(screen.getByRole("alert")).toHaveTextContent("No se pudo importar el archivo: formato desconocido");
  });

  it("ofrece reutilizar, ignorar o cancelar el perfil guardado", () => {
    const onProfileReviewAction = vi.fn();
    render(<LoadPhase
      {...loadPhaseProps({ onProfileReviewAction })}
      pendingTaskName="Ventas mensuales"
      inspection={{ kind: "profile_review", source: delimitedSource, profile }}
    />);
    const dialog = screen.getByRole("dialog", { name: "Reutilizar interpretación guardada" });
    expect(dialog).toHaveTextContent("1 columnas de esquema");
    expect(dialog).toHaveTextContent("Ventas mensuales");
    fireEvent.click(within(dialog).getByRole("button", { name: "Usar perfil y revisar esquema" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Importar sin perfil" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancelar" }));
    fireEvent.keyDown(dialog, { key: "Escape" });
    expect(onProfileReviewAction.mock.calls.map(([action]) => action.kind)).toEqual([
      "use_profile", "use_defaults", "cancelled", "cancelled",
    ]);
  });

  it("cierra el preflight de recursos con Esc", () => {
    const onResourcePreflightAction = vi.fn();
    render(<LoadPhase {...loadPhaseProps({ onResourcePreflightAction })} inspection={{ kind: "resource_preflight", source: delimitedSource }} />);
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(onResourcePreflightAction).toHaveBeenCalledWith({ kind: "cancelled" });
  });

  it("resume más de doce diferencias de esquema y deja importar o conservar", () => {
    const onSchemaMismatchAction = vi.fn();
    const mismatch = {
      code: "importProfileSchemaMismatch" as const,
      missingColumns: Array.from({ length: 6 }, (_, index) => `vieja_${index}`),
      addedColumns: Array.from({ length: 6 }, (_, index) => `nueva_${index}`),
      changedTypes: [
        { column: "total", expected: "Int64", actual: "String" },
        { column: "fecha", expected: "Date", actual: "String" },
      ],
    };
    render(<LoadPhase
      {...loadPhaseProps({ onSchemaMismatchAction })}
      pendingTaskName="Ventas"
      inspection={{ kind: "schema_mismatch", source: delimitedSource, profile, mismatch, sheetId: null, headerMode: "firstRow" }}
    />);
    const list = screen.getByRole("list", { name: "Diferencias de esquema" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(13);
    expect(list).toHaveTextContent("Y 2 diferencias más.");
    expect(screen.getByRole("alertdialog")).toHaveTextContent("La tarea “Ventas” no se aplicará");
    fireEvent.click(screen.getByRole("button", { name: "Importar con esquema nuevo" }));
    fireEvent.keyDown(screen.getByRole("alertdialog"), { key: "Escape" });
    expect(onSchemaMismatchAction.mock.calls.map(([action]) => action.kind)).toEqual(["import_new_schema", "cancelled"]);
  });

  it("propone convertir un CSV en Windows-1252 y reintentar la muestra tras otro error", () => {
    const onConvertEncoding = vi.fn();
    const onRetryHeaderPreview = vi.fn();
    const onSheetAction = vi.fn();
    const pending = delimitedHeaderInspection(delimitedSource);
    if (pending.kind !== "sheet") throw new Error("estado inesperado");
    const view = render(<LoadPhase
      {...loadPhaseProps({ onConvertEncoding, onRetryHeaderPreview, onSheetAction })}
      inspection={{ ...pending, headerReviewLoading: false, error: `${LEGACY_ENCODING_PREFIX}Año` }}
    />);
    expect(screen.getByRole("heading", { name: "Leer como Excel para Windows" })).toBeInTheDocument();
    expect(screen.getByText(/Así se leerá: «Año»/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Convertir y continuar" }));
    expect(onConvertEncoding).toHaveBeenCalledOnce();

    view.rerender(<LoadPhase
      {...loadPhaseProps({ onConvertEncoding, onRetryHeaderPreview, onSheetAction })}
      inspection={{ ...pending, headerReviewLoading: false, error: "muestra ilegible" }}
    />);
    expect(screen.getByRole("alert")).toHaveTextContent("No se pudo previsualizar el archivo: muestra ilegible");
    fireEvent.click(screen.getByRole("button", { name: "Reintentar muestra" }));
    expect(onRetryHeaderPreview).toHaveBeenCalledOnce();
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(onSheetAction).toHaveBeenCalledWith({ kind: "cancelled" });
  });

  it("muestra cabeceras repetidas sin claves duplicadas de React (COD-10)", () => {
    const keyWarnings = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const repeated = {
      ...delimitedHeaderReview,
      firstRow: {
        ...delimitedHeaderReview.firstRow,
        columns: [{ name: "id", dataType: "String" }, { name: "id", dataType: "String" }],
        rows: [["1", "2"]],
      },
    };
    const ready = completeDelimitedHeaderReview(delimitedHeaderInspection(delimitedSource), repeated);
    render(<LoadPhase {...loadPhaseProps()} inspection={ready} />);
    const sample = screen.getByLabelText("Muestra importada");
    expect(within(sample).getAllByRole("columnheader", { name: "id" })).toHaveLength(2);
    expect(keyWarnings.mock.calls.flat().join(" ")).not.toMatch(/same key/);
    keyWarnings.mockRestore();
  });

  it("emite las convenciones y el perfil guardado, y las desactiva en lectura por bloques", () => {
    const onSheetAction = vi.fn();
    const ready = completeDelimitedHeaderReview(delimitedHeaderInspection(delimitedSource, profile), delimitedHeaderReview);
    if (ready.kind !== "sheet") throw new Error("estado inesperado");
    const view = render(<LoadPhase {...loadPhaseProps({ onSheetAction })} inspection={{ ...ready, headerMode: "generated" }} />);
    fireEvent.click(screen.getByRole("radio", { name: /Usar la primera fila como encabezados/ }));
    expect(onSheetAction).toHaveBeenCalledWith({ kind: "header_mode_changed", headerMode: "firstRow" });
    fireEvent.change(screen.getByLabelText("Fechas"), { target: { value: "dmy" } });
    expect(onSheetAction).toHaveBeenCalledWith({ kind: "date_convention_changed", value: "dmy" });
    const profileToggle = screen.queryByRole("checkbox", { name: /Usar/ });
    if (profileToggle) {
      fireEvent.click(profileToggle);
      expect(onSheetAction).toHaveBeenCalledWith(expect.objectContaining({ kind: "profile_toggled" }));
    }

    const sourceBacked = {
      ...delimitedSource,
      resourceEstimate: { ...delimitedSource.resourceEstimate, processingPath: "sourceBacked" as const },
    };
    view.rerender(<LoadPhase {...loadPhaseProps({ onSheetAction })} inspection={{ ...ready, source: sourceBacked }} />);
    expect(screen.getByText(/Esta fuente se lee por bloques; las conversiones de fecha y número solo se aplican a archivos de menos de 512/)).toBeInTheDocument();
  });

  it("no falla cuando una acción opcional no recibe manejador", () => {
    const pending = delimitedHeaderInspection(delimitedSource);
    if (pending.kind !== "sheet") throw new Error("estado inesperado");
    const minimal = {
      runtime: { kind: "connected" as const },
      datasetStatus: { kind: "empty" as const },
      recentDatasets: [],
      onSelect: () => undefined,
      onSelectRecent: () => undefined,
      onClearRecent: () => undefined,
      onRemoveRecent: () => undefined,
      onSheetAction: () => undefined,
      onCancelLoad: () => undefined,
    };
    const view = render(<LoadPhase {...minimal} previousExitUnclean inspection={{ kind: "workbook_inspecting", source: workbook }} />);
    fireEvent.click(screen.getByRole("button", { name: "Entendido" }));
    fireEvent.click(screen.getByRole("button", { name: "Cancelar inspección" }));
    view.rerender(<LoadPhase {...minimal} inspection={{ kind: "selection_cancellation_failed", source: workbook, message: "x", cancelPending: false, discardPending: false }} />);
    fireEvent.click(screen.getByRole("button", { name: "Reintentar cancelación" }));
    view.rerender(<LoadPhase {...minimal} inspection={{ kind: "profile_review", source: delimitedSource, profile }} />);
    fireEvent.click(screen.getByRole("button", { name: "Importar sin perfil" }));
    view.rerender(<LoadPhase {...minimal} inspection={{ kind: "resource_preflight", source: delimitedSource }} />);
    fireEvent.click(screen.getByRole("button", { name: "Continuar con la carga" }));
    view.rerender(<LoadPhase {...minimal} inspection={{ kind: "schema_mismatch", source: delimitedSource, profile, mismatch: { code: "importProfileSchemaMismatch", missingColumns: ["a"], addedColumns: [], changedTypes: [] }, sheetId: null, headerMode: null }} />);
    fireEvent.click(screen.getByRole("button", { name: "Cancelar y conservar dataset" }));
    view.rerender(<LoadPhase {...minimal} inspection={{ ...pending, headerReviewLoading: false, error: `${LEGACY_ENCODING_PREFIX}x` }} />);
    fireEvent.click(screen.getByRole("button", { name: "Convertir y continuar" }));
    view.rerender(<LoadPhase {...minimal} inspection={{ ...pending, headerReviewLoading: false, error: "y" }} />);
    fireEvent.click(screen.getByRole("button", { name: "Reintentar muestra" }));
    view.rerender(<LoadPhase {...minimal} inspection={completeDelimitedHeaderReview(pending, delimitedHeaderReview)} />);
    fireEvent.click(screen.getByText("¿Columnas o acentos mal leídos?"));
    fireEvent.change(screen.getByRole("combobox", { name: "Separador" }), { target: { value: "|" } });
    fireEvent.click(screen.getByRole("button", { name: "Volver a leer" }));
    view.rerender(<LoadPhase {...minimal} inspection={{ kind: "idle" }} sampleDatasets={[{ id: "s", name: "Ejemplo", format: "csv", description: "d" }]} />);
    const sample = screen.queryAllByRole("button").find((button) => button.textContent?.includes("Ejemplo"));
    if (sample) fireEvent.click(sample);
  });
});
