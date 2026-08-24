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
  columnCount: 1,
  columns: [{ name: "total", dataType: "Int64" }],
  rows: [["10"], ["20"]],
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
      validation: { kind: "explicitly_unvalidated" },
    });
  });
});
