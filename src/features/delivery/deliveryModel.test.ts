import { describe, expect, it } from "vitest";

import { QUALITY_DATASET_COLUMN, type ColumnProfile, type DatabaseConnection, type DatasetPreview, type QualityRule, type QualityValidationResult } from "../../bridge";
import {
  INITIAL_DATABASE_TARGET,
  completeDatabaseTarget,
  databaseTargetTable,
  deliverySummary,
  sqlServerScriptTable,
  TSQL_SCRIPT_WARNING_BYTES,
  estimateTsqlScriptBytes,
  tsqlScriptIsTooLarge,
  MAX_QUALITY_RULES,
  bundleFileNames,
  listFileNames,
  databaseKindForExportFormat,
  validateDatabaseTargetDraft,
  validateQualityRuleDraft,
  databaseTargetIssue,
  invalidQualityRuleIndex,
} from "./deliveryModel";
import {
  INITIAL_DELIVERY_CONTRACT,
  deliveryContractFromRules,
  deliveryRules,
  invalidateDeliveryContract,
  reduceDeliveryContract,
  type DeliveryContractState,
} from "./deliveryContract";
import { MAX_SUGGESTED_QUALITY_RULES, excelLimitIssues, personalDataColumnNames, suggestQualityRules } from "./deliverySignals";

const dataset: DatasetPreview = {
  fileName: "ventas.csv",
  fileSizeBytes: 128,
  rowCount: 2,
  columnCount: 2,
  columns: [
    { name: "total", dataType: "Int64" },
    { name: "limite", dataType: "Int64" },
    { name: "estado", dataType: "String" },
    { name: "fecha", dataType: "String" },
  ],
  rows: [["10", "12", "ok", "2024-01-01"], ["20", "20", "ok", "2024-06-01"]],
};

const validRule: QualityRule = {
  column: "total",
  kind: "not_null",
  maxInvalid: 0,
};

const passedResult: QualityValidationResult = {
  passed: true,
  rowCount: 2,
  totalRules: 1,
  failedRules: 0,
  rules: [{
    ...validRule,
    checkedCount: 2,
    invalidCount: 0,
    invalidPct: 0,
    passed: true,
  }],
};

