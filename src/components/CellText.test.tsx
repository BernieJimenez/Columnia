import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { CellText } from "./CellText";

afterEach(cleanup);

describe("CellText", () => {
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
});
