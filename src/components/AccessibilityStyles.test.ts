import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const styles = readFileSync(join(process.cwd(), "src", "styles.css"), "utf8");

describe("contratos CSS de accesibilidad", () => {
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