describe("validateQualityRuleDraft", () => {
  it("acepta un contrato válido", () => {
    expect(validateQualityRuleDraft([validRule], dataset)).toBeNull();
    expect(validateQualityRuleDraft([{ ...validRule, maxInvalidPct: 0 }], dataset)).toBeNull();
  });

  it.each([
    [{ ...validRule, column: "ausente" }, /columna existente/],
    [{ ...validRule, maxInvalid: -1 }, /entero igual o mayor/],
    [{ ...validRule, maxInvalid: undefined, maxInvalidPct: 101 }, /entre 0 y 100/],
    [{ ...validRule, kind: "numeric_range", maxInvalid: 0 }, /al menos un límite/],
    [{ ...validRule, kind: "numeric_range", min: 2.5 }, /enteros seguros/],
    [{ ...validRule, kind: "numeric_range", min: 20, max: 10 }, /mínimo no puede superar/],
    [{ ...validRule, min: 10 }, /solo se permiten para rangos/],
    [{ ...validRule, kind: "column_compare", columns: ["total", "limite"], maxInvalid: 0 }, /operador de comparación/],
    [{ ...validRule, kind: "column_compare", columns: ["total", "estado"], operator: "lte", maxInvalid: 0 }, /compartir tipo físico/],
    [{ ...validRule, column: "fecha", kind: "date_range", maxInvalid: 0 }, /fecha mínima/],
    [{ ...validRule, column: "fecha", kind: "date_range", minDate: "2024-12-31", maxDate: "2024-01-01", maxInvalid: 0 }, /no puede superar/],
    [{ ...validRule, kind: "referential_integrity", columns: ["total"], maxInvalid: 0 }, /al menos una referencia/],
    [{ ...validRule, kind: "referential_integrity", columns: ["total", "limite"], referenceValues: ["10,12"], maxInvalid: 0 }, /arreglo JSON/],
    [{ ...validRule, kind: "referential_integrity", columns: ["total", "total"], referenceValues: ["[10,10]"], maxInvalid: 0 }, /no repitas/],
    [{ ...validRule, direction: "decreasing" }, /direction solo aplica/],
    [{ ...validRule, kind: "monotonic", direction: "sideways" } as unknown as QualityRule, /dirección monotónica válida/],
    [{ ...validRule, kind: "aggregate_check", maxInvalid: 0 }, /valor esperado o referencias/],
    [{ ...validRule, kind: "aggregate_check", expected: 2, referenceValues: ["bad"], maxInvalid: 0 }, /referencias numéricas/],
    [{ ...validRule, kind: "aggregate_reconciliation", columns: ["total"], maxInvalid: 0 }, /exactamente dos columnas/],
    [{ ...validRule, kind: "aggregate_check", expected: 2, toleranceAbs: -1, maxInvalid: 0 }, /tolerancia absoluta/],
    [{ ...validRule, kind: "distribution_drift", maxInvalid: 0 }, /línea base numérica/],
    [{ ...validRule, kind: "distribution_drift", baseline: ["bad"], maxInvalid: 0 }, /línea base deben ser finitos/],
    [{ ...validRule, kind: "distribution_drift", baseline: ["1", "2"], threshold: -1, maxInvalid: 0 }, /umbral/],
   ] satisfies Array<[QualityRule, RegExp]>)("rechaza borradores inválidos", (rule, message) => {
    expect(validateQualityRuleDraft([rule], dataset)).toMatch(message);
  });

  it("centraliza el límite máximo de reglas", () => {
    const rules = Array.from({ length: MAX_QUALITY_RULES + 1 }, () => ({ ...validRule }));
    expect(validateQualityRuleDraft(rules, dataset)).toBe(
      `El contrato admite como máximo ${MAX_QUALITY_RULES} reglas.`,
    );
  });

  it("acepta la primera extensión avanzada del contrato v3", () => {
    const advancedRules: QualityRule[] = [
      { column: "estado", kind: "allowed_values", values: ["ok", "pending"], maxInvalid: 0 },
      { column: "estado", kind: "regex", pattern: "^(ok|pending)$", maxInvalidPct: 0 },
      { column: "total", kind: "dtype", dtype: "integer", maxInvalid: 0 },
      { column: "total", kind: "unique_together", columns: ["total", "estado"], maxInvalid: 0 },
      { column: "total", kind: "column_compare", columns: ["total", "limite"], operator: "lte", maxInvalid: 0 },
      { column: "fecha", kind: "date_range", minDate: "2024-01-01", maxDate: "2024-12-31", maxInvalid: 0 },
      {
        column: "total",
        kind: "conditional",
        when: { column: "estado", operator: "eq", value: "ok" },
        then: { column: "total", kind: "numeric_range", min: 0, maxInvalid: 0 },
        maxInvalid: 0,
      },
      {
        column: "estado",
        kind: "referential_integrity",
        columns: ["estado"],
        referenceValues: ["ok", "pending"],
        maxInvalid: 0,
      },
      {
        column: "total",
        kind: "referential_integrity",
        columns: ["total", "limite"],
        referenceValues: ["[10,12]", "[20,20]"],
        maxInvalid: 0,
      },
      { column: "total", kind: "monotonic", direction: "increasing", maxInvalid: 0 },
      { column: "total", kind: "aggregate_check", aggregate: "sum", expected: 30, maxInvalid: 0 },
      {
        column: "total",
        kind: "aggregate_reconciliation",
        columns: ["total", "limite"],
        toleranceAbs: 2,
        maxInvalid: 0,
      },
      {
        column: "total",
        kind: "distribution_drift",
        baseline: ["10", "20"],
        threshold: 0,
        maxInvalid: 0,
      },
      {
        column: QUALITY_DATASET_COLUMN,
        kind: "schema_contract",
        columns: ["total", "limite"],
        allowAdditional: true,
        requiredOrder: ["total", "limite"],
        maxInvalid: 0,
      },
      { column: QUALITY_DATASET_COLUMN, kind: "row_count", min: 1, max: 10, maxInvalid: 0 },
    ];

    expect(validateQualityRuleDraft(advancedRules, dataset)).toBeNull();
  });

  it.each([
    [{ column: "estado", kind: "allowed_values", maxInvalid: 0 }, /al menos un valor/],
    [{ column: "estado", kind: "regex", pattern: "[", maxInvalid: 0 }, /patrón regular no es válido/],
    [{ column: "total", kind: "unique_together", columns: ["total"], maxInvalid: 0 }, /al menos dos columnas/],
    [{ column: "total", kind: "conditional", maxInvalid: 0 }, /condición when/],
    [{
      column: "total",
      kind: "conditional",
      when: { column: "estado", operator: "eq", value: "ok" },
      then: { column: "total", kind: "unique", maxInvalid: 0 },
      maxInvalid: 0,
    }, /then solo admite/],
    [{ column: QUALITY_DATASET_COLUMN, kind: "schema_contract", maxInvalid: 0 }, /columna requerida/],
    [{
      column: QUALITY_DATASET_COLUMN,
      kind: "schema_contract",
      columns: ["total", "total"],
      maxInvalid: 0,
    }, /no repitas/],
    [{ column: QUALITY_DATASET_COLUMN, kind: "row_count", maxInvalid: 0 }, /al menos un límite/],
  ] satisfies Array<[QualityRule, RegExp]>)("rechaza reglas avanzadas incompletas", (rule, message) => {
    expect(validateQualityRuleDraft([rule], dataset)).toMatch(message);
  });
});

