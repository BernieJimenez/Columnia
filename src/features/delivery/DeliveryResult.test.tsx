import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { DeliveryResult } from "./DeliveryResult";
import { INITIAL_DELIVERY_CONTRACT } from "./deliveryModel";

afterEach(cleanup);

const baseResult = {
  fileName: "ventas.csv",
  fileSizeBytes: 2048,
  format: "CSV" as const,
  protectedColumnCount: 0,
  protectedColumns: [],
  replacedControlCellCount: 0,
};

function renderResult(result: typeof baseResult & { rowCount?: number; folderName?: string; formulaProtectedCellCount?: number }) {
  render(
    <DeliveryResult
      result={result}
      autoOpenPowerBi={false}
      contract={INITIAL_DELIVERY_CONTRACT}
      recipeDraft={null}
      preparationChanges={[]}
      approvedQualitySummary={null}
    />,
  );
}

describe("DeliveryResult", () => {
  it("muestra las filas, la carpeta y las celdas protegidas de la copia (PROD-13)", () => {
    renderResult({ ...baseResult, rowCount: 1250, folderName: "Entregas", formulaProtectedCellCount: 1 });
    expect(screen.getByText("Filas exportadas").nextElementSibling).toHaveTextContent((1250).toLocaleString());
    expect(screen.getByText("Carpeta").nextElementSibling).toHaveTextContent("Entregas");
    expect(screen.getByText("Celdas protegidas").nextElementSibling)
      .toHaveTextContent("1 celda empezaba como una fórmula y se guardó como texto");
  });

  it("no inventa cifras que el motor no dio (PROD-13)", () => {
    renderResult(baseResult);
    expect(screen.queryByText("Filas exportadas")).not.toBeInTheDocument();
    expect(screen.queryByText("Carpeta")).not.toBeInTheDocument();
    expect(screen.queryByText("Celdas protegidas")).not.toBeInTheDocument();
  });
});
