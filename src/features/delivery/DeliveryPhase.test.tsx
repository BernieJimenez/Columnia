import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as bridge from "../../bridge";
import type { DatasetPreview } from "../../bridge";
import { DeliveryPhase } from "./DeliveryPhase";
import {
  INITIAL_DELIVERY_CONTRACT,
  reduceDeliveryContract,
  type DeliveryContractState,
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

function DeliveryHarness({ onExport }: { onExport: (request: DeliveryExportRequest) => void }) {
  const [contract, setContract] = useState<DeliveryContractState>(INITIAL_DELIVERY_CONTRACT);
  return (
    <DeliveryPhase
      dataset={dataset}
      contract={contract}
      exportState={{ kind: "idle" }}
      onContractAction={(action) => setContract((current) => reduceDeliveryContract(current, action))}
      onExport={onExport}
      onCancelExport={() => undefined}
    />
  );
}

describe("DeliveryPhase", () => {
  it("exige confirmación explícita antes de exportar sin contrato", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    const exportButton = screen.getByRole("button", { name: "Exportar Parquet" });
    expect(exportButton).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox", {
      name: "Entiendo y deseo exportar sin contrato de calidad",
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

    fireEvent.click(screen.getByRole("checkbox", { name: "Validar antes de exportar" }));
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
      name: "Entiendo y deseo exportar sin contrato de calidad",
    }));
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
      name: "Entiendo y deseo exportar sin contrato de calidad",
    }));
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
      name: "Entiendo y deseo exportar sin contrato de calidad",
    }));
    fireEvent.click(screen.getByRole("button", { name: "Exportar Excel" }));
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

  it("permite seleccionar una política de privacidad antes de exportar", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("checkbox", {
      name: "Entiendo y deseo exportar sin contrato de calidad",
    }));
    fireEvent.change(screen.getByRole("combobox", { name: "Protección de datos personales" }), {
      target: { value: "hash" },
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

    fireEvent.click(screen.getByRole("checkbox", { name: "Validar antes de exportar" }));
    fireEvent.change(screen.getByRole("combobox", { name: "Comprobación regla 1" }), {
      target: { value: "allowed_values" },
    });

    expect(screen.getByRole("textbox", { name: "Valores permitidos regla 1" })).toBeInTheDocument();
    expect(screen.getByText("Un valor por línea; se compara sin transformar.")).toBeInTheDocument();
  });

  it("expone operadores y dos columnas para comparar columnas", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("checkbox", { name: "Validar antes de exportar" }));
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

  it("expone límites de fecha para date_range", () => {
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("checkbox", { name: "Validar antes de exportar" }));
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

    fireEvent.click(screen.getByRole("checkbox", { name: "Validar antes de exportar" }));
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

    fireEvent.click(screen.getByRole("checkbox", { name: "Validar antes de exportar" }));
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

  it("importa reglas DataPrep, aplica las convertibles y muestra las omitidas", async () => {
    vi.spyOn(bridge, "pickQualityRulesMigration").mockResolvedValue({
      sourceVersion: "3",
      convertedRules: [{ column: "total", kind: "not_null", maxInvalid: 0 }],
      omittedRules: 1,
      warnings: [{
        ruleIndex: 2,
        sourceKind: "column_compare",
        severity: "omitted",
        message: "La regla no tiene una representación equivalente.",
      }],
    });
    const onExport = vi.fn();
    render(<DeliveryHarness onExport={onExport} />);

    fireEvent.click(screen.getByRole("checkbox", { name: "Validar antes de exportar" }));
    fireEvent.click(screen.getByRole("button", { name: "Importar reglas DataPrep" }));

    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Importación revisada"));
    expect(screen.getByRole("status")).toHaveTextContent("1 reglas convertidas");
    expect(screen.getByRole("status")).toHaveTextContent("1 omitidas");
    expect(screen.getByRole("status")).toHaveTextContent("column_compare");
    expect(screen.getByRole("combobox", { name: "Comprobación regla 1" })).toHaveValue("not_null");
  });
});