describe("estado de entrega", () => {
  it("mapea únicamente los tres formatos remotos a un motor ODBC", () => {
    expect(databaseKindForExportFormat("postgresql")).toBe("postgresql");
    expect(databaseKindForExportFormat("mysql")).toBe("mysql");
    expect(databaseKindForExportFormat("sqlserver")).toBe("sqlserver");
    expect(databaseKindForExportFormat("csv")).toBeNull();
  });

  it("bloquea el reemplazo MySQL antes de iniciar la conexión", () => {
    expect(validateDatabaseTargetDraft({
      ...{
        kind: "mysql",
        connection: null,
        schema: "public",
        table: "dataset",
        tablePolicy: "replace",
      },
    })).toMatch(/Reemplazar está deshabilitada/);
  });

  it("pide completar la conexión mientras el formulario no la entrega, y la envía cuando está completa", () => {
    const draft = { ...INITIAL_DATABASE_TARGET, schema: "public", table: "dataset" };
    expect(validateDatabaseTargetDraft(draft)).toBe("Completa la conexión con el servidor.");
    expect(completeDatabaseTarget(draft)).toBeNull();
    const connection: DatabaseConnection = {
      driver: "PostgreSQL Unicode(x64)", server: "localhost", port: "", database: "", authentication: "password",
      user: "ana", password: "x", trustServerCertificate: false, allowUnencrypted: false,
    };
    const complete = { ...draft, connection };
    expect(validateDatabaseTargetDraft(complete)).toBeNull();
    expect(completeDatabaseTarget(complete)).toEqual(complete);
  });

  it("indica el campo del destino con un valor con tipo (AUD-030)", () => {
    const connection = {
      driver: "PostgreSQL Unicode(x64)", server: "localhost", port: "", database: "", authentication: "password",
      user: "ana", password: "x", trustServerCertificate: false, allowUnencrypted: false,
    } as DatabaseConnection;
    const draft = { ...INITIAL_DATABASE_TARGET, schema: "public", table: "dataset" };
    expect(databaseTargetIssue(draft)?.field).toBe("connection");
    expect(databaseTargetIssue({ ...draft, connection, schema: "mal esquema" })?.field).toBe("schema");
    expect(databaseTargetIssue({ ...draft, connection, table: "1tabla" })?.field).toBe("table");
    expect(databaseTargetIssue({ ...draft, kind: "mysql", tablePolicy: "replace", connection })?.field).toBe("policy");
    expect(databaseTargetIssue({ ...draft, connection })).toBeNull();
  });

  it("indica la regla inválida por su posición (AUD-030)", () => {
    const bad: QualityRule = { column: "no_existe", kind: "not_null", maxInvalid: 0 };
    expect(invalidQualityRuleIndex([validRule, bad], dataset)).toBe(1);
    expect(invalidQualityRuleIndex([validRule], dataset)).toBeNull();
    const tooMany = Array.from({ length: MAX_QUALITY_RULES + 1 }, () => validRule);
    expect(invalidQualityRuleIndex(tooMany, dataset)).toBeNull();
  });

  it("invalida un gate aprobado cuando cambian las reglas", () => {
    const approved: DeliveryContractState = {
      kind: "with_contract",
      rules: [validRule],
      gate: { kind: "ready", result: passedResult },
    };
    const next = reduceDeliveryContract(approved, {
      kind: "rules_changed",
      rules: [{ ...validRule, maxInvalid: 1 }],
    });

    expect(next).toMatchObject({ kind: "with_contract", gate: { kind: "stale" } });
  });

  it("revoca la confirmación sin contrato cuando cambia el dataset", () => {
    const confirmed = reduceDeliveryContract(INITIAL_DELIVERY_CONTRACT, {
      kind: "confirmation_changed",
      confirmation: "confirmed",
    });
    expect(invalidateDeliveryContract(confirmed)).toEqual(INITIAL_DELIVERY_CONTRACT);
  });

  it("restaura reglas persistidas con el gate sin validar", () => {
    const restored = deliveryContractFromRules([validRule]);
    expect(restored).toEqual({ kind: "with_contract", rules: [validRule], gate: { kind: "idle" } });
    expect(deliveryRules(restored)).toEqual([validRule]);
    expect(deliveryContractFromRules([])).toEqual(INITIAL_DELIVERY_CONTRACT);
  });
});

