import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { DENSITY_STORAGE_KEY } from "../features/settings/densityModel";
import { THEME_STORAGE_KEY } from "../features/settings/themeModel";
import { ThemeSwitcher } from "./ThemeSwitcher";

afterEach(() => cleanup());

describe("ThemeSwitcher", () => {
  beforeEach(() => window.localStorage.clear());

  it("muestra los tres modos y conserva la selección", () => {
    render(<ThemeSwitcher />);

    expect(screen.getByRole("region", { name: "Tema" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Sistema" })).toHaveAttribute("aria-pressed", "true");

    fireEvent.click(screen.getByRole("button", { name: "Oscuro" }));

    expect(screen.getByRole("button", { name: "Oscuro" })).toHaveAttribute("aria-pressed", "true");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(window.localStorage.getItem(THEME_STORAGE_KEY)).toBe("dark");
    expect(screen.getByText("Panel de baja luz")).toBeInTheDocument();
  });

  it("cambia a la densidad compacta y la recuerda", () => {
    render(<ThemeSwitcher />);
    const density = screen.getByRole("group", { name: "Densidad" });
    expect(screen.getByRole("button", { name: "Normal" })).toHaveAttribute("aria-pressed", "true");

    fireEvent.click(screen.getByRole("button", { name: "Compacta" }));

    expect(density).toContainElement(screen.getByRole("button", { name: "Compacta" }));
    expect(screen.getByRole("button", { name: "Compacta" })).toHaveAttribute("aria-pressed", "true");
    expect(document.documentElement.dataset.density).toBe("compact");
    expect(window.localStorage.getItem(DENSITY_STORAGE_KEY)).toBe("compact");
    expect(screen.getByText(/Más filas y columnas a la vista/)).toBeInTheDocument();
  });

  it("recupera una selección guardada al montar", () => {
    window.localStorage.setItem(THEME_STORAGE_KEY, "light");

    render(<ThemeSwitcher />);

    expect(screen.getByRole("button", { name: "Claro" })).toHaveAttribute("aria-pressed", "true");
    expect(document.documentElement.dataset.theme).toBe("light");
  });
});
