import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { THEME_STORAGE_KEY } from "../features/settings/themeModel";
import { ThemeSwitcher } from "./ThemeSwitcher";

afterEach(() => cleanup());

describe("paletas adicionales del selector de tema", () => {
  beforeEach(() => window.localStorage.clear());

  it.each([
    ["Papel", "paper", "Tonos cálidos para sesiones largas"],
    ["Océano", "ocean", "Azules frescos y definidos"],
    ["Pizarra", "slate", "Neutros sobrios con acento azul"],
  ])("aplica y conserva la paleta %s", (label, value, description) => {
    render(<ThemeSwitcher />);

    fireEvent.click(screen.getByRole("button", { name: label }));

    expect(document.documentElement.dataset.theme).toBe(value);
    expect(document.documentElement.style.colorScheme).toBe("light");
    expect(window.localStorage.getItem(THEME_STORAGE_KEY)).toBe(value);
    expect(screen.getByText(description)).toBeInTheDocument();
  });
});
