import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const styles = readFileSync(join(process.cwd(), "src", "data-views.css"), "utf8");

describe("contratos CSS del perfil de cabecera (AUD-068)", () => {
  it("fija una altura mínima común al bloque, también en densidad compacta", () => {
    const block = styles.match(/\.col-profile\s*\{([^}]*)\}/)?.[1] ?? "";
    expect(block).toContain("min-height: calc(33px + 4.05 * .75rem)");
    expect(block).toContain("align-content: start");
    expect(styles).toMatch(/html\[data-density="compact"\] \.col-profile\s*\{\s*min-height: calc\(30px \+ 4\.05 \* \.75rem\)/);
  });

  it("el esqueleto no anima y la nota pendiente conserva su sitio", () => {
    expect(styles).not.toMatch(/col-profile--pending[^{]*\{[^}]*animation/);
    expect(styles).toMatch(/\.preview-note--pending\s*\{\s*visibility: hidden;\s*\}/);
  });
});