describe("tamaño estimado del script T-SQL", () => {
  it("un dataset pequeño queda por debajo del umbral y uno grande lo supera", () => {
    expect(estimateTsqlScriptBytes(10_000, 10)).toBeLessThan(TSQL_SCRIPT_WARNING_BYTES);
    expect(tsqlScriptIsTooLarge(10_000, 10)).toBe(false);
    expect(estimateTsqlScriptBytes(3_000_000, 20)).toBeGreaterThan(TSQL_SCRIPT_WARNING_BYTES);
    expect(tsqlScriptIsTooLarge(3_000_000, 20)).toBe(true);
  });

  it("crece con las filas, las columnas y la longitud de las celdas", () => {
    const base = estimateTsqlScriptBytes(1_000, 5);
    expect(estimateTsqlScriptBytes(2_000, 5)).toBeGreaterThan(base);
    expect(estimateTsqlScriptBytes(1_000, 6)).toBeGreaterThan(base);
    expect(estimateTsqlScriptBytes(1_000, 5, 40)).toBeGreaterThan(base);
  });

  it("no estima nada sin filas ni columnas", () => {
    expect(estimateTsqlScriptBytes(0, 5)).toBe(0);
    expect(estimateTsqlScriptBytes(5, 0)).toBe(0);
  });
});

