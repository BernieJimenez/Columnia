import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as bridge from "../../bridge";
import { DatabaseConnectionForm } from "./DatabaseConnectionForm";
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

describe("motivo asociado al campo que falta", () => {
  it.each(["sqlserver", "postgresql", "mysql"] as const)("%s: el servidor es obligatorio y el motivo vive en un único contenedor de estado", async (kind) => {
    vi.spyOn(bridge, "listDatabaseDrivers").mockResolvedValue(["Driver"]);
    render(<DatabaseConnectionForm kind={kind} disabled={false} onChange={vi.fn()} />);

    const server = screen.getByLabelText("Servidor");
    const status = await screen.findByText("Indica el servidor.");
    expect(server).toBeRequired();
    expect(status).toHaveAttribute("role", "status");
    expect(server).toHaveAccessibleDescription("Indica el servidor.");
    // A freshly opened form is not announced as an error.
    expect(server).not.toHaveAttribute("aria-invalid");

    fireEvent.blur(server);
    expect(server).toHaveAttribute("aria-invalid", "true");

    fireEvent.change(server, { target: { value: "localhost" } });
    expect(server).not.toHaveAttribute("aria-invalid");
    expect(server).not.toHaveAttribute("aria-describedby");
    // The same element stays in place; only its text changes.
    expect(screen.getAllByRole("status")).toEqual([status]);
  });

  it("PostgreSQL y MySQL: el usuario falta y el motivo lo nombra", async () => {
    vi.spyOn(bridge, "listDatabaseDrivers").mockResolvedValue(["Driver"]);
    render(<DatabaseConnectionForm kind="mysql" disabled={false} onChange={vi.fn()} />);
    await screen.findByText("Indica el servidor.");

    fireEvent.change(screen.getByLabelText("Servidor"), { target: { value: "localhost" } });
    const user = screen.getByLabelText("Usuario");
    expect(user).toBeRequired();
    expect(user).toHaveAccessibleDescription("Indica el usuario de MySQL.");
    expect(user).toHaveAttribute("aria-invalid", "true");
  });

  it("SQL Server: con Windows el usuario no existe; con usuario y contraseña falta y se nombra", async () => {
    vi.spyOn(bridge, "listDatabaseDrivers").mockResolvedValue(["Driver"]);
    render(<DatabaseConnectionForm kind="sqlserver" disabled={false} onChange={vi.fn()} />);
    await screen.findByText("Indica el servidor.");

    fireEvent.change(screen.getByLabelText("Servidor"), { target: { value: "localhost" } });
    expect(screen.queryByLabelText("Usuario")).toBeNull();
    expect(screen.getByRole("status")).toBeEmptyDOMElement();

    fireEvent.click(screen.getByRole("radio", { name: "Usuario y contraseña de SQL Server" }));
    expect(screen.getByLabelText("Usuario")).toHaveAccessibleDescription("Indica el usuario de SQL Server.");
    expect(screen.getByLabelText("Usuario")).toBeRequired();
  });
});

describe("motivo de ocupado (AUD-059)", () => {
  it.each(["sqlserver", "postgresql", "mysql"] as const)("el grupo de %s apunta al motivo solo si el bloqueo es por ocupado", async (kind) => {
    vi.spyOn(bridge, "listDatabaseDrivers").mockResolvedValue([]);
    const { rerender } = render(<DatabaseConnectionForm kind={kind} disabled blockedByBusy onChange={vi.fn()} />);
    expect(await screen.findByRole("group", { name: "Datos de conexión" })).toHaveAttribute("aria-describedby", "busy-reason");
    rerender(<DatabaseConnectionForm kind={kind} disabled onChange={vi.fn()} />);
    expect(screen.getByRole("group", { name: "Datos de conexión" })).not.toHaveAttribute("aria-describedby");
  });
});
