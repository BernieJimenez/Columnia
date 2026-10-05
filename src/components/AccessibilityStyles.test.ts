import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const baseStyles = readFileSync(join(process.cwd(), "src", "styles.css"), "utf8");
const workflowStyles = readFileSync(join(process.cwd(), "src", "workflow-styles.css"), "utf8");
const styles = `${baseStyles}\n${workflowStyles}`;

describe("contratos CSS de accesibilidad", () => {
  it("carga el bloque visual diferido antes de renderizar React", () => {
    const entrypoint = readFileSync(join(process.cwd(), "src", "main.tsx"), "utf8");
    const styleModule = readFileSync(join(process.cwd(), "src", "workflow-styles.ts"), "utf8");
    const styleImport = 'await import("./workflow-styles")';

    expect(entrypoint).toContain(styleImport);
    expect(entrypoint.indexOf(styleImport)).toBeLessThan(entrypoint.indexOf("createRoot(appRoot).render"));
    expect(styleModule.trim()).toBe('import "./workflow-styles.css";');
  });

  it("ajusta la barra lateral y sus paneles a la resolución disponible", () => {
    const sidebarRule = styles.match(/\.sidebar\s*\{([^}]*)\}/)?.[1] ?? "";

    expect(sidebarRule).toContain("height: 100dvh");
    expect(sidebarRule).toContain("overflow-y: visible");
    expect(styles).toMatch(/@media \(min-width: 901px\)[\s\S]*\.sidebar__utilities-content,[\s\S]*\.sidebar__legal-content\s*\{[^}]*position:\s*fixed/);
    expect(styles).toMatch(/\.sidebar__utilities-content,[\s\S]*\.sidebar__legal-content\s*\{[^}]*top:\s*16px/);
    expect(styles).toMatch(/\.sidebar__utilities-content,[\s\S]*\.sidebar__legal-content\s*\{[^}]*max-height:\s*calc\(100dvh - 32px\)/);
    expect(styles).toMatch(/\.sidebar__utilities-content,[\s\S]*\.sidebar__legal-content\s*\{[^}]*overflow-y:\s*auto/);
    expect(styles).toMatch(/body:has\(\.sidebar__tools > details\[open\]\)\s*\{\s*overflow-y:\s*hidden/);
    expect(styles).toContain("@media (min-width: 901px) and (max-height: 700px)");
  });

  it("mantiene targets táctiles mínimos y respeta reduced motion", () => {
    expect(styles).toMatch(/\.recipe-add\s*\{[^}]*min-height:\s*44px/);
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

  // ACC-03/ACC-07/ACC-02: each theme's control edge reaches 3:1 on its
  // surfaces, and text tokens used on the skip link and chart labels 4.5:1.
  it("los bordes de controles y el skip link tienen contraste suficiente en cada tema", () => {
    const luminance = (hex: string) => {
      const channels = [1, 3, 5].map((index) => parseInt(hex.slice(index, index + 2), 16) / 255)
        .map((value) => (value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4));
      return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2];
    };
    const ratio = (left: string, right: string) => {
      const [high, low] = [luminance(left), luminance(right)].sort((a, b) => b - a);
      return (high + 0.05) / (low + 0.05);
    };
    const blocks = [...baseStyles.matchAll(/(?:^|\n)\s*(:root(?:\[data-[a-z-]+="[a-z]+"\])?(?: \.sidebar)?)\s*\{([^}]*)\}/g)]
      .map(([, selector, body]) => ({ selector, tokens: Object.fromEntries([...body.matchAll(/--([a-z-]+):\s*(#[0-9a-f]{6})/g)].map(([, name, value]) => [name, value])) }))
      .filter(({ tokens }) => tokens["control-border"]);
    expect(blocks.length).toBeGreaterThanOrEqual(8);
    for (const { selector, tokens } of blocks) {
      for (const surface of ["canvas", "surface", "surface-subtle"]) {
        if (tokens[surface]) expect(ratio(tokens["control-border"], tokens[surface]), `${selector} ${surface}`).toBeGreaterThanOrEqual(3);
        // ACC-08: destructive actions use the theme's danger colour as text.
        if (tokens[surface] && tokens.danger) expect(ratio(tokens.danger, tokens[surface]), `${selector} danger`).toBeGreaterThanOrEqual(4.5);
        // ACC-16: small accent text (links, sample descriptions) on every surface.
        if (tokens[surface] && tokens.accent) expect(ratio(tokens.accent, tokens[surface]), `${selector} accent`).toBeGreaterThanOrEqual(4.5);
      }
      if (tokens.canvas && tokens["text-primary"]) expect(ratio(tokens.canvas, tokens["text-primary"])).toBeGreaterThanOrEqual(4.5);
    }
    expect(styles).toMatch(/\.skip-link \{[^}]*background: var\(--text-primary\); color: var\(--canvas\)/);
    expect(styles).not.toMatch(/#93473f/);
    // ACC-15: the updater error uses the sidebar's light danger colour.
    expect(styles).toMatch(/\.update-panel__status--error \{ color: var\(--danger\)/);
    expect(styles).toMatch(/\.sidebar \{[^}]*--danger: #f4a39b/);
    expect(styles).not.toMatch(/quality-temporal-line__label \{[^}]*fill: var\(--border-subtle\)/);
  });
});