describe("suggestQualityRules", () => {
  const column = (name: string, nullCount: number, uniqueCount: number, privacySignal: ColumnProfile["privacySignal"] = null) =>
    ({ name, nullCount, uniqueCount, privacySignal }) as ColumnProfile;

  it("proposes not null and unique for identifiers first, then not null for complete columns", () => {
    expect(suggestQualityRules([
      column("monto", 0, 40),
      column("ciudad", 3, 5),
      column("id", 0, 100),
      column("codigo_cliente", 0, 100),
      column("_cambios", 0, 100),
    ], 100)).toEqual([
      { column: "id", kind: "not_null", maxInvalid: 0 },
      { column: "id", kind: "unique", maxInvalid: 0 },
      { column: "codigo_cliente", kind: "not_null", maxInvalid: 0 },
      { column: "codigo_cliente", kind: "unique", maxInvalid: 0 },
      { column: "monto", kind: "not_null", maxInvalid: 0 },
    ]);
  });

  it("treats a detected identifier as unique only when every value is distinct", () => {
    expect(suggestQualityRules([column("cedula", 0, 100, "identifier")], 100))
      .toContainEqual({ column: "cedula", kind: "unique", maxInvalid: 0 });
    expect(suggestQualityRules([column("id", 0, 99)], 100))
      .toEqual([{ column: "id", kind: "not_null", maxInvalid: 0 }]);
  });

  it("caps the proposal and proposes nothing without a profile or rows", () => {
    const many = Array.from({ length: 10 }, (_, index) => column(`c${index}`, 0, 1));
    expect(suggestQualityRules(many, 100)).toHaveLength(MAX_SUGGESTED_QUALITY_RULES);
    expect(suggestQualityRules(null, 100)).toEqual([]);
    expect(suggestQualityRules(many, 0)).toEqual([]);
  });
});

describe("personalDataColumnNames", () => {
  const column = (name: string, privacySignal: ColumnProfile["privacySignal"]) =>
    ({ name, privacySignal }) as ColumnProfile;

  it("lists contact signals and ignores identifiers, unsignaled and audit columns", () => {
    expect(
      personalDataColumnNames([
        column("correo", "email"),
        column("telefono", "phone"),
        column("direccion", "address"),
        column("nombre", "name"),
        column("cedula", "identifier"),
        column("monto", null),
        column("_cambios", "name"),
      ]),
    ).toEqual(["correo", "telefono", "direccion", "nombre"]);
  });

  it("returns no columns while the profile is not ready", () => {
    expect(personalDataColumnNames(null)).toEqual([]);
  });
});

describe("excelLimitIssues (RV19 / FUN-05)", () => {
  const column = (name: string, maximumLength: number | null) => ({ name, maximumLength }) as ColumnProfile;

  it("fits when rows and cells are within Excel limits or the profile is unknown", () => {
    expect(excelLimitIssues(1_048_575, [column("notas", 32_767)])).toEqual([]);
    expect(excelLimitIssues(10, null)).toEqual([]);
  });

  it("names the rows and the columns that would not open", () => {
    const issues = excelLimitIssues(1_100_000, [column("notas", 40_000), column("id", 6), column("detalle", 32_768)]);

    expect(issues).toHaveLength(2);
    expect(issues[0]).toContain((1_100_000).toLocaleString());
    expect(issues[1]).toMatch(/^notas, detalle: /);
  });
});

describe("rustRegexProblem (FUN-54)", () => {
  it("señala lo que el motor Rust no admite y acepta el resto", async () => {
    const { rustRegexProblem } = await import("./deliveryModel");
    expect(rustRegexProblem("^(?=.*\\d)")).toMatch(/hacia delante/);
    expect(rustRegexProblem("(?<!x)y")).toMatch(/hacia delante o hacia atrás/);
    expect(rustRegexProblem("(a)\\1")).toMatch(/grupos anteriores/);
    expect(rustRegexProblem("(?>ab)")).toMatch(/atómicos/);
    expect(rustRegexProblem("a++")).toMatch(/posesivos/);
    expect(rustRegexProblem("^[A-Z]{3}-\\d{4}$")).toBeNull();
    expect(rustRegexProblem("precio \\(\\?=\\)")).toBeNull();
    expect(rustRegexProblem("(?i)hola")).toBeNull();
  });
});

