import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { localeText } from "../../test/localeText";
import { QualitySnapshot } from "./QualitySnapshot";

afterEach(cleanup);

describe("QualitySnapshot", () => {
  it("dice la completitud global con su cifra, como texto y no como barra de progreso", () => {
    render(<QualitySnapshot rowCount={4} columnCount={5} nullCount={3} />);

    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
    expect(screen.getByText(localeText("85.0%"), { exact: false })).toHaveTextContent("de las celdas tienen valor");
    expect(screen.getByText(/3 nulos en 20 celdas/)).toBeInTheDocument();
  });

  it("evita porcentajes inválidos cuando no hay celdas", () => {
    render(<QualitySnapshot rowCount={0} columnCount={0} nullCount={0} />);

    expect(screen.getByText(localeText("100.0%"), { exact: false })).toBeInTheDocument();
    expect(screen.queryByText(/celdas\)/)).not.toBeInTheDocument();
  });
});
