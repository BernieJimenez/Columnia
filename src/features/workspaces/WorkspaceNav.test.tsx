import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { WorkspaceNav } from "./WorkspaceNav";

describe("WorkspaceNav", () => {
  it("expone una lista informativa, no una segunda navegación", () => {
    render(<WorkspaceNav activePhase="prepare" />);

    const region = screen.getByRole("region", { name: "Espacios de trabajo" });
    const list = within(region).getByRole("list");
    const items = within(list).getAllByRole("listitem");

    expect(items).toHaveLength(3);
    expect(items[0]).toHaveTextContent("AnalizarActual");
    expect(items[0]).toHaveAttribute("aria-current", "true");
    expect(items[1]).toHaveTextContent("AutomatizarCLI disponible · Interfaz en preparación");
    expect(items[2]).toHaveTextContent("Preparar para BIInterfaz planificada");
    expect(region.querySelectorAll("a, button, input, summary, [tabindex]")).toHaveLength(0);
    expect(screen.queryByRole("navigation", { name: "Espacios de trabajo" })).not.toBeInTheDocument();
  });
});
