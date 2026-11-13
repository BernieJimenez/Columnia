import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";

import { plural } from "./plural";
import type { DatasetProfile } from "./bridge";
import { buildUncorrectedIssues } from "./features/review/qualityActionPlan";

describe("plural", () => {
  it("usa el singular solo para 1", () => {
    expect(plural(0, "fila", "filas")).toBe("0 filas");
    expect(plural(1, "fila", "filas")).toBe("1 fila");
    expect(plural(2, "fila", "filas")).toBe("2 filas");
  });

  it("«No se corrige solo» concuerda con un solo caso (TXT-05)", () => {
    const one = {
      name: "a", dataType: "String", nullCount: 1, invalidTypeCount: 1, privacySignal: "email", emptyCount: 0,
    } as unknown as DatasetProfile["columns"][number];
    const text = buildUncorrectedIssues({
      rowCount: 1, duplicateRowCount: 0, nearDuplicateRowCount: 0, duplicatePercentage: 0, columns: [one],
    } as DatasetProfile).map((item) => item.title).join(" ");
    expect(text).toContain("1 nulo en 1 columna");
    expect(text).toContain("1 valor incompatible con el tipo sugerido en 1 columna");
    expect(text).toContain("1 columna con datos personales");
  });

  it("ningún contador escribe «1 filas», «1 columnas» o «1 valores» (TXT-05)", () => {
    const sourceRoot = join(process.cwd(), "src");
    const files: string[] = [];
    const visit = (directory: string) => {
      for (const name of readdirSync(directory)) {
        const path = join(directory, name);
        if (statSync(path).isDirectory()) visit(path);
        else if (/\.tsx?$/.test(name) && !/\.test\.tsx?$/.test(name)) files.push(path);
      }
    };
    visit(sourceRoot);
    const fixedPlural = /\$\{[^}]*\}\s+(?:filas|columnas|valores|celdas|filtros|reglas)\b/;
    const offending = files.flatMap((file) => readFileSync(file, "utf8").split(/\r?\n/).flatMap((line, index) => {
      if (!fixedPlural.test(line)) return [];
      // The singular is handled on the line, or the count is never 1: the
      // else branch of a `=== 1` test, a limit constant, or «N de M columnas».
      if (/=== 1|plural\(|^\s*: `|MAX_[A-Z_]+\}|EXCEL_MAX|referencia compuesta|\} de \$\{/.test(line)) return [];
      return [`${relative(sourceRoot, file)}:${index + 1}`];
    }));
    expect(offending).toEqual([]);
  });
});
