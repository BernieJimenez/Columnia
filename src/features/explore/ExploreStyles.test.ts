import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const css = readFileSync(join(process.cwd(), "src", "features", "explore", "explore.css"), "utf8");

describe("estilos de Explorar", () => {
  it("la franja de cifras no usa el fondo del color del borde como separador (AUD-071)", () => {
    const strip = /\.explore__kpis\s*\{([^}]*)\}/.exec(css)?.[1] ?? "";
    expect(strip).not.toMatch(/background:\s*var\(--border-subtle\)/);
    expect(strip).not.toMatch(/\bgap:/);
    expect(css).toMatch(/\.explore__kpi\s*\{[^}]*box-shadow:\s*0 0 0 1px var\(--border-subtle\)/);
    expect(css).toMatch(/@media \(forced-colors: active\)\s*\{[^@]*\.explore__kpi\s*\{[^}]*border-left:\s*1px solid CanvasText/);
  });

  it("la etiqueta de una barra admite dos líneas en lugar de cortarse con puntos suspensivos (AUD-070)", () => {
    const label = /\.explore__label\s*\{([^}]*)\}/.exec(css)?.[1] ?? "";
    expect(label).toMatch(/-webkit-line-clamp:\s*2/);
    expect(label).not.toMatch(/text-overflow|white-space:\s*nowrap/);
  });
});
