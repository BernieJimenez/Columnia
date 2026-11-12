import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { DEFAULT_SQL_SERVER_SCRIPT, SqlServerScriptTarget } from "./SqlServerScriptTarget";

afterEach(cleanup);

describe("SqlServerScriptTarget", () => {
  it("apunta al motivo de ocupado solo mientras está desactivado (AUD-059)", () => {
    const props = { value: DEFAULT_SQL_SERVER_SCRIPT, columns: ["id"], onChange: vi.fn() };
    const { rerender } = render(<SqlServerScriptTarget {...props} disabled />);
    expect(screen.getByRole("group", { name: "Tabla en SQL Server" })).toHaveAttribute("aria-describedby", "busy-reason");
    rerender(<SqlServerScriptTarget {...props} disabled={false} />);
    expect(screen.getByRole("group", { name: "Tabla en SQL Server" })).not.toHaveAttribute("aria-describedby");
  });

  describe("aviso de la clave (AUD-098)", () => {
    const profiles = [
      { name: "id", nullCount: 2, uniqueCount: 6 },
      { name: "codigo", nullCount: 0, uniqueCount: 6 },
      { name: "limpia", nullCount: 0, uniqueCount: 8 },
    ];
    const upsert = (keyColumns: string[]) => ({ ...DEFAULT_SQL_SERVER_SCRIPT, mode: "upsert" as const, keyColumns });
    const base = { columns: ["id", "codigo", "limpia"], disabled: false, onChange: vi.fn(), rowCount: 8 };

    it("avisa de los nulos de una clave con su número sin bloquear las casillas", () => {
      render(<SqlServerScriptTarget {...base} value={upsert(["id"])} columnProfiles={profiles} />);
      expect(screen.getByRole("status")).toHaveTextContent("La columna «id» tiene 2 nulos");
      expect(screen.getByRole("status")).not.toHaveTextContent("repetidos");
      expect(screen.getByRole("checkbox", { name: "limpia" })).toBeEnabled();
    });

    it("con clave compuesta solo habla de nulos y se asocia al grupo", () => {
      render(<SqlServerScriptTarget {...base} value={upsert(["id", "codigo"])} columnProfiles={profiles} />);
      const note = screen.getByRole("status");
      expect(note).toHaveTextContent("«id» tiene 2 nulos");
      expect(note).not.toHaveTextContent("repetidos");
      expect(screen.getByRole("group", { name: "Columnas clave" }).getAttribute("aria-describedby")).toContain(note.id);
      expect(screen.getByRole("checkbox", { name: "limpia" })).toBeEnabled();
    });

    it("una clave con repetidos y sin nulos avisa de las filas que sobran", () => {
      render(<SqlServerScriptTarget {...base} value={upsert(["codigo"])} columnProfiles={profiles} />);
      expect(screen.getByRole("status")).toHaveTextContent("«codigo» tiene valores repetidos (2 filas repiten una clave)");
    });

    it("sin aviso con una clave limpia o sin perfil", () => {
      const { rerender } = render(<SqlServerScriptTarget {...base} value={upsert(["limpia"])} columnProfiles={profiles} />);
      expect(screen.queryByRole("status")).toBeNull();
      rerender(<SqlServerScriptTarget {...base} value={upsert(["id"])} columnProfiles={null} />);
      expect(screen.queryByRole("status")).toBeNull();
    });
  });
});
