import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as bridge from "../../bridge";
import type { DatabaseKind, DatasetPreview, RemoteExportPreflight, SavedRecipe } from "../../bridge";
import { DeliveryPhase } from "./DeliveryPhase";
import {
  INITIAL_DELIVERY_CONTRACT,
  reduceDeliveryContract,
  type DeliveryContractState,
  type DeliveryExportState,
  type DeliveryExportRequest,
} from "./deliveryModel";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const dataset: DatasetPreview = {
  fileName: "ventas.csv",
  fileSizeBytes: 2048,
  rowCount: 2,
  columnCount: 4,
  columns: [
    { name: "total", dataType: "Int64" },
    { name: "limite", dataType: "Int64" },
    { name: "estado", dataType: "String" },
    { name: "fecha", dataType: "String" },
  ],
  rows: [["10", "12", "ok", "2024-01-01"], ["20", "20", "ok", "2024-06-01"]],
};

function DeliveryHarness({
  onExport,
  recipeDraft = null,
  preparationChanges = [],
  initialContract = INITIAL_DELIVERY_CONTRACT,
  exportState = { kind: "idle" },
  onCancelExport = () => undefined,
  personalDataColumns = [],
}: {
  onExport: (request: DeliveryExportRequest) => void;
  recipeDraft?: SavedRecipe | null;
  preparationChanges?: string[];
  initialContract?: DeliveryContractState;
  exportState?: DeliveryExportState;
  onCancelExport?: () => void;
  personalDataColumns?: string[];
}) {
  const [contract, setContract] = useState<DeliveryContractState>(initialContract);
  return (
    <DeliveryPhase
      dataset={dataset}
      recipeDraft={recipeDraft}
      preparationChanges={preparationChanges}
      contract={contract}
      exportState={exportState}
      onContractAction={(action) => setContract((current) => reduceDeliveryContract(current, action))}
      onExport={onExport}
      onCancelExport={onCancelExport}
      personalDataColumns={personalDataColumns}
    />
  );
}

describe("señales de datos personales en Entregar", () => {
  it("lista las columnas detectadas y exige confirmar una exportación sin protección", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} personalDataColumns={["correo", "cliente"]} />);

    expect(screen.getByText(/Datos personales detectados/)).toBeInTheDocument();
    expect(screen.getByText(/en 2 columnas: correo, cliente\./)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("checkbox", { name: "Confirmo que quiero exportar sin validar la calidad" }));
    const exportButton = screen.getByRole("button", { name: "Exportar CSV" });
    expect(exportButton).toBeDisabled();

    fireEvent.click(screen.getByRole("checkbox", { name: "Confirmo que exporto estas columnas sin enmascarar ni aplicar hash" }));
    expect(exportButton).toBeEnabled();
  });

  it("no pide confirmación cuando se elige una protección", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} personalDataColumns={["correo"]} />);

    fireEvent.click(screen.getByRole("checkbox", { name: "Confirmo que quiero exportar sin validar la calidad" }));
    fireEvent.change(screen.getByRole("combobox", { name: "Protección de datos personales" }), { target: { value: "mask" } });

    expect(screen.queryByRole("checkbox", { name: /sin enmascarar ni aplicar hash/ })).not.toBeInTheDocument();
    expect(screen.getByText("La protección elegida se aplicará a estas columnas en la copia.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Exportar CSV" }));
    expect(onExport).toHaveBeenCalledWith(expect.objectContaining({ privacyMode: "mask" }));
  });
});

const recipeDraft: SavedRecipe = {
  version: 1,
  name: "Receta de prueba",
  savedAt: "2026-08-26T00:00:00Z",
  recipe: {
    renames: [],
    casts: [],
    dateParses: [],
    filters: [],
    calculatedColumn: null,
    findReplace: null,
    keepColumns: null,
    splitColumn: null,
    mergeColumns: null,
    outlierTreatments: [],
    groupSummary: null,
    contactNormalizations: [],
    textExtractions: [],
  },
};

function preflightResult(kind: DatabaseKind, ready = true): RemoteExportPreflight {
  return {
    kind,
    schema: "public",
    table: "dataset",
    tablePolicy: "create_only",
    tableExists: false,
    ready,
    issues: ready ? [] : [{
      severity: "blocking",
      category: "policy",
      column: null,
      message: "La tabla ya existe y la política Crear requiere una tabla nueva.",
    }],
  };
}

