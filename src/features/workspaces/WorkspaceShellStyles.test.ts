// Guardas de texto: comprueban que las declaraciones existen en shell.css, no que el layout
// funcione. La comprobación medida (posiciones y desbordes reales) está en e2e/workspace-shell.spec.ts.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const shell = readFileSync(join(process.cwd(), "src", "shell.css"), "utf8");

describe("estilos del shell", () => {
  it("la barra lateral se vuelve una franja sobre el trabajo en la vista compacta y al 200 % (UX-02)", () => {
    expect(shell).toMatch(/@media \(max-width: 900px\)[\s\S]*\.app-shell\s*\{[^}]*grid-template-columns:\s*minmax\(0, 1fr\)/);
    expect(shell).toMatch(/@media \(max-width: 900px\)[\s\S]*\.app-sidebar\s*\{[^}]*position:\s*static/);
    expect(shell).toMatch(/html\[data-columnia-zoom="2"\] \.app-sidebar\s*\{[^}]*height:\s*auto/);
    expect(shell).toMatch(/html\[data-columnia-zoom="2"\] \.app-sidebar \.sidebar__tools\s*\{[^}]*grid-area:\s*tools/);
  });

  it("los nombres de etapa no se parten a media palabra (AUD-055)", () => {
    expect(shell).toMatch(/\.flow-nav__copy strong\s*\{[^}]*overflow-wrap:\s*normal/);
  });

  it("en compacto «Revisar de nuevo» se ve como punto y subrayado discontinuo, y el ✓ sobrevive bajo 700 px (AUD-065)", () => {
    expect(shell).toMatch(/\.flow-nav__item:has\(\.flow-nav__badge\)::after\s*\{[^}]*position:\s*absolute[^}]*background:\s*var\(--accent\)/);
    expect(shell).toMatch(/\.flow-nav__copy:has\(\.flow-nav__badge\) strong\s*\{[^}]*text-decoration:\s*underline dashed/);
    expect(shell).toMatch(/@media \(max-width: 700px\)\s*\{[^@]*\.flow-nav__item--complete \.flow-nav__marker\s*\{[^}]*display:\s*block[^}]*position:\s*absolute/);
  });

  it("el nombre del dataset y las columnas clave se leen enteros sin pasar el ratón (AUD-054)", () => {
    const read = (file: string) => readFileSync(join(process.cwd(), "src", file), "utf8");
    const rules: Array<[string, RegExp]> = [
      [shell, /\.dataset-bar__name\s*\{([^}]*)\}/],
      [read("styles.css"), /\.comparison-key-options strong\s*\{([^}]*)\}/],
      [read("styles.css"), /\.recent-datasets__copy strong\s*\{([^}]*)\}/],
      [read("data-views.css"), /\.sqlserver-script-target__key span\s*\{([^}]*)\}/],
    ];
    for (const [css, selector] of rules) {
      const body = css.match(selector)?.[1] ?? "";
      expect(body).toMatch(/overflow-wrap:\s*anywhere/);
      expect(body).not.toMatch(/text-overflow:\s*ellipsis/);
      expect(body).not.toMatch(/white-space:\s*nowrap/);
    }
  });

  it("la barra superior solo queda fija si hay altura, y el foco no se esconde debajo", () => {
    expect(shell).toMatch(/@media \(min-width: 901px\) and \(min-height: 701px\)\s*\{\s*html\s*\{\s*scroll-padding-top:/);
  });

  it("el estado nunca depende solo del color del punto", () => {
    expect(shell).toMatch(/\.engine-status::before,\s*\.dataset-save__status::before\s*\{/);
    expect(shell).toContain("/* The dot is the visual cue; the text always names the state. */");
  });
});
