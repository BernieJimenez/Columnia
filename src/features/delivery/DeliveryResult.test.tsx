import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import type { ExportResult } from "../../bridge";
import { DeliveryResult } from "./DeliveryResult";
import { INITIAL_DELIVERY_CONTRACT } from "./deliveryContract";

afterEach(cleanup);

const baseResult: ExportResult = {
  fileName: "ventas.csv",
  fileSizeBytes: 2048,
  format: "CSV",
  protectedColumnCount: 0,
  protectedColumns: [],
  replacedControlCellCount: 0,
};

function renderResult(
  result: ExportResult,
  databaseLocation: string | null = null,
  extra: { databasePolicy?: "replace"; replacedRowCount?: number | null } = {},
) {
  render(
    <DeliveryResult
      result={result}
      autoOpenPowerBi={false}
      contract={INITIAL_DELIVERY_CONTRACT}
      recipeDraft={null}
      preparationChanges={[]}
      approvedQualitySummary={null}
      databaseLocation={databaseLocation}
      {...extra}
    />,
  );
}

describe("DeliveryResult", () => {
  it("muestra las filas, la carpeta y las celdas protegidas de la copia (PROD-13)", () => {
    renderResult({ ...baseResult, rowCount: 1250, folderName: "Entregas", formulaProtectedCellCount: 1 });
    expect(screen.getByText("Filas").nextElementSibling).toHaveTextContent(`${(1250).toLocaleString("es-ES")} filas`);
    expect(screen.getByText("Filas").nextElementSibling)
      .toHaveTextContent("1 celda empezaba como una fórmula y se guardó como texto");
    expect(screen.getByText("Archivo").nextElementSibling).toHaveTextContent("ventas.csv en la carpeta Entregas");
    expect(screen.getByRole("heading", { name: "Copia lista" })).toHaveFocus();
  });

  it("no inventa cifras que el motor no dio (PROD-13)", () => {
    renderResult(baseResult);
    expect(screen.queryByText("Filas")).not.toBeInTheDocument();
    expect(screen.queryByText(/en la carpeta/)).not.toBeInTheDocument();
    expect(screen.queryByText(/fórmula/)).not.toBeInTheDocument();
  });

  it("con Reemplazar dice cuántas filas se borraron, con plural y sin inventar el recuento", () => {
    const sqlResult = { ...baseResult, fileName: "ventas", format: "SQL Server" as const, rowCount: 3 };
    renderResult(sqlResult, "db · dbo.ventas", { databasePolicy: "replace", replacedRowCount: 1 });
    expect(screen.getByRole("heading", { name: "Tabla escrita" }))
      .toHaveAccessibleDescription(/Se reemplazó la tabla anterior\. Se borró la única fila anterior\.$/);
    cleanup();
    renderResult(sqlResult, "db · dbo.ventas", { databasePolicy: "replace", replacedRowCount: 40 });
    expect(screen.getByRole("heading", { name: "Tabla escrita" }))
      .toHaveAccessibleDescription(/Se borraron las 40 filas anteriores\.$/);
    cleanup();
    renderResult(sqlResult, "db · dbo.ventas", { databasePolicy: "replace", replacedRowCount: null });
    expect(screen.getByRole("heading", { name: "Tabla escrita" }))
      .toHaveAccessibleDescription(/Se reemplazó la tabla anterior\.$/);
  });

  it("dice dónde quedó una entrega a base de datos: servidor, base y tabla", () => {
    renderResult({ ...baseResult, fileName: "ventas", format: "SQL Server", rowCount: 3 }, "db.local · Ventas · dbo.ventas");
    const heading = screen.getByRole("heading", { name: "Tabla escrita" });
    expect(heading).toHaveAccessibleDescription("Las 3 filas quedaron en db.local · Ventas · dbo.ventas.");
    expect(screen.getByText("Destino").nextElementSibling).toHaveTextContent("db.local · Ventas · dbo.ventas");
    expect(screen.queryByRole("button", { name: "Abrir carpeta" })).not.toBeInTheDocument();
  });
});
