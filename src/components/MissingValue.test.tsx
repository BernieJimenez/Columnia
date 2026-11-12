import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderCellValue } from "./CellText";
import { MissingValue } from "./MissingValue";

describe("MissingValue", () => {
  it("marca el nulo con símbolo decorativo y title, sin duplicar el nombre accesible", () => {
    const { container } = render(<MissingValue />);
    const root = container.querySelector(".null-value");
    expect(root).toHaveAttribute("title", "Sin valor");
    const decorative = Array.from(root?.querySelectorAll("[aria-hidden='true']") ?? []);
    expect(decorative.map((node) => node.textContent).join("")).toContain("∅");
    expect(screen.getAllByText("valor ausente")).toHaveLength(1);
  });

  it("una celda con el texto literal «vacío» no lleva marcador ni title", () => {
    const { container } = render(<>{renderCellValue("vacío")}</>);
    expect(container).toHaveTextContent("vacío");
    expect(container).not.toHaveTextContent("∅");
    expect(container.querySelector(".null-value")).toBeNull();
    expect(container.querySelector("[title]")).toBeNull();
  });
});
