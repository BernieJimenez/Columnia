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
});
