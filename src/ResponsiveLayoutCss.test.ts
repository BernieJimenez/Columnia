import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const read = (file: string) => readFileSync(join(process.cwd(), file), "utf8");
const shell = read("src/shell.css");
const dataViews = read("src/data-views.css");
const styles = read("src/styles.css");

describe("diseño a 320 px (AUD-055)", () => {
  it("las ramas de una columna usan minmax(0, 1fr)", () => {
    expect(shell).toMatch(/\.delivery-flow \{[^}]*grid-template-columns: minmax\(0, 1fr\)/);
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
    expect(styles).toMatch(/\.quality-temporal__table \{[^}]*position: relative/);
    expect(styles).toMatch(/\.quality-temporal-trend__table \{[^}]*position: relative/);
  });
});

describe("Revisar a 320 px", () => {
  it("la etiqueta de la distribución numérica envuelve: nombre legible y Q1/Mediana/Q3 en --text-secondary", () => {
    expect(styles).toMatch(/\.quality-chart__label--stats \{[^}]*flex-wrap: wrap/);
    expect(styles).toMatch(/\.quality-chart__label--stats span \{[^}]*flex: 1 1 auto;[^}]*white-space: normal;[^}]*overflow-wrap: anywhere/);
    expect(styles).toMatch(/max-width: 520px\) \{\s*\.quality-chart__label--stats strong \{ flex-basis: 100%/);
    expect(styles).toMatch(/\.quality-chart__label--stats strong \{[^}]*color: var\(--text-secondary\)/);
    expect(styles).toMatch(/\.quality-chart__label--stats strong \{ color: var\(--text-secondary\)/);
    expect(styles).not.toMatch(/\.quality-chart__label--stats[^{]*\{[^}]*var\(--accent\)/);
  });
  it("«Más análisis» no pone una caja dentro del plegable y el resultado de la comparación va en línea", () => {
    expect(dataViews).toMatch(/\.review-more__section > \.quality-visuals \{[^}]*border: 0/);
    expect(read("src/workflow-styles.css")).toMatch(/\.comparison-counts \{ display: flex; flex-wrap: wrap/);
  });
});

describe("Entregar en una sola columna (2026-11-14)", () => {
  it("los pasos van en una columna de lectura, sin rejilla de dos columnas ni barra fija", () => {
    expect(shell).toMatch(/\.delivery-flow \{[^}]*max-width: 760px/);
    expect(shell).not.toMatch(/delivery-layout/);
    expect(shell).not.toMatch(/\.delivery-commit \{[^}]*position: sticky/);
    expect(shell).not.toMatch(/--delivery-commit-height/);
  });
});

describe("columna de trabajo de Preparar con el historial al lado", () => {
  const workflow = read("src/workflow-styles.css");
  it("la columna es un contenedor y sus filas responden a su ancho, no al de la ventana", () => {
    expect(dataViews).toMatch(/\.prepare-layout__main \{[^}]*container: prepare-main \/ inline-size/);
    // A narrow column puts each row's button under its own text, full width.
    expect(styles).toMatch(/@container prepare-main \(max-width: 36rem\) \{[^@]*\.cleaning-signals__column-text \{ flex-basis: 100%;/);
    expect(styles).toMatch(/@container prepare-main \(max-width: 36rem\) \{[^@]*\.cleaning-signals__action-button \{ flex-basis: 100%; width: 100%;/);
  });
  it("una fila por columna: el texto guarda su ancho y el botón pasa debajo, sin base flex que se vuelva altura", () => {
    expect(styles).toMatch(/\.cleaning-signals__column \{[^}]*display: flex;[^}]*flex-wrap: wrap/);
    expect(styles).toMatch(/\.cleaning-signals__column-text \{[^}]*flex: 1 1 26rem/);
    // Always a row: in a column the 26rem basis would become a height.
    expect(styles).not.toMatch(/\.cleaning-signals__column \{[^}]*flex-direction: column/);
  });
  it("el primario de la propuesta mientras se aplica conserva su color, no el contorno discontinuo", () => {
    expect(workflow).toMatch(/\.workspace \.prepare-proposal__primary\[aria-disabled="true"\]\[aria-busy="true"\] \{[^}]*background: var\(--accent\);[^}]*color: var\(--on-accent\)/);
    // The text of a row is .875rem in --text-secondary, never .78rem.
    expect(styles).toMatch(/\.cleaning-signals__column-text p \{[^}]*color: var\(--text-secondary\); font-size: \.875rem/);
    expect(styles).not.toMatch(/cleaning-signals[^{]*\{[^}]*font-size: \.78rem/);
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