describe("DeliveryPhase", () => {
  it("exige confirmación explícita antes de exportar sin contrato", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.change(screen.getByRole("combobox", { name: "Formato de exportación" }), {
      target: { value: "parquet" },
    });
    const exportButton = screen.getByRole("button", { name: "Exportar Parquet" });
    expect(exportButton).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox", {
      name: "Confirmo que quiero exportar sin validar la calidad",
    }));
    expect(exportButton).toBeEnabled();
    fireEvent.click(exportButton);

    expect(onExport).toHaveBeenCalledWith({
      format: "parquet",
      privacyMode: "none",
      validation: { kind: "explicitly_unvalidated" },
    });
  });

  it("valida y exporta desde una acción cuando el contrato aprueba", async () => {
    const validate = vi.spyOn(bridge, "validateQualityRules").mockResolvedValue({
      passed: true,
      rowCount: 2,
      totalRules: 1,
      failedRules: 0,
      rules: [{
        column: "total",
        kind: "not_null",
        maxInvalid: 0,
        checkedCount: 2,
        invalidCount: 0,
        invalidPct: 0,
        passed: true,
      }],
    });
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    const exportButton = screen.getByRole("button", { name: "Validar y exportar CSV" });
    expect(exportButton).toBeEnabled();
    expect(screen.queryByRole("button", { name: "Validar contrato" })).not.toBeInTheDocument();
    fireEvent.click(exportButton);

    await waitFor(() =>
      expect(screen.getByRole("status")).toHaveTextContent("Contrato aprobado"),
    );
    expect(validate).toHaveBeenCalledOnce();
    expect(onExport).toHaveBeenCalledWith({
      format: "csv",
      privacyMode: "none",
      validation: {
        kind: "contract",
        rules: [{ column: "total", kind: "not_null", maxInvalid: 0 }],
      },
    });
    expect(onExport).toHaveBeenCalledOnce();
  });

  it("no duplica la validación si la acción principal recibe dos clics síncronos", async () => {
    let resolveValidation!: (result: Awaited<ReturnType<typeof bridge.validateQualityRules>>) => void;
    const validationPromise = new Promise<Awaited<ReturnType<typeof bridge.validateQualityRules>>>((resolve) => {
      resolveValidation = resolve;
    });
    const validate = vi.spyOn(bridge, "validateQualityRules").mockReturnValue(validationPromise);
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    const exportButton = screen.getByRole("button", { name: "Validar y exportar CSV" });
    act(() => {
      fireEvent.click(exportButton);
      fireEvent.click(exportButton);
    });

    expect(validate).toHaveBeenCalledOnce();
    resolveValidation({
      passed: true,
      rowCount: 2,
      totalRules: 1,
      failedRules: 0,
      rules: [{
        column: "total",
        kind: "not_null",
        maxInvalid: 0,
        checkedCount: 2,
        invalidCount: 0,
        invalidPct: 0,
        passed: true,
      }],
    });
    await waitFor(() => expect(onExport).toHaveBeenCalledOnce());
  });

  it("no exporta si falla la validación iniciada desde la acción principal", async () => {
    const validate = vi.spyOn(bridge, "validateQualityRules").mockResolvedValue({
      passed: false,
      rowCount: 2,
      totalRules: 1,
      failedRules: 1,
      rules: [{
        column: "total",
        kind: "not_null",
        maxInvalid: 0,
        checkedCount: 2,
        invalidCount: 1,
        invalidPct: 50,
        passed: false,
      }],
    });
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    fireEvent.click(screen.getByRole("button", { name: "Validar y exportar CSV" }));

    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Contrato fallido"));
    expect(screen.getByText("Valores nulos · total")).toBeInTheDocument();
    expect(validate).toHaveBeenCalledOnce();
    expect(onExport).not.toHaveBeenCalled();
  });

  it("exporta directamente cuando el gate vigente ya está aprobado", () => {
    const validate = vi.spyOn(bridge, "validateQualityRules");
    const onExport = vi.fn();
    const passedContract: DeliveryContractState = {
      kind: "with_contract",
      rules: [{ column: "total", kind: "not_null", maxInvalid: 0 }],
      gate: {
        kind: "ready",
        result: {
          passed: true,
          rowCount: 2,
          totalRules: 1,
          failedRules: 0,
          rules: [{
            column: "total",
            kind: "not_null",
            maxInvalid: 0,
            checkedCount: 2,
            invalidCount: 0,
            invalidPct: 0,
            passed: true,
          }],
        },
      },
    };
    render(<DeliveryHarness onExport={onExport} initialContract={passedContract} />);

    fireEvent.click(screen.getByRole("button", { name: "Exportar CSV" }));

    expect(validate).not.toHaveBeenCalled();
    expect(onExport).toHaveBeenCalledOnce();
    expect(onExport).toHaveBeenCalledWith({
      format: "csv",
      privacyMode: "none",
      validation: {
        kind: "contract",
        rules: [{ column: "total", kind: "not_null", maxInvalid: 0 }],
      },
    });
  });

  it("resume las reglas guardadas y mantiene el constructor fuera del recorrido habitual", () => {
    const initialContract: DeliveryContractState = {
      kind: "with_contract",
      rules: [{ column: "total", kind: "not_null", maxInvalid: 0 }],
      gate: { kind: "idle" },
    };
    render(<DeliveryHarness onExport={vi.fn()} initialContract={initialContract} />);

    expect(screen.getByRole("heading", { name: "Qué se exige" })).toBeInTheDocument();
    expect(screen.getByText("total: no admite valores nulos · no se permiten incumplimientos.")).toBeInTheDocument();
    expect(screen.queryByRole("combobox", { name: "Comprobación regla 1" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Validar y exportar CSV" })).toBeEnabled();

    fireEvent.click(screen.getByRole("button", { name: "Editar reglas" }));
    expect(screen.getByRole("combobox", { name: "Comprobación regla 1" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cerrar edición" })).toHaveAttribute("aria-expanded", "true");
  });

  it("incluye la calidad aprobada en el resultado publicado", () => {
    const approvedContract: DeliveryContractState = {
      kind: "with_contract",
      rules: [{ column: "total", kind: "not_null", maxInvalid: 0 }],
      gate: {
        kind: "ready",
        result: {
          passed: true,
          rowCount: 2,
          totalRules: 1,
          failedRules: 0,
          rules: [{
            column: "total",
            kind: "not_null",
            maxInvalid: 0,
            checkedCount: 2,
            invalidCount: 0,
            invalidPct: 0,
            passed: true,
          }],
        },
      },
    };
    render(<DeliveryHarness
      onExport={vi.fn()}
      initialContract={approvedContract}
      exportState={{
        kind: "success",
        result: {
          fileName: "ventas.csv",
          fileSizeBytes: 512,
          format: "CSV",
          protectedColumnCount: 0,
          protectedColumns: [],
        },
      }}
    />);

    const result = screen.getByRole("region", { name: "Copia lista" });
    expect(within(result).getByText("1 regla aprobada sobre 2 filas")).toBeInTheDocument();
    expect(within(result).queryByText("Esta copia no incluye una validación de calidad.")).not.toBeInTheDocument();
  });

  it("ofrece exportación JSON con la misma compuerta de calidad", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("checkbox", {
      name: "Confirmo que quiero exportar sin validar la calidad",
    }));
    fireEvent.change(screen.getByRole("combobox", { name: "Formato de exportación" }), {
      target: { value: "json" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Exportar JSON" }));

    expect(onExport).toHaveBeenCalledWith({
      format: "json",
      privacyMode: "none",
      validation: { kind: "explicitly_unvalidated" },
    });
  });

  it("ofrece exportación SQL con la misma compuerta de calidad", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("checkbox", {
      name: "Confirmo que quiero exportar sin validar la calidad",
    }));
    fireEvent.change(screen.getByRole("combobox", { name: "Formato de exportación" }), {
      target: { value: "sql" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Exportar SQL" }));

    expect(onExport).toHaveBeenCalledWith({
      format: "sql",
      privacyMode: "none",
      validation: { kind: "explicitly_unvalidated" },
    });
  });

  it("ofrece destinos Excel y SQLite con la compuerta de calidad", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("checkbox", {
      name: "Confirmo que quiero exportar sin validar la calidad",
    }));
    const format = screen.getByRole("combobox", { name: "Formato de exportación" });
    fireEvent.change(format, { target: { value: "excel" } });
    fireEvent.click(screen.getByRole("button", { name: "Exportar Excel" }));
    fireEvent.change(format, { target: { value: "sqlite" } });
    fireEvent.click(screen.getByRole("button", { name: "Exportar SQLite" }));

    expect(onExport).toHaveBeenNthCalledWith(1, {
      format: "excel",
      privacyMode: "none",
      validation: { kind: "explicitly_unvalidated" },
    });
    expect(onExport).toHaveBeenNthCalledWith(2, {
      format: "sqlite",
      privacyMode: "none",
      validation: { kind: "explicitly_unvalidated" },
    });
  });

  it("explica que el bundle incluirá la receta validada de la sesión", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} recipeDraft={recipeDraft} />);

    fireEvent.change(screen.getByRole("combobox", { name: "Formato de exportación" }), {
      target: { value: "bundle" },
    });

    expect(screen.getByRole("note")).toHaveTextContent("recipe.json");
    expect(screen.getByRole("note")).toHaveTextContent("manifest.json");
    expect(screen.getByRole("note")).toHaveTextContent("delivery-summary.md");
  });

  it("permite seleccionar una política de privacidad antes de exportar", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);
    const privacyControl = screen.getByRole("combobox", { name: "Protección de datos personales" });
    expect(privacyControl).toHaveAttribute("aria-describedby", "privacy-mode-note");
    expect(screen.getByText(/No equivale a anonimización/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("checkbox", {
      name: "Confirmo que quiero exportar sin validar la calidad",
    }));
    fireEvent.change(privacyControl, {
      target: { value: "hash" },
    });
    fireEvent.change(screen.getByRole("combobox", { name: "Formato de exportación" }), {
      target: { value: "excel" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Exportar Excel" }));

    expect(onExport).toHaveBeenCalledWith({
      format: "excel",
      privacyMode: "hash",
      validation: { kind: "explicitly_unvalidated" },
    });
  });

  it("analiza política y esquema antes de habilitar una tabla remota", async () => {
    const onExport = vi.fn();
    const preflight = vi.spyOn(bridge, "preflightDatabaseExport").mockResolvedValue(preflightResult("postgresql"));
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("checkbox", {
      name: "Confirmo que quiero exportar sin validar la calidad",
    }));
    fireEvent.change(screen.getByRole("combobox", { name: "Formato de exportación" }), {
      target: { value: "postgresql" },
    });
    const exportButton = screen.getByRole("button", { name: "Exportar PostgreSQL" });
    expect(exportButton).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Cadena de conexión ODBC"), {
      target: { value: "Driver={PostgreSQL Unicode};Server=localhost;Pwd=secret" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Analizar compatibilidad" }));

    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Preflight completo"));
    expect(preflight).toHaveBeenCalledWith(expect.objectContaining({
      kind: "postgresql",
      connectionString: "Driver={PostgreSQL Unicode};Server=localhost;Pwd=secret",
      table: "dataset",
      tablePolicy: "create_only",
    }), "none");
    expect(exportButton).toBeEnabled();
    fireEvent.click(exportButton);
    expect(onExport).toHaveBeenCalledWith(expect.objectContaining({
      format: "postgresql",
      databaseTarget: expect.objectContaining({ tablePolicy: "create_only" }),
      validation: { kind: "explicitly_unvalidated" },
    }));
  });

  it("deriva el motor ODBC del formato remoto seleccionado", async () => {
    const preflight = vi.spyOn(bridge, "preflightDatabaseExport")
      .mockImplementation(async (target) => preflightResult(target.kind));
    render(<DeliveryHarness onExport={vi.fn()} />);
    fireEvent.click(screen.getByRole("checkbox", {
      name: "Confirmo que quiero exportar sin validar la calidad",
    }));
    for (const format of ["mysql", "sqlserver"] as const) {
      fireEvent.change(screen.getByRole("combobox", { name: "Formato de exportación" }), {
        target: { value: format },
      });
      fireEvent.change(screen.getByLabelText("Cadena de conexión ODBC"), {
        target: { value: `Driver={${format}};Server=localhost` },
      });
      fireEvent.click(screen.getByRole("button", { name: "Analizar compatibilidad" }));
      await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Preflight completo"));
    }
    expect(preflight.mock.calls.map(([target]) => target.kind)).toEqual(["mysql", "sqlserver"]);
  });

  it("descarta un preflight cuando cambia el destino durante la petición", async () => {
    let resolvePreflight: (result: RemoteExportPreflight) => void = () => undefined;
    const pending = new Promise<RemoteExportPreflight>((resolve) => {
      resolvePreflight = resolve;
    });
    vi.spyOn(bridge, "preflightDatabaseExport").mockReturnValue(pending);
    render(<DeliveryHarness onExport={vi.fn()} />);
    fireEvent.click(screen.getByRole("checkbox", {
      name: "Confirmo que quiero exportar sin validar la calidad",
    }));
    fireEvent.change(screen.getByRole("combobox", { name: "Formato de exportación" }), {
      target: { value: "mysql" },
    });
    fireEvent.change(screen.getByLabelText("Cadena de conexión ODBC"), {
      target: { value: "Driver={mysql};Server=A" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Analizar compatibilidad" }));
    fireEvent.change(screen.getByLabelText("Cadena de conexión ODBC"), {
      target: { value: "Driver={mysql};Server=B" },
    });
    resolvePreflight(preflightResult("mysql"));
    await Promise.resolve();
    expect(screen.queryByText("Preflight completo para dataset.")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Exportar MySQL" })).toBeDisabled();
  });

  it("bloquea la exportación cuando el preflight detecta un conflicto de política", async () => {
    vi.spyOn(bridge, "preflightDatabaseExport").mockResolvedValue(preflightResult("postgresql", false));
    render(<DeliveryHarness onExport={vi.fn()} />);
    fireEvent.click(screen.getByRole("checkbox", {
      name: "Confirmo que quiero exportar sin validar la calidad",
    }));
    fireEvent.change(screen.getByRole("combobox", { name: "Formato de exportación" }), {
      target: { value: "postgresql" },
    });
    fireEvent.change(screen.getByLabelText("Cadena de conexión ODBC"), {
      target: { value: "Driver={PostgreSQL Unicode};Server=localhost" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Analizar compatibilidad" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("Preflight bloqueado");
    expect(screen.getByRole("alert")).toHaveTextContent("La tabla ya existe");
    expect(screen.getByRole("button", { name: "Exportar PostgreSQL" })).toBeDisabled();
  });

  it("relee presets, verifica el esquema y exige volver a autorizar una política replace", async () => {
    const summary = {
      id: "0123456789abcdef0123456789abcdef",
      name: "Destino analítico",
      format: "postgresql" as const,
      updatedAt: "2026-09-14T00:00:00Z",
      selectedColumnCount: 2,
      remote: true,
    };
    const preset: bridge.DeliveryPreset = {
      version: 1,
      name: summary.name,
      format: "postgresql",
      selectedColumns: ["total", "cliente_antiguo"],
      privacyMode: "hash",
      databaseTarget: {
        kind: "postgresql",
        schema: "analytics",
        table: "ventas",
        tablePolicy: "replace",
      },
    };
    const list = vi.spyOn(bridge, "listDeliveryPresets").mockResolvedValue([summary]);
    vi.spyOn(bridge, "openDeliveryPreset").mockResolvedValue(preset);
    render(<DeliveryHarness onExport={vi.fn()} />);

    fireEvent.click(screen.getByText("Presets de entrega guardados"));
    await waitFor(() => expect(list).toHaveBeenCalledOnce());
    fireEvent.change(screen.getByRole("combobox", { name: "Preset de entrega local" }), {
      target: { value: summary.id },
    });
    fireEvent.click(screen.getByRole("button", { name: "Abrir y verificar" }));

    expect(await screen.findByText(/El esquema guardado no coincide exactamente/)).toBeInTheDocument();
    expect(screen.getByText(/Faltan: cliente_antiguo/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Aplicar tras revisar esquema" }));

    expect(screen.getByRole("combobox", { name: "Formato de exportación" })).toHaveValue("postgresql");
    expect(screen.getByRole("combobox", { name: "Protección de datos personales" })).toHaveValue("hash");
    expect(screen.getByLabelText("Esquema de destino")).toHaveValue("analytics");
    expect(screen.getByLabelText("Tabla")).toHaveValue("ventas");
    expect(screen.getByLabelText("Política de tabla")).toHaveValue("create_only");
    expect(screen.getByLabelText("Cadena de conexión ODBC")).toHaveValue("");
    expect(screen.getByText(/El preset proponía reemplazar la tabla/)).toHaveTextContent("Se cargó Crear sin reemplazar");
    expect(screen.getByRole("button", { name: "Exportar PostgreSQL" })).toBeDisabled();
  });

  it("expone los parámetros de una regla avanzada según su tipo", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    fireEvent.change(screen.getByRole("combobox", { name: "Comprobación regla 1" }), {
      target: { value: "allowed_values" },
    });

    expect(screen.getByRole("textbox", { name: "Valores permitidos regla 1" })).toBeInTheDocument();
    expect(screen.getByText("Un valor por línea; se compara sin transformar.")).toBeInTheDocument();
  });

  it("expone operadores y dos columnas para comparar columnas", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    fireEvent.change(screen.getByRole("combobox", { name: "Comprobación regla 1" }), {
      target: { value: "column_compare" },
    });

    expect(screen.getByRole("combobox", { name: "Columna izquierda comparar regla 1" })).toHaveValue("total");
    expect(screen.getByRole("combobox", { name: "Operador comparar regla 1" })).toHaveValue("eq");
    expect(screen.getByRole("combobox", { name: "Columna derecha comparar regla 1" })).toHaveValue("limite");
    fireEvent.change(screen.getByRole("combobox", { name: "Operador comparar regla 1" }), {
      target: { value: "lte" },
    });
    expect(screen.getByRole("combobox", { name: "Operador comparar regla 1" })).toHaveValue("lte");
  });

  it("expone claves y valores para integridad referencial simple y compuesta", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    fireEvent.change(screen.getByRole("combobox", { name: "Comprobación regla 1" }), {
      target: { value: "referential_integrity" },
    });

    expect(screen.getByRole("checkbox", { name: "Columna referencial total, regla 1" })).toBeChecked();
    expect(screen.getByRole("textbox", { name: "Valores permitidos de referencia regla 1" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("checkbox", { name: "Columna referencial limite, regla 1" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Valores permitidos de referencia regla 1" }), {
      target: { value: "[10,12]\n[20,20]" },
    });

    expect(screen.getByRole("checkbox", { name: "Columna referencial limite, regla 1" })).toBeChecked();
    expect(screen.getByRole("textbox", { name: "Valores permitidos de referencia regla 1" })).toHaveValue("[10,12]\n[20,20]");
  });

  it("expone la dirección de monotonicidad", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    fireEvent.change(screen.getByRole("combobox", { name: "Comprobación regla 1" }), {
      target: { value: "monotonic" },
    });

    const direction = screen.getByRole("combobox", { name: "Dirección monotónica regla 1" });
    expect(direction).toHaveValue("increasing");
    fireEvent.change(direction, { target: { value: "decreasing" } });
    expect(direction).toHaveValue("decreasing");
  });

  it("expone controles de agregación y reconciliación", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    const kind = screen.getByRole("combobox", { name: "Comprobación regla 1" });
    fireEvent.change(kind, { target: { value: "aggregate_check" } });

    expect(screen.getByRole("combobox", { name: "Agregación regla 1" })).toHaveValue("sum");
    expect(screen.getByRole("spinbutton", { name: "Valor esperado agregado regla 1" })).toHaveValue(0);
    expect(screen.getByRole("textbox", { name: "Referencias numéricas opcionales regla 1" })).toBeInTheDocument();
    fireEvent.change(screen.getByRole("combobox", { name: "Agregación regla 1" }), {
      target: { value: "max" },
    });
    expect(screen.getByRole("combobox", { name: "Agregación regla 1" })).toHaveValue("max");

    fireEvent.change(kind, { target: { value: "aggregate_reconciliation" } });
    expect(screen.getByRole("combobox", { name: "Columna izquierda agregada regla 1" })).toHaveValue("total");
    expect(screen.getByRole("combobox", { name: "Columna derecha agregada regla 1" })).toHaveValue("limite");
    expect(screen.getByRole("spinbutton", { name: "Tolerancia absoluta agregada regla 1" })).toBeInTheDocument();
    expect(screen.getByRole("spinbutton", { name: "Tolerancia relativa agregada regla 1" })).toBeInTheDocument();
  });

  it("expone controles de línea base para distribution_drift", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    fireEvent.change(screen.getByRole("combobox", { name: "Comprobación regla 1" }), {
      target: { value: "distribution_drift" },
    });

    const baseline = screen.getByRole("textbox", { name: "Línea base numérica regla 1" });
    expect(baseline).toBeInTheDocument();
    fireEvent.change(baseline, { target: { value: "10\n20" } });
    const threshold = screen.getByRole("spinbutton", { name: "Umbral de drift regla 1" });
    fireEvent.change(threshold, { target: { value: "2" } });
    expect(baseline).toHaveValue("10\n20");
    expect(threshold).toHaveValue(2);
  });

  it("expone límites de fecha para date_range", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    fireEvent.change(screen.getByRole("combobox", { name: "Comprobación regla 1" }), {
      target: { value: "date_range" },
    });
    fireEvent.change(screen.getByLabelText("Columna regla 1"), { target: { value: "fecha" } });
    fireEvent.change(screen.getByLabelText("Fecha mínima regla 1"), { target: { value: "2024-01-01" } });
    fireEvent.change(screen.getByLabelText("Fecha máxima regla 1"), { target: { value: "2024-12-31" } });

    expect(screen.getByLabelText("Fecha mínima regla 1")).toHaveValue("2024-01-01");
    expect(screen.getByLabelText("Fecha máxima regla 1")).toHaveValue("2024-12-31");
  });

  it("expone la condición y la subregla then de conditional", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    fireEvent.change(screen.getByRole("combobox", { name: "Comprobación regla 1" }), {
      target: { value: "conditional" },
    });

    expect(screen.getByRole("combobox", { name: "Columna condición regla 1" })).toHaveValue("total");
    expect(screen.getByRole("combobox", { name: "Operador condición regla 1" })).toHaveValue("eq");
    expect(screen.getByRole("textbox", { name: "Valor condición regla 1" })).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Columna objetivo conditional regla 1" })).toHaveValue("total");

    fireEvent.change(screen.getByRole("combobox", { name: "Comprobación then regla 1" }), {
      target: { value: "allowed_values" },
    });
    expect(screen.getByRole("textbox", { name: "Valores permitidos then regla 1" })).toBeInTheDocument();
  });

  it("expone el contrato de esquema y sus restricciones estructurales", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    fireEvent.change(screen.getByRole("combobox", { name: "Comprobación regla 1" }), {
      target: { value: "schema_contract" },
    });

    expect(screen.getByRole("textbox", { name: "Columnas requeridas esquema regla 1" })).toBeInTheDocument();
    expect(screen.getByRole("checkbox", {
      name: "Permitir columnas adicionales esquema regla 1",
    })).toBeChecked();
    expect(screen.getByRole("textbox", { name: "Orden requerido, opcional esquema regla 1" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("checkbox", {
      name: "Permitir columnas adicionales esquema regla 1",
    }));
    expect(screen.getByRole("checkbox", {
      name: "Permitir columnas adicionales esquema regla 1",
    })).not.toBeChecked();
  });

  it("cubre los editores de parámetros y sus transiciones de regla", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    fireEvent.click(screen.getByRole("radio", { name: /^Exportar sin validar/ }));
    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    const kind = screen.getByRole("combobox", { name: "Comprobación regla 1" });

    fireEvent.change(screen.getByRole("spinbutton", { name: "Inválidos máximos regla 1" }), {
      target: { value: "2" },
    });
    fireEvent.change(kind, { target: { value: "numeric_range" } });
    fireEvent.change(screen.getByRole("spinbutton", { name: "Mínimo inclusivo regla 1" }), {
      target: { value: "1" },
    });
    fireEvent.change(screen.getByRole("spinbutton", { name: "Máximo inclusivo regla 1" }), {
      target: { value: "" },
    });
    fireEvent.change(kind, { target: { value: "allowed_values" } });
    fireEvent.change(screen.getByRole("textbox", { name: "Valores permitidos regla 1" }), {
      target: { value: "ok\npending" },
    });
    fireEvent.change(kind, { target: { value: "regex" } });
    fireEvent.change(screen.getByRole("textbox", { name: "Patrón regular regla 1" }), {
      target: { value: "^ok$" },
    });
    fireEvent.change(kind, { target: { value: "dtype" } });
    fireEvent.change(screen.getByRole("combobox", { name: "Tipo esperado regla 1" }), {
      target: { value: "integer" },
    });

    fireEvent.change(kind, { target: { value: "aggregate_check" } });
    fireEvent.change(screen.getByRole("spinbutton", { name: "Valor esperado agregado regla 1" }), {
      target: { value: "10" },
    });
    fireEvent.change(screen.getByRole("textbox", { name: "Referencias numéricas opcionales regla 1" }), {
      target: { value: "10\n20" },
    });
    fireEvent.change(screen.getByRole("spinbutton", { name: "Tolerancia absoluta agregada regla 1" }), {
      target: { value: "1" },
    });
    fireEvent.change(screen.getByRole("spinbutton", { name: "Tolerancia relativa agregada regla 1" }), {
      target: { value: "0.1" },
    });

    fireEvent.change(kind, { target: { value: "aggregate_reconciliation" } });
    fireEvent.change(screen.getByRole("combobox", { name: "Columna izquierda agregada regla 1" }), {
      target: { value: "limite" },
    });
    fireEvent.change(screen.getByRole("combobox", { name: "Columna derecha agregada regla 1" }), {
      target: { value: "estado" },
    });
    fireEvent.change(kind, { target: { value: "column_compare" } });
    fireEvent.change(screen.getByRole("combobox", { name: "Columna izquierda comparar regla 1" }), {
      target: { value: "limite" },
    });
    fireEvent.change(screen.getByRole("combobox", { name: "Columna derecha comparar regla 1" }), {
      target: { value: "estado" },
    });

    fireEvent.change(kind, { target: { value: "conditional" } });
    fireEvent.change(screen.getByRole("combobox", { name: "Columna condición regla 1" }), {
      target: { value: "estado" },
    });
    fireEvent.change(screen.getByRole("combobox", { name: "Operador condición regla 1" }), {
      target: { value: "ne" },
    });
    fireEvent.change(screen.getByRole("textbox", { name: "Valor condición regla 1" }), {
      target: { value: "pending" },
    });
    fireEvent.change(screen.getByRole("combobox", { name: "Columna objetivo conditional regla 1" }), {
      target: { value: "limite" },
    });
    const thenKind = screen.getByRole("combobox", { name: "Comprobación then regla 1" });
    fireEvent.change(thenKind, { target: { value: "numeric_range" } });
    fireEvent.change(screen.getByRole("spinbutton", { name: "Mínimo then regla 1" }), {
      target: { value: "1" },
    });
    fireEvent.change(screen.getByRole("spinbutton", { name: "Máximo then regla 1" }), {
      target: { value: "2" },
    });
    fireEvent.change(thenKind, { target: { value: "allowed_values" } });
    fireEvent.change(screen.getByRole("textbox", { name: "Valores permitidos then regla 1" }), {
      target: { value: "ok" },
    });
    fireEvent.change(thenKind, { target: { value: "regex" } });
    fireEvent.change(screen.getByRole("textbox", { name: "Patrón regular then regla 1" }), {
      target: { value: "^ok$" },
    });
    fireEvent.change(thenKind, { target: { value: "dtype" } });
    fireEvent.change(screen.getByRole("combobox", { name: "Tipo esperado then regla 1" }), {
      target: { value: "string" },
    });

    fireEvent.change(kind, { target: { value: "schema_contract" } });
    fireEvent.change(screen.getByRole("textbox", { name: "Columnas requeridas esquema regla 1" }), {
      target: { value: "total\nestado" },
    });
    fireEvent.change(screen.getByRole("textbox", { name: "Orden requerido, opcional esquema regla 1" }), {
      target: { value: "total\nestado" },
    });

    expect(screen.getByRole("textbox", { name: "Columnas requeridas esquema regla 1" })).toHaveValue("total\nestado");
    expect(screen.getByRole("textbox", { name: "Orden requerido, opcional esquema regla 1" })).toHaveValue("total\nestado");
  });

  it("guarda el contrato activo como documento Columnia v1", async () => {
    const save = vi.spyOn(bridge, "saveQualityRulesDocument").mockResolvedValue({
      format: "columnia-quality-rules",
      version: 1,
      rules: [{ column: "total", kind: "not_null", maxInvalid: 0 }],
    });
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    fireEvent.click(screen.getByRole("button", { name: "Guardar contrato" }));

    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Contrato guardado"));
    expect(screen.getByRole("status")).toHaveTextContent("Columnia v1");
    expect(screen.getByRole("status")).toHaveTextContent("1 reglas");
    expect(save).toHaveBeenCalledWith([
      { column: "total", kind: "not_null", maxInvalid: 0 },
    ]);
  });

  it("muestra los estados fallido, desactualizado y de error del gate", async () => {
    const result = {
      passed: false,
      rowCount: 2,
      totalRules: 1,
      failedRules: 1,
      rules: [{
        column: "total",
        kind: "not_null" as const,
        maxInvalid: 0,
        checkedCount: 2,
        invalidCount: 1,
        invalidPct: 50,
        passed: false,
      }],
    };
    vi.spyOn(bridge, "validateQualityRules").mockResolvedValue(result);
    const failedContract: DeliveryContractState = {
      kind: "with_contract",
      rules: [{ column: "total", kind: "not_null", maxInvalid: 0 }],
      gate: { kind: "ready", result },
    };
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} initialContract={failedContract} />);
    expect(screen.getByRole("status")).toHaveTextContent("Contrato fallido");
    expect(screen.getByText("Problemas detectados")).toBeInTheDocument();
    expect(screen.getByText("Valores nulos · total")).toBeInTheDocument();
    expect(screen.getByText(/1 incumplimientos entre 2 elementos evaluados \(50\.00%\)/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Revisar regla 1" }));
    expect(document.activeElement).toBe(document.getElementById("quality-rule-1"));
    expect(screen.getByRole("button", { name: "Validar y exportar CSV" })).toBeEnabled();

    const staleContract: DeliveryContractState = {
      ...failedContract,
      gate: { kind: "stale", result },
    };
    cleanup();
    render(<DeliveryHarness onExport={onExport} initialContract={staleContract} />);
    expect(screen.getByRole("status")).toHaveTextContent("Resultado desactualizado");
    expect(screen.queryByRole("button", { name: "Revisar regla 1" })).not.toBeInTheDocument();

    vi.restoreAllMocks();
    vi.spyOn(bridge, "validateQualityRules").mockRejectedValue(new Error("gate no disponible"));
    cleanup();
    render(<DeliveryHarness onExport={onExport} />);
    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    fireEvent.click(screen.getByRole("button", { name: "Validar y exportar CSV" }));
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("gate no disponible"));
    expect(onExport).not.toHaveBeenCalled();
  });

  it("maneja importaciones y guardados cancelados, nulos y fallidos", async () => {
    const onExport = vi.fn();
    const pick = vi.spyOn(bridge, "pickQualityRulesMigration").mockResolvedValue(null);
    render(<DeliveryHarness onExport={onExport} />);
    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    fireEvent.click(screen.getByRole("button", { name: "Importar contrato" }));
    await waitFor(() => expect(pick).toHaveBeenCalledOnce());
    expect(screen.queryByText("Importación revisada")).not.toBeInTheDocument();

    pick.mockRejectedValueOnce(new Error("archivo ilegible"));
    fireEvent.click(screen.getByRole("button", { name: "Importar contrato" }));
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("archivo ilegible"));

    const save = vi.spyOn(bridge, "saveQualityRulesDocument").mockResolvedValue(null);
    fireEvent.click(screen.getByRole("button", { name: "Guardar contrato" }));
    await waitFor(() => expect(save).toHaveBeenCalledOnce());
    expect(screen.queryByText("Contrato guardado")).not.toBeInTheDocument();

    save.mockRejectedValueOnce(new Error("carpeta no disponible"));
    fireEvent.click(screen.getByRole("button", { name: "Guardar contrato" }));
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("carpeta no disponible"));
  });

  it("presenta el progreso de exportación, permite cancelarlo y muestra resultados", async () => {
    const onCancelExport = vi.fn();
    const loading: DeliveryExportState = {
      kind: "loading",
      format: "csv",
      progress: { operation: "export", stage: "Escribiendo", percent: 40 },
      cancellation: "available",
    };
    render(<DeliveryHarness onExport={vi.fn()} exportState={loading} onCancelExport={onCancelExport} />);
    expect(screen.getByRole("heading", { name: "Exportando dataset" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(onCancelExport).toHaveBeenCalledOnce();

    cleanup();
    render(<DeliveryHarness onExport={vi.fn()} exportState={{ ...loading, cancellation: "requested" }} />);
    expect(screen.getByRole("button", { name: "Cancelando…" })).toBeDisabled();

    cleanup();
    render(<DeliveryHarness
      onExport={vi.fn()}
      recipeDraft={recipeDraft}
      preparationChanges={["Espacios exteriores recortados", "Encabezados normalizados"]}
      exportState={{
        kind: "success",
        result: {
          fileName: "entrega.zip",
          fileSizeBytes: 2048,
          format: "Paquete Columnia",
          protectedColumnCount: 1,
          protectedColumns: ["email"],
        },
      }}
    />);
    const result = screen.getByRole("region", { name: "Copia lista" });
    expect(within(result).getByRole("heading", { name: "Copia lista" })).toBeInTheDocument();
    expect(within(result).getByText("entrega.zip")).toBeInTheDocument();
    expect(within(result).getByText(/^2[.,]0 KiB$/)).toBeInTheDocument();
    expect(within(result).getByText("Salida confirmada sin reglas de calidad")).toBeInTheDocument();
    expect(within(result).getByText("2 cambios del historial activo")).toBeInTheDocument();
    expect(within(result).getByText("1 columnas: email")).toBeInTheDocument();
    expect(within(result).getByText(/recipe\.json incluye la receta validada/)).toBeInTheDocument();
    expect(within(result).getByText("Esta copia no incluye una validación de calidad.")).toBeInTheDocument();
    fireEvent.click(within(result).getByText("Ver cambios incluidos (2)"));
    expect(within(result).getByText("Espacios exteriores recortados")).toBeInTheDocument();
    const openLastExport = vi.spyOn(bridge, "openLastExport").mockResolvedValue(undefined);
    fireEvent.click(screen.getByRole("button", { name: "Abrir carpeta" }));
    await waitFor(() => expect(openLastExport).toHaveBeenCalledOnce());
    expect(screen.getByText("Carpeta de exportación abierta.")).toBeInTheDocument();
    openLastExport.mockRejectedValueOnce(new Error("salida eliminada"));
    fireEvent.click(screen.getByRole("button", { name: "Abrir carpeta" }));
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("No se pudo abrir la carpeta de exportación."));

    cleanup();
    render(<DeliveryHarness onExport={vi.fn()} exportState={{ kind: "cancelled" }} />);
    expect(screen.getByRole("status")).toHaveTextContent("Exportación cancelada");
    expect(screen.getByRole("status")).toHaveTextContent("dataset preparado sigue disponible");

    cleanup();
    render(<DeliveryHarness onExport={vi.fn()} exportState={{ kind: "error", message: "disco lleno" }} />);
    expect(screen.getByRole("alert")).toHaveTextContent("No se pudo crear la copia");
    expect(screen.getByRole("alert")).toHaveTextContent("disco lleno");
    expect(screen.getByRole("alert")).toHaveTextContent("dataset preparado sigue disponible");
  });

  it("cubre cambios de regla, tolerancias, columnas y eliminación", () => {
    const onContractAction = vi.fn();
    function ControlledDelivery() {
      const [contract, setContract] = useState<DeliveryContractState>({
        kind: "with_contract",
        rules: [{ column: "total", kind: "not_null", maxInvalid: 0 }],
        gate: { kind: "idle" },
      });
      return <DeliveryPhase
        dataset={dataset}
        contract={contract}
        exportState={{ kind: "idle" }}
        onContractAction={(action) => {
          onContractAction(action);
          setContract((current) => reduceDeliveryContract(current, action));
        }}
        onExport={vi.fn()}
        onCancelExport={vi.fn()}
      />;
    }
    render(<ControlledDelivery />);
    fireEvent.click(screen.getByRole("button", { name: "Editar reglas" }));
    const kind = screen.getByRole("combobox", { name: "Comprobación regla 1" });
    fireEvent.change(screen.getByRole("combobox", { name: "Tolerancia regla 1" }), { target: { value: "both" } });
    fireEvent.change(screen.getByRole("spinbutton", { name: "Porcentaje máximo regla 1" }), { target: { value: "5" } });
    fireEvent.change(kind, { target: { value: "unique_together" } });
    fireEvent.click(screen.getByRole("checkbox", { name: "Columna compuesta limite, regla 1" }));
    fireEvent.change(screen.getByRole("combobox", { name: "Comprobación regla 1" }), { target: { value: "column_compare" } });
    fireEvent.change(screen.getByRole("combobox", { name: "Columna derecha comparar regla 1" }), { target: { value: "estado" } });
    fireEvent.click(screen.getByRole("button", { name: "Eliminar regla 1" }));
    expect(onContractAction).toHaveBeenCalled();
    expect(screen.getByText("Entrega no validada")).toBeInTheDocument();
  });

  it("cubre la cancelación y el error al cancelar una validación de calidad", async () => {
    const cancel = vi.spyOn(bridge, "cancelOperation").mockResolvedValue(undefined);
    const loadingContract: DeliveryContractState = {
      kind: "with_contract",
      rules: [{ column: "total", kind: "not_null", maxInvalid: 0 }],
      gate: { kind: "loading" },
    };
    render(<DeliveryHarness onExport={vi.fn()} initialContract={loadingContract} />);

    fireEvent.click(screen.getByRole("button", { name: "Cancelar validación" }));
    await waitFor(() => expect(cancel).toHaveBeenCalledWith("qualityValidation"));
    expect(screen.getByRole("button", { name: "Esperando cancelación…" })).toBeDisabled();

    cleanup();
    cancel.mockRejectedValueOnce(new Error("cancelación no disponible"));
    render(<DeliveryHarness onExport={vi.fn()} initialContract={loadingContract} />);
    fireEvent.click(screen.getByRole("button", { name: "Cancelar validación" }));
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("cancelación no disponible"));
    expect(screen.getByRole("button", { name: "Cancelar validación" })).toBeEnabled();
  });

  it("expone los estados de error, motor inesperado y cancelación del preflight", async () => {
    const preflight = vi.spyOn(bridge, "preflightDatabaseExport");
    const cancel = vi.spyOn(bridge, "cancelOperation").mockResolvedValue(undefined);
    render(<DeliveryHarness onExport={vi.fn()} />);
    fireEvent.click(screen.getByRole("checkbox", {
      name: "Confirmo que quiero exportar sin validar la calidad",
    }));
    fireEvent.change(screen.getByRole("combobox", { name: "Formato de exportación" }), {
      target: { value: "postgresql" },
    });
    const connection = screen.getByLabelText("Cadena de conexión ODBC");
    fireEvent.change(connection, { target: { value: "Driver={PostgreSQL};Server=localhost" } });

    preflight.mockResolvedValueOnce(preflightResult("mysql"));
    fireEvent.click(screen.getByRole("button", { name: "Analizar compatibilidad" }));
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("no corresponde al motor"));

    preflight.mockRejectedValueOnce(new Error("ODBC no disponible"));
    fireEvent.click(screen.getByRole("button", { name: "Analizar compatibilidad" }));
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("ODBC no disponible"));

    let resolvePreflight!: (result: RemoteExportPreflight) => void;
    preflight.mockReturnValueOnce(new Promise((resolve) => {
      resolvePreflight = resolve;
    }));
    fireEvent.click(screen.getByRole("button", { name: "Analizar compatibilidad" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Cancelar análisis" })).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: "Cancelar análisis" }));
    await waitFor(() => expect(cancel).toHaveBeenCalledWith("databasePreflight"));
    resolvePreflight(preflightResult("postgresql"));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Preflight completo"));
  });

  it("muestra una cancelación y un error del catálogo de presets", async () => {
    let resolveCatalog!: (value: bridge.DeliveryPresetSummary[]) => void;
    const list = vi.spyOn(bridge, "listDeliveryPresets").mockReturnValue(new Promise((resolve) => {
      resolveCatalog = resolve;
    }));
    const cancel = vi.spyOn(bridge, "cancelOperation").mockResolvedValue(undefined);
    render(<DeliveryHarness onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Presets de entrega guardados"));
    await waitFor(() => expect(list).toHaveBeenCalledOnce());
    fireEvent.click(screen.getByRole("button", { name: "Cancelar carga" }));
    await waitFor(() => expect(cancel).toHaveBeenCalledWith("deliveryPresetCatalog"));
    expect(screen.getByRole("status")).toHaveTextContent("Se canceló la carga de presets locales.");

    resolveCatalog([]);
    const retry = screen.getByRole("button", { name: "Reintentar catálogo" });
    list.mockRejectedValueOnce(new Error("catálogo no disponible"));
    fireEvent.click(retry);
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("catálogo no disponible"));
  });

  it("guarda, aplica y elimina un preset de entrega con el esquema actual", async () => {
    const summary: bridge.DeliveryPresetSummary = {
      id: "0123456789abcdef0123456789abcdef",
      name: "Cierre mensual",
      format: "csv",
      updatedAt: "2026-09-14T00:00:00Z",
      selectedColumnCount: dataset.columns.length,
      remote: false,
    };
    const save = vi.spyOn(bridge, "saveDeliveryPreset").mockResolvedValue(summary);
    const remove = vi.spyOn(bridge, "deleteDeliveryPreset").mockResolvedValue(undefined);
    vi.spyOn(bridge, "listDeliveryPresets").mockResolvedValue([]);
    render(<DeliveryHarness onExport={vi.fn()} />);
    fireEvent.click(screen.getByText("Presets de entrega guardados"));
    await waitFor(() => expect(screen.getByRole("note")).toHaveTextContent("Aún no hay presets locales."));
    fireEvent.change(screen.getByRole("textbox", { name: "Nombre del preset de entrega" }), {
      target: { value: "  Cierre mensual  " },
    });
    fireEvent.click(screen.getByRole("button", { name: "Guardar preset" }));
    await waitFor(() => expect(screen.getByText(/Preset guardado localmente/)).toBeInTheDocument());
    expect(save).toHaveBeenCalledWith(null, expect.objectContaining({
      name: "Cierre mensual",
      format: "csv",
      selectedColumns: ["total", "limite", "estado", "fecha"],
      privacyMode: "none",
    }));
    expect(screen.getByRole("button", { name: "Aplicar preset verificado" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Aplicar preset verificado" }));
    expect(screen.getByText("Preset verificado contra el esquema actual.")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Eliminar preset" }));
    await waitFor(() => expect(remove).toHaveBeenCalledWith(summary.id));
    expect(screen.getByText("Preset eliminado del catálogo local.")).toBeInTheDocument();
  });
});
