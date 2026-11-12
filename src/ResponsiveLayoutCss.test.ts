import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const read = (file: string) => readFileSync(join(process.cwd(), file), "utf8");
const shell = read("src/shell.css");
const dataViews = read("src/data-views.css");
const styles = read("src/styles.css");

describe("diseño a 320 px (AUD-055)", () => {
  it("las ramas de una columna usan minmax(0, 1fr)", () => {
    expect(shell).toMatch(/\.delivery-layout \{[^}]*grid-template-columns: minmax\(0, 1fr\)/);
    expect(dataViews).toMatch(/\.prepare-layout \{[^}]*grid-template-columns: minmax\(0, 1fr\)/);
    expect(dataViews).toMatch(/\.database-import \{[^}]*grid-template-columns: minmax\(0, 1fr\)/);
  });
  it("los fieldset de Preparar pueden encogerse", () => {
    expect(styles).toMatch(/\.text-cleaning-options fieldset \{[^}]*min-width: 0/);
    expect(styles).toMatch(/\.transform-recipe fieldset \{[^}]*min-width: 0/);
  });
  it("las etapas no parten palabras y el servidor pasa a una columna", () => {
    expect(shell).toMatch(/\.flow-nav__copy strong \{[^}]*overflow-wrap: normal/);
    expect(shell).toMatch(/max-width: 480px\) \{\s*\.connection-form__row--server \{ grid-template-columns: minmax\(0, 1fr\)/);
  });
});

describe("barra superior de alto variable (AUD-058)", () => {
  it("no quedan 88 px fijos sin la variable", () => {
    expect(shell).toContain("scroll-padding-top: var(--topbar-height, 88px)");
    expect(dataViews).not.toMatch(/top: 88px/);
    expect(dataViews).toContain("var(--topbar-height, 88px)");
  });
});
