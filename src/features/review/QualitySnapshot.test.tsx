import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { QualitySnapshot } from "./QualitySnapshot";

afterEach(cleanup);

describe("QualitySnapshot", () => {
  it("resume completitud y señales con porcentajes comparables", () => {
    render(
      <QualitySnapshot rowCount={4} columnCount={5} nullCount={3} duplicateCount={1}
        duplicatePercentage={25} invalidTypeCount={2} />,
    );

    expect(screen.getByRole("progressbar", { name: "Completitud global" })).toHaveAttribute("aria-valuenow", "85");
    expect(screen.getByText("3 · 15.0%")).toBeInTheDocument();
    expect(screen.getByText("1 · 25.0%")).toBeInTheDocument();
    expect(screen.getByText("2 · 10.0%")).toBeInTheDocument();
  });

  it("evita porcentajes inválidos cuando no hay celdas", () => {
    render(
      <QualitySnapshot rowCount={0} columnCount={0} nullCount={0} duplicateCount={0}
        duplicatePercentage={0} invalidTypeCount={0} />,
    );

    expect(screen.getByRole("progressbar", { name: "Completitud global" })).toHaveAttribute("aria-valuenow", "100");
  });
});
