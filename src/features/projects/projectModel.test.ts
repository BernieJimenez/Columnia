import { describe, expect, it } from "vitest";

import type { ProjectSummary } from "../../bridge";
import { sortProjects, suggestedProjectName, validateProjectName } from "./projectModel";

const project = (overrides: Partial<ProjectSummary>): ProjectSummary => ({
  id: "project-1",
  name: "Ventas",
  datasetFileName: "ventas.csv",
  rowCount: 12,
  columnCount: 3,
  createdAt: "2026-08-20T10:00:00Z",
  updatedAt: "2026-08-20T10:00:00Z",
  ...overrides,
});

describe("projectModel", () => {
  it("recorta el nombre y aplica el contrato de 1 a 128 caracteres", () => {
    expect(validateProjectName("  Ventas agosto  ")).toEqual({ valid: true, name: "Ventas agosto" });
    expect(validateProjectName("   ")).toEqual({ valid: false, message: "Escribe un nombre para el proyecto." });
    expect(validateProjectName("x".repeat(129))).toEqual({
      valid: false,
      message: "El nombre admite hasta 128 caracteres.",
    });
  });

  it("ordena por actualización descendente sin mutar la respuesta IPC", () => {
    const original = [
      project({ id: "old", name: "Anterior", updatedAt: "2026-08-20T10:00:00Z" }),
      project({ id: "new", name: "Reciente", updatedAt: "2026-08-21T10:00:00Z" }),
    ];
    const sorted = sortProjects(original);
    expect(sorted.map(({ id }) => id)).toEqual(["new", "old"]);
    expect(original.map(({ id }) => id)).toEqual(["old", "new"]);
  });

  it("propone el nombre del archivo sin extensión y respeta el límite", () => {
    expect(suggestedProjectName("ventas.final.csv")).toBe("ventas.final");
    expect(suggestedProjectName(`${"a".repeat(140)}.csv`)).toHaveLength(128);
    expect(suggestedProjectName(null)).toBe("");
  });
});
