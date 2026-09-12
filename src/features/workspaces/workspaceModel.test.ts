import { describe, expect, it } from "vitest";

import { workflowPhaseIds, workflowPhases, workspaceByPhase, workspaceForPhase, workspaces } from "./workspaceModel";

describe("modelo de espacios de trabajo", () => {
  it("usa descriptores centrales para las cuatro fases durables", () => {
    expect(workflowPhases.map(({ id }) => id)).toEqual(["load", "review", "prepare", "deliver"]);
    expect(workflowPhaseIds).toEqual(workflowPhases.map(({ id }) => id));
  });

  it("mapea exhaustivamente las cuatro fases actuales a Analizar", () => {
    expect(Object.keys(workspaceByPhase)).toEqual(workflowPhaseIds);

    for (const phase of workflowPhaseIds) {
      expect(workspaceForPhase(phase)).toBe("analyze");
    }
  });

  it("mantiene la disponibilidad y el copy aprobado de cada espacio", () => {
    expect(workspaces).toEqual([
      { id: "analyze", label: "Analizar", status: "Actual", available: true },
      {
        id: "automate",
        label: "Automatizar",
        status: "CLI disponible · Interfaz en preparación",
        available: false,
      },
      {
        id: "bi",
        label: "Preparar para BI",
        status: "Interfaz planificada",
        available: false,
      },
    ]);
  });
});
