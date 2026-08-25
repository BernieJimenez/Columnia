import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

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

  it("recupera una selección guardada al montar", () => {
    window.localStorage.setItem(THEME_STORAGE_KEY, "light");

    render(<ThemeSwitcher />);

    expect(screen.getByRole("button", { name: "Claro" })).toHaveAttribute("aria-pressed", "true");
    expect(document.documentElement.dataset.theme).toBe("light");
  });
});
