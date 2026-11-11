import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
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

  it("muestra el motivo si no se pueden listar los controladores, no «instala el controlador»", async () => {
    vi.spyOn(bridge, "listDatabaseDrivers").mockRejectedValue(new Error("No se pudieron listar los controladores ODBC: sin acceso"));
    render(<ServerConnectionForm kind="postgresql" disabled={false} onChange={vi.fn()} />);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("sin acceso");
    expect(screen.queryByText(/No hay un controlador ODBC/)).toBeNull();
    expect(screen.queryByText(/Instala /)).toBeNull();
  });

  it("avisa al padre con el controlador cuando llega después de que el usuario escribió", async () => {
    let resolveDrivers: (drivers: string[]) => void = () => {};
    vi.spyOn(bridge, "listDatabaseDrivers").mockReturnValue(new Promise<string[]>((resolve) => { resolveDrivers = resolve; }));
    const onChange = vi.fn();
    render(<ServerConnectionForm kind="postgresql" disabled={false} onChange={onChange} />);

    fireEvent.change(screen.getByLabelText("Servidor"), { target: { value: "localhost" } });
    fireEvent.change(screen.getByLabelText("Puerto"), { target: { value: "5432" } });
    fireEvent.change(screen.getByLabelText("Base de datos"), { target: { value: "datos" } });
    fireEvent.change(screen.getByLabelText("Usuario"), { target: { value: "ana" } });
    expect(onChange).toHaveBeenLastCalledWith(null);

    await act(async () => resolveDrivers(["PostgreSQL Unicode(x64)"]));

    const last = onChange.mock.lastCall?.[0];
    expect(last).not.toBeNull();
    expect(last).toMatchObject({ server: "localhost", database: "datos", user: "ana", driver: "PostgreSQL Unicode(x64)" });
  });
});
