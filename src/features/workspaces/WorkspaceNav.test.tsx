import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { WorkspaceNav } from "./WorkspaceNav";

describe("WorkspaceNav", () => {
  it("muestra solo el espacio actual y no ofrece áreas futuras sin acción", () => {
    render(<WorkspaceNav activePhase="prepare" />);

    const region = screen.getByRole("region", { name: "Espacio actual" });

    expect(region).toHaveTextContent("Analizar · En uso");
    expect(region).not.toHaveTextContent("Automatizar");
    expect(region).not.toHaveTextContent("Preparar para BI");
    expect(region.querySelectorAll("a, button, input, summary, [tabindex]")).toHaveLength(0);
    expect(screen.queryByRole("navigation", { name: "Espacios de trabajo" })).not.toBeInTheDocument();
  });
});
