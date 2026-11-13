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
  it("los spans .visually-hidden de las tablas quedan dentro de su región con scroll", () => {
    expect(styles).toMatch(/\.table-region \{[^}]*position: relative/);
    expect(styles).toMatch(/\.profile-region \{[^}]*position: relative/);
    expect(styles).toMatch(/\.quality-chart__table \{[^}]*position: relative/);
  });
});

describe("columna de trabajo de Preparar con el historial al lado", () => {
  const workflow = read("src/workflow-styles.css");
  it("la columna es un contenedor y sus filas responden a su ancho, no al de la ventana", () => {
    expect(dataViews).toMatch(/\.prepare-layout__main \{[^}]*container: prepare-main \/ inline-size/);
    expect(styles).toMatch(/@container prepare-main \(max-width: 40rem\) \{[^@]*\.cleaning-signals__action,\s*\.missing-data-plan__steps li \{ align-items: stretch; flex-direction: column; \}/);
  });
  it("el texto de una señal conserva 22rem y los botones bajan de línea", () => {
    expect(styles).toMatch(/\.cleaning-signals__action \{[^}]*flex-wrap: wrap/);
    expect(styles).toMatch(/\.cleaning-signals__action p \{[^}]*flex: 1 1 22rem/);
    expect(styles).toMatch(/\.missing-data-plan__steps li \{[^}]*flex-wrap: wrap/);
    expect(styles).toMatch(/\.missing-data-plan__steps li > div:first-child \{[^}]*flex: 1 1 22rem/);
  });
  it("el título de un plegable guarda 12rem y su nota pasa debajo en una columna estrecha", () => {
    expect(workflow).toMatch(/grid-template-columns: minmax\(12rem, 1fr\) minmax\(0, max-content\) 24px/);
    expect(workflow).toMatch(/@container prepare-main \(max-width: 46rem\) \{[\s\S]*?> small \{\s*grid-column: 1;\s*grid-row: 2;/);
  });
  it("las decisiones en bloque de Combinar usan tokens del tema", () => {
    const bulk = styles.match(/\.conflict-resolution__bulk button[^{]*\{[^}]*\}/g)?.join("\n") ?? "";
    expect(bulk).not.toMatch(/#[0-9a-f]{3,6}/i);
    expect(bulk).toContain("var(--accent)");
  });
});

describe("barra superior de alto variable (AUD-058)", () => {
  it("no quedan 88 px fijos sin la variable", () => {
    expect(shell).toContain("scroll-padding-top: var(--topbar-height, 88px)");
    expect(dataViews).not.toMatch(/top: 88px/);
    expect(dataViews).toContain("var(--topbar-height, 88px)");
  });
});
