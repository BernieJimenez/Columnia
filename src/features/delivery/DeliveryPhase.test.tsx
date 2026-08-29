import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as bridge from "../../bridge";
import type { DatasetPreview, SavedRecipe } from "../../bridge";
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
  initialContract = INITIAL_DELIVERY_CONTRACT,
  exportState = { kind: "idle" },
  onCancelExport = () => undefined,
}: {
  onExport: (request: DeliveryExportRequest) => void;
  recipeDraft?: SavedRecipe | null;
  initialContract?: DeliveryContractState;
  exportState?: DeliveryExportState;
  onCancelExport?: () => void;
}) {
  const [contract, setContract] = useState<DeliveryContractState>(initialContract);
  return (
    <DeliveryPhase
      dataset={dataset}
      recipeDraft={recipeDraft}
      contract={contract}
      exportState={exportState}
      onContractAction={(action) => setContract((current) => reduceDeliveryContract(current, action))}
      onExport={onExport}
      onCancelExport={onCancelExport}
    />
  );
}

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

  it("habilita la exportación con contrato solo después de aprobar el gate", async () => {
    vi.spyOn(bridge, "validateQualityRules").mockResolvedValue({
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
    const exportButton = screen.getByRole("button", { name: "Exportar CSV" });
    expect(exportButton).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Validar contrato" }));

    await waitFor(() =>
      expect(screen.getByRole("status")).toHaveTextContent("Contrato aprobado"),
    );
    expect(exportButton).toBeEnabled();
    fireEvent.click(exportButton);
    await waitFor(() => expect(onExport).toHaveBeenCalledWith({
      format: "csv",
      privacyMode: "none",
      validation: {
        kind: "contract",
        rules: [{ column: "total", kind: "not_null", maxInvalid: 0 }],
      },
    }));
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
  });

  it("permite seleccionar una política de privacidad antes de exportar", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("checkbox", {
      name: "Confirmo que quiero exportar sin validar la calidad",
    }));
    fireEvent.change(screen.getByRole("combobox", { name: "Protección de datos personales" }), {
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
    expect(screen.getByRole("textbox", { name: "Valores de referencia regla 1" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("checkbox", { name: "Columna referencial limite, regla 1" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Valores de referencia regla 1" }), {
      target: { value: "[10,12]\n[20,20]" },
    });

    expect(screen.getByRole("checkbox", { name: "Columna referencial limite, regla 1" })).toBeChecked();
    expect(screen.getByRole("textbox", { name: "Valores de referencia regla 1" })).toHaveValue("[10,12]\n[20,20]");
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
    expect(screen.getByRole("textbox", { name: "Referencias agregadas regla 1" })).toBeInTheDocument();
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

    const baseline = screen.getByRole("textbox", { name: "Línea base de distribución regla 1" });
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
    expect(screen.getByRole("textbox", { name: "Orden requerido esquema regla 1" })).toBeInTheDocument();

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

    fireEvent.change(screen.getByRole("spinbutton", { name: "Inválidos regla 1" }), {
      target: { value: "2" },
    });
    fireEvent.change(kind, { target: { value: "numeric_range" } });
    fireEvent.change(screen.getByRole("spinbutton", { name: "Mínimo regla 1" }), {
      target: { value: "1" },
    });
    fireEvent.change(screen.getByRole("spinbutton", { name: "Máximo regla 1" }), {
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
    fireEvent.change(screen.getByRole("textbox", { name: "Referencias agregadas regla 1" }), {
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
    fireEvent.change(screen.getByRole("textbox", { name: "Orden requerido esquema regla 1" }), {
      target: { value: "total\nestado" },
    });

    expect(screen.getByRole("textbox", { name: "Columnas requeridas esquema regla 1" })).toHaveValue("total\nestado");
    expect(screen.getByRole("textbox", { name: "Orden requerido esquema regla 1" })).toHaveValue("total\nestado");
  });

  it("importa reglas DataPrep, aplica las convertibles y muestra la compatibilidad", async () => {
    vi.spyOn(bridge, "pickQualityRulesMigration").mockResolvedValue({
      sourceFormat: "dataprep",
      sourceVersion: "3",
      convertedRules: [{ column: "total", kind: "not_null", maxInvalid: 0 }],
      omittedRules: 1,
      warnings: [{
        ruleIndex: 2,
        sourceKind: "column_compare",
        severity: "omitted",
        message: "La regla no tiene una representación equivalente.",
      }],
      report: {
        artifactSha256: "b".repeat(64),
        totalItems: 2,
        convertedItems: 1,
        omittedItems: 1,
        warningCount: 1,
        manualActions: [
          "Validar el contrato convertido antes de exportar.",
          "Revisar las reglas omitidas y recrearlas manualmente si siguen siendo necesarias.",
        ],
      },
    });
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    fireEvent.click(screen.getByRole("button", { name: "Importar contrato" }));

    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Importación revisada"));
    expect(screen.getByRole("status")).toHaveTextContent("1 reglas importadas");
    expect(screen.getByRole("status")).toHaveTextContent("1 omitidas");
    expect(screen.getByRole("status")).toHaveTextContent("origen DataPrep v3");
    expect(screen.getByRole("status")).toHaveTextContent("column_compare");
    expect(screen.getByRole("status")).toHaveTextContent("SHA-256 del artefacto");
    expect(screen.getByRole("status")).toHaveTextContent("Acciones manuales");
    expect(screen.getByRole("combobox", { name: "Comprobación regla 1" })).toHaveValue("not_null");
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
    expect(screen.getByRole("button", { name: "Exportar CSV" })).toBeDisabled();

    const staleContract: DeliveryContractState = {
      ...failedContract,
      gate: { kind: "stale", result },
    };
    cleanup();
    render(<DeliveryHarness onExport={onExport} initialContract={staleContract} />);
    expect(screen.getByRole("status")).toHaveTextContent("Resultado desactualizado");

    vi.restoreAllMocks();
    vi.spyOn(bridge, "validateQualityRules").mockRejectedValue(new Error("gate no disponible"));
    cleanup();
    render(<DeliveryHarness onExport={onExport} />);
    fireEvent.click(screen.getByRole("radio", { name: /^Validar calidad/ }));
    fireEvent.click(screen.getByRole("button", { name: "Validar contrato" }));
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("gate no disponible"));
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
    expect(screen.getByRole("status")).toHaveTextContent("Paquete Columnia exportado");
    expect(screen.getByRole("status")).toHaveTextContent("recipe.json validada");
    expect(screen.getByRole("status")).toHaveTextContent("Privacidad aplicada a 1 columnas: email");
    const openLastExport = vi.spyOn(bridge, "openLastExport").mockResolvedValue(undefined);
    fireEvent.click(screen.getByRole("button", { name: "Abrir carpeta de exportación" }));
    await waitFor(() => expect(openLastExport).toHaveBeenCalledOnce());
    expect(screen.getByText("Carpeta de exportación abierta.")).toBeInTheDocument();
    openLastExport.mockRejectedValueOnce(new Error("salida eliminada"));
    fireEvent.click(screen.getByRole("button", { name: "Abrir carpeta de exportación" }));
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("No se pudo abrir la carpeta de exportación."));

    cleanup();
    render(<DeliveryHarness onExport={vi.fn()} exportState={{ kind: "error", message: "disco lleno" }} />);
    expect(screen.getByRole("alert")).toHaveTextContent("disco lleno");
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
    const kind = screen.getByRole("combobox", { name: "Comprobación regla 1" });
    fireEvent.change(screen.getByRole("combobox", { name: "Tolerancia regla 1" }), { target: { value: "both" } });
    fireEvent.change(screen.getByRole("spinbutton", { name: "Porcentaje regla 1" }), { target: { value: "5" } });
    fireEvent.change(kind, { target: { value: "unique_together" } });
    fireEvent.click(screen.getByRole("checkbox", { name: "Columna compuesta limite, regla 1" }));
    fireEvent.change(screen.getByRole("combobox", { name: "Comprobación regla 1" }), { target: { value: "column_compare" } });
    fireEvent.change(screen.getByRole("combobox", { name: "Columna derecha comparar regla 1" }), { target: { value: "estado" } });
    fireEvent.click(screen.getByRole("button", { name: "Eliminar regla 1" }));
    expect(onContractAction).toHaveBeenCalled();
    expect(screen.getByText("Entrega no validada")).toBeInTheDocument();
  });
});
