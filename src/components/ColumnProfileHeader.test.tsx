import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { ColumnProfile } from "../bridge";
import { ColumnProfileHeader, columnProfileSignals, histogramDescription } from "./ColumnProfileHeader";

function columnProfile(overrides: Partial<ColumnProfile> = {}): ColumnProfile {
  return {
    name: "ingresos",
    dataType: "Float64",
    nullCount: 10,
    completenessPercentage: 99.2,
    uniqueCount: 1204,
    minimum: "120.5",
    maximum: "4300",
    mean: 1500,
    emptyCount: null,
    minimumLength: null,
    maximumLength: null,
    averageLength: null,
    suggestedType: null,
    typeMatchPercentage: null,
    invalidTypeCount: null,
    sentinelCount: null,
    encodingIssueCount: null,
    privacySignal: null,
    standardDeviation: null,
    firstQuartile: null,
    median: null,
    thirdQuartile: null,
    outlierCount: null,
    histogram: [{ lower: 0, upper: 10, count: 3 }, { lower: 10, upper: 20, count: 9 }, { lower: 20, upper: 30, count: 0 }],
    dateOrder: null,
    dateHasTime: null,
    untrimmedCount: null,
    ...overrides,
  };
}

const column = { name: "ingresos", dataType: "Float64" };

describe("ColumnProfileHeader", () => {
  it("sin perfil muestra solo nombre y tipo", () => {
    const { container } = render(<ColumnProfileHeader column={column} rowCount={100} />);
    expect(container).toHaveTextContent("ingresosDecimal");
    expect(container.querySelector(".col-profile")).toBeNull();
  });

  it("resume completitud, valores distintos, rango y distribución de toda la columna", () => {
    const { container } = render(<ColumnProfileHeader column={column} profile={columnProfile()} rowCount={1284} />);
    expect(screen.getByText(/con valor/)).toHaveTextContent(/99[,.]2\s?% con valor · 1[.,]?204 valores distintos/);
    expect(container.querySelector(".col-profile__range")).toHaveTextContent(/120[,.]5 – 4[.,]?300/);
    expect(container.querySelectorAll(".col-profile__histogram rect")).toHaveLength(3);
    // The bar and the histogram are decorative: the text already says it.
    expect(container.querySelector(".col-profile__fill")).toHaveAttribute("aria-hidden", "true");
    expect(container.querySelector(".col-profile__histogram")).toHaveAttribute("aria-hidden", "true");
  });

  it("dice «todos distintos» cuando cada fila tiene un valor propio", () => {
    const { container } = render(<ColumnProfileHeader column={{ name: "id", dataType: "Int64" }} profile={columnProfile({ nullCount: 0, completenessPercentage: 100, uniqueCount: 50 })} rowCount={50} />);
    expect(container).toHaveTextContent("sin nulos · todos distintos");
    expect(container.querySelector(".col-profile__histogram")).toBeNull();
  });

  it("oculta el histograma de un entero identificador con todos los valores distintos (AUD-029)", () => {
    const { container } = render(<ColumnProfileHeader column={{ name: "id", dataType: "Int64" }} profile={columnProfile({ nullCount: 0, completenessPercentage: 100, uniqueCount: 100, minimum: "1", maximum: "100" })} rowCount={100} />);
    expect(container).toHaveTextContent("todos distintos");
    expect(container.querySelector(".col-profile__histogram")).toBeNull();
  });

  it("mantiene el histograma de una columna decimal continua con todos los valores distintos (AUD-029)", () => {
    for (const dataType of ["Float64", "f64", "Decimal(10, 2)"]) {
      const { container, unmount } = render(<ColumnProfileHeader column={{ name: "importe", dataType }} profile={columnProfile({ nullCount: 0, completenessPercentage: 100, uniqueCount: 100 })} rowCount={100} />);
      expect(container).toHaveTextContent("todos distintos");
      expect(container.querySelectorAll(".col-profile__histogram rect")).toHaveLength(3);
      unmount();
    }
  });

  it("concuerda en singular con un solo valor distinto (TXT-05)", () => {
    const { container } = render(<ColumnProfileHeader column={column} profile={columnProfile({ uniqueCount: 1, minimum: "5", maximum: "5", histogram: null })} rowCount={3} />);
    expect(container).toHaveTextContent(/1 valor distinto/);
    expect(container.querySelector(".col-profile__range")).toBeNull();
  });

  it("nombra las señales de calidad con texto", () => {
    const profile = columnProfile({ invalidTypeCount: 4, untrimmedCount: 37, sentinelCount: 1, encodingIssueCount: 2 });
    expect(columnProfileSignals(profile)).toEqual([
      "4 con otro tipo",
      "37 con espacios",
      "1 marcador «sin dato»",
      "2 con acentos mal leídos",
    ]);
    render(<ColumnProfileHeader column={column} profile={profile} rowCount={100} />);
    expect(screen.getByText(/37 con espacios/)).toHaveClass("col-profile__signal");
  });

  it("describe la forma del histograma con un texto oculto dentro del bloque del perfil (AUD-052)", () => {
    const { container } = render(<ColumnProfileHeader column={column} profile={columnProfile()} profileId="perfil-1" rowCount={1284} />);
    const block = container.querySelector(".col-profile");
    expect(block).toHaveAttribute("id", "perfil-1");
    const hidden = block?.querySelector(".visually-hidden");
    expect(hidden).toHaveTextContent("Distribución: de 0 a 30, la mayoría entre 10 y 20.");
  });

  it("no describe la distribución de un identificador ni de un histograma vacío (AUD-052)", () => {
    const { container } = render(<ColumnProfileHeader column={{ name: "id", dataType: "Int64" }} profile={columnProfile({ nullCount: 0, completenessPercentage: 100, uniqueCount: 50 })} rowCount={50} />);
    expect(container.querySelector(".visually-hidden")).toBeNull();
    expect(histogramDescription([{ lower: 0, upper: 1, count: 0 }, { lower: 1, upper: 2, count: 0 }])).toBeNull();
  });
});
