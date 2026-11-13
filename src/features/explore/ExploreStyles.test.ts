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

  it("marca la selección de histograma y tendencia con un contorno sobre el relleno, también en colores forzados", () => {
    expect(css).toMatch(/\.explore__column\[aria-pressed="true"\] \.explore__fill\s*\{[^}]*outline:\s*2px solid var\(--text-primary\)/);
    expect(css).toMatch(/@media \(forced-colors: active\)[\s\S]*\.explore__column\[aria-pressed="true"\] \.explore__fill\s*\{[^}]*outline:\s*2px solid CanvasText/);
  });

  it("las columnas miden al menos 24 px (WCAG 2.5.8) y el eje se desplaza con ellas", () => {
    expect(css).toMatch(/\.explore__columns\s*\{[^}]*minmax\(24px, 1fr\)/);
    expect(/\.explore__columns\s*\{([^}]*)\}/.exec(css)?.[1]).not.toMatch(/overflow/);
    expect(css).toMatch(/\.explore__scroll\s*\{[^}]*overflow-x:\s*auto/);
  });

  it("los avisos informativos son neutros, no ámbar, y las tablas tienen tope de alto", () => {
    const info = /\.explore__info\s*\{([^}]*)\}/.exec(css)?.[1] ?? "";
    expect(info).not.toMatch(/#c98a2f|#fff7e8/i);
    expect(css).toMatch(/\.explore__table-scroll\s*\{[^}]*max-height/);
  });
});
