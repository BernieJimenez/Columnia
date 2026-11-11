import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as bridge from "../../bridge";
import { ServerConnectionForm } from "./ServerConnectionForm";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("ServerConnectionForm", () => {
  it("avisa de que el cifrado es oportunista y puede forzarse una conexión sin cifrar", async () => {
    vi.spyOn(bridge, "listDatabaseDrivers").mockResolvedValue([]);
    render(<ServerConnectionForm kind="postgresql" disabled={false} onChange={vi.fn()} />);

    const note = await screen.findByText(/Solo en servidores propios o de prueba/);
    expect(note.textContent).toContain("alguien con acceso a la red puede forzar una conexión sin cifrar");
    expect(note.textContent).not.toContain("se usa igual");
  });
});
