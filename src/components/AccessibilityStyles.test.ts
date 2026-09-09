import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const styles = readFileSync(join(process.cwd(), "src", "styles.css"), "utf8");

describe("contratos CSS de accesibilidad", () => {
  it("ajusta la barra lateral y sus paneles a la resolución disponible", () => {
    const sidebarRule = styles.match(/\.sidebar\s*\{([^}]*)\}/)?.[1] ?? "";

    expect(sidebarRule).toContain("height: 100dvh");
    expect(sidebarRule).toContain("overflow-y: visible");
    expect(styles).toMatch(/@media \(min-width: 901px\)[\s\S]*\.sidebar__utilities-content,[\s\S]*\.sidebar__legal-content\s*\{[^}]*position:\s*fixed/);
    expect(styles).toMatch(/\.sidebar__utilities-content,[\s\S]*\.sidebar__legal-content\s*\{[^}]*inset:\s*16px auto/);
    expect(styles).toMatch(/\.sidebar__utilities-content,[\s\S]*\.sidebar__legal-content\s*\{[^}]*overflow-y:\s*auto/);
    expect(styles).toMatch(/body:has\(\.sidebar__tools > details\[open\]\)\s*\{\s*overflow-y:\s*hidden/);
    expect(styles).toContain("@media (min-width: 901px) and (max-height: 700px)");
  });

  it("mantiene targets táctiles mínimos y respeta reduced motion", () => {
    expect(styles).toMatch(/\.recipe-add\s*\{[^}]*min-height:\s*24px/);
    expect(styles).toContain("@media (prefers-reduced-motion: reduce)");
    expect(styles).toMatch(/animation-duration:\s*0\.01ms\s*!important/);
    expect(styles).toMatch(/transition-duration:\s*0\.01ms\s*!important/);
  });

  it("ofrece una paleta explícita para forced-colors/alto contraste", () => {
    expect(styles).toContain("@media (forced-colors: active)");
    expect(styles).toContain("background: Canvas");
    expect(styles).toContain("background: Highlight");
    expect(styles).toContain("color: HighlightText");
    expect(styles).toContain("forced-color-adjust: none");
    expect(styles).toMatch(/outline:\s*3px solid Highlight/);
  });
});