describe("parseQualityDateBound (FUN-55)", () => {
  it("lee las fechas como el motor Rust y rechaza el resto", async () => {
    const { parseQualityDateBound } = await import("./deliveryModel");
    expect(parseQualityDateBound("1/2/2024")).toBe(Date.UTC(2024, 1, 1));
    expect(parseQualityDateBound("15/01/2024")).toBe(Date.UTC(2024, 0, 15));
    expect(parseQualityDateBound("2024/01/15")).toBe(Date.UTC(2024, 0, 15));
    expect(parseQualityDateBound("2024-01-15T10:30:00")).toBe(Date.UTC(2024, 0, 15, 10, 30));
    expect(parseQualityDateBound("Jan 2, 2024")).toBeNull();
    expect(parseQualityDateBound("31/02/2024")).toBeNull();
  });
});

describe("contenido del paquete (TXT-07)", () => {
  it("lista la receta y la calidad solo cuando el ZIP las lleva", () => {
    expect(bundleFileNames({ recipe: false, quality: false })).toEqual(["dataset.csv", "dictionary.json", "delivery-summary.md", "manifest.json"]);
    expect(bundleFileNames({ recipe: true, quality: true })).toEqual([
      "dataset.csv", "dictionary.json", "quality-report.json", "recipe.json", "delivery-summary.md", "manifest.json",
    ]);
    expect(listFileNames(["a", "b", "c"])).toBe("a, b y c");
    expect(listFileNames(["a"])).toBe("a");
  });
});

describe("resumen de la entrega", () => {
  it("dice destino, tabla, datos, calidad y datos personales", () => {
    const table = sqlServerScriptTable(
      { schema: "", table: "Pedidos", mode: "upsert", keyColumns: ["id"], textSize: "fitted" },
      ["id"],
    );
    expect(table).toBe("dbo.Pedidos · actualizar por clave (id)");
    expect(deliverySummary({
      destination: "SQL Server",
      format: "Script T-SQL",
      rowCount: 1_000,
      columnCount: 1,
      table,
      quality: { kind: "rules", count: 3, passed: true },
      privacyMode: "mask",
      personalDataColumnCount: 2,
    }).map((item) => [item.label, item.value])).toEqual([
      ["Destino", "SQL Server · Script T-SQL"],
      ["Tabla", "dbo.Pedidos · actualizar por clave (id)"],
      ["Datos", `${(1_000).toLocaleString()} filas · 1 columna`],
      ["Calidad", "3 reglas aprobadas"],
      ["Datos personales", "enmascarados en 2 columnas"],
    ]);
  });

  it("un archivo no tiene tabla y avisa de lo que sale sin validar o sin proteger", () => {
    const items = deliverySummary({
      destination: "Archivo",
      format: "CSV",
      rowCount: 2,
      columnCount: 4,
      table: null,
      quality: { kind: "unvalidated" },
      privacyMode: "none",
      personalDataColumnCount: 1,
    });
    expect(items.map((item) => item.label)).not.toContain("Tabla");
    expect(items.find((item) => item.label === "Calidad")?.value).toBe("sin validar");
    expect(items.find((item) => item.label === "Datos personales")?.value).toBe("sin proteger en 1 columna");
    expect(sqlServerScriptTable({ schema: "", table: "", mode: "create", keyColumns: [], textSize: "fitted" }, []))
      .toBe("dbo.(nombre del archivo) · crear tabla nueva");
    expect(databaseTargetTable({ schema: "public", table: "ventas", tablePolicy: "append" }))
      .toBe("public.ventas · añadir filas");
  });
});
