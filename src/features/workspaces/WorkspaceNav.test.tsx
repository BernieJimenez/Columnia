import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { WorkspaceNav } from "./WorkspaceNav";

describe("WorkspaceNav", () => {
  it("nombra el espacio actual sin ofrecer áreas futuras ni controles", () => {
    const { container } = render(<WorkspaceNav activePhase="prepare" />);

    expect(screen.getByText("Espacio de trabajo:", { exact: false }).closest("p")).toHaveTextContent("Espacio de trabajo: Analizar");
    expect(container).not.toHaveTextContent("Automatizar");
    expect(container).not.toHaveTextContent("Preparar para BI");
    expect(container.querySelectorAll("a, button, input, summary, [tabindex]")).toHaveLength(0);
    expect(screen.queryByRole("navigation")).not.toBeInTheDocument();
  });
});
