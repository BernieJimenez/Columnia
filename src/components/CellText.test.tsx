import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { CellText, renderCellValue } from "./CellText";

afterEach(cleanup);

describe("CellText", () => {
  it("usa el singular con un solo espacio (TXT-10)", () => {
    const { container } = render(<CellText value=" " />);
    expect(container.querySelector(".visually-hidden")?.textContent).toBe("1 espacio");
  });

  it("deja intacto el texto sin espacios exteriores", () => {
    const { container } = render(<CellText value="Ana Pérez" />);
    expect(container.textContent).toBe("Ana Pérez");
    expect(container.querySelector(".whitespace-marker")).toBeNull();
  });

  it("marca los espacios iniciales y finales y los anuncia", () => {
    const { container } = render(<CellText value="  María Núñez " />);
    const markers = [...container.querySelectorAll(".whitespace-marker")].map((marker) => marker.textContent);
    expect(markers).toEqual(["··", "·"]);
    expect(container.querySelector(".visually-hidden")?.textContent).toContain("espacios al inicio o al final");
  });

  it("representa una celda formada solo por espacios", () => {
    const { container } = render(<CellText value="   " />);
    expect(container.querySelector(".whitespace-marker")?.textContent).toBe("···");
    expect(container.querySelector(".visually-hidden")?.textContent).toBe("3 espacios");
  });

  it("limita los textos largos y deja el valor completo en title y en foco", () => {
    const long = "x".repeat(2000);
    const { container } = render(<>{renderCellValue(long)}</>);
    const clip = container.querySelector<HTMLElement>(".cell-clip");
    expect(clip?.getAttribute("title")).toBe(long);
    expect(clip?.tabIndex).toBe(0);
  });

  it("no envuelve los valores cortos", () => {
    const { container } = render(<>{renderCellValue("Ana")}</>);
    expect(container.querySelector(".cell-clip")).toBeNull();
  });
});
