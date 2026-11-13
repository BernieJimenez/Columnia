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

const READY = /^Controlador: Driver\.$/;

describe("motivo asociado al campo que falta", () => {
  it.each(["sqlserver", "postgresql", "mysql"] as const)("%s: el motivo del servidor espera a que se toque el campo y vive en un único contenedor de estado", async (kind) => {
    vi.spyOn(bridge, "listDatabaseDrivers").mockResolvedValue(["Driver"]);
    render(<DatabaseConnectionForm kind={kind} disabled={false} onChange={vi.fn()} />);

    const server = screen.getByLabelText("Servidor");
    await screen.findByText(READY);
    expect(server).toBeRequired();
    // A freshly opened form says nothing about the missing field and does not announce an error.
    expect(screen.queryByText("Indica el servidor.")).toBeNull();
    expect(server).not.toHaveAttribute("aria-invalid");
    expect(server).not.toHaveAttribute("aria-describedby");
    const status = screen.getByText(READY).nextElementSibling as HTMLElement;
    expect(status).toHaveAttribute("role", "status");
    expect(status).toBeEmptyDOMElement();

    fireEvent.blur(server);
    // The same element stays in place; only its text changes.
    expect(status).toHaveTextContent("Indica el servidor.");
    expect(server).toHaveAccessibleDescription("Indica el servidor.");
    expect(server).toHaveAttribute("aria-invalid", "true");

    fireEvent.change(server, { target: { value: "localhost" } });
    expect(server).not.toHaveAttribute("aria-invalid");
    expect(server).not.toHaveAttribute("aria-describedby");
    expect(status.isConnected).toBe(true);
    expect(screen.queryByText("Indica el servidor.")).toBeNull();
  });

  it("escribir en otro campo no destapa el motivo del servidor", async () => {
    vi.spyOn(bridge, "listDatabaseDrivers").mockResolvedValue(["Driver"]);
    render(<DatabaseConnectionForm kind="postgresql" disabled={false} onChange={vi.fn()} />);
    await screen.findByText(READY);
    fireEvent.change(screen.getByLabelText("Base de datos"), { target: { value: "datos" } });
    expect(screen.queryByText("Indica el servidor.")).toBeNull();
  });

  it("si el padre da el id de su mensaje, el motivo se muestra aunque no se haya tocado nada", async () => {
    vi.spyOn(bridge, "listDatabaseDrivers").mockResolvedValue(["Driver"]);
    render(<DatabaseConnectionForm kind="mysql" disabled={false} onChange={vi.fn()} errorId="aviso" />);
    const server = screen.getByLabelText("Servidor");
    expect(await screen.findByText("Indica el servidor.")).toBeInTheDocument();
    expect(server).toHaveAttribute("aria-invalid", "true");
    expect(server.getAttribute("aria-describedby")).toContain("aviso");
  });

  it("sin controlador instalado lo dice enseguida, sin esperar a que se toque nada", async () => {
    vi.spyOn(bridge, "listDatabaseDrivers").mockResolvedValue([]);
    render(<DatabaseConnectionForm kind="postgresql" disabled={false} onChange={vi.fn()} />);
    expect(await screen.findByText(/No hay un controlador ODBC de PostgreSQL instalado/)).toBeInTheDocument();
  });

  it("PostgreSQL y MySQL: el usuario falta y el motivo lo nombra al salir del campo", async () => {
    vi.spyOn(bridge, "listDatabaseDrivers").mockResolvedValue(["Driver"]);
    render(<DatabaseConnectionForm kind="mysql" disabled={false} onChange={vi.fn()} />);
    await screen.findByText(READY);

    fireEvent.change(screen.getByLabelText("Servidor"), { target: { value: "localhost" } });
    const user = screen.getByLabelText("Usuario");
    expect(user).toBeRequired();
    expect(screen.queryByText("Indica el usuario de MySQL.")).toBeNull();
    fireEvent.blur(user);
    expect(user).toHaveAccessibleDescription("Indica el usuario de MySQL.");
    expect(user).toHaveAttribute("aria-invalid", "true");
  });

  it("SQL Server: con Windows el usuario no existe; con usuario y contraseña falta y se nombra", async () => {
    vi.spyOn(bridge, "listDatabaseDrivers").mockResolvedValue(["Driver"]);
    render(<DatabaseConnectionForm kind="sqlserver" disabled={false} onChange={vi.fn()} />);
    await screen.findByText(READY);

    fireEvent.change(screen.getByLabelText("Servidor"), { target: { value: "localhost" } });
    expect(screen.queryByLabelText("Usuario")).toBeNull();
    expect(screen.getByText(READY).nextElementSibling).toBeEmptyDOMElement();

    fireEvent.click(screen.getByRole("radio", { name: "Usuario y contraseña de SQL Server" }));
    const user = screen.getByLabelText("Usuario");
    expect(user).toBeRequired();
    fireEvent.blur(user);
    expect(user).toHaveAccessibleDescription("Indica el usuario de SQL Server.");
  });
});

describe("cambio de motor", () => {
  it("conserva servidor y usuario, descarta la contraseña y lo dice", async () => {
    vi.spyOn(bridge, "listDatabaseDrivers").mockResolvedValue(["Driver"]);
    const { rerender } = render(<DatabaseConnectionForm kind="postgresql" disabled={false} onChange={vi.fn()} />);
    await screen.findByText(READY);
    fireEvent.change(screen.getByLabelText("Servidor"), { target: { value: "db.ejemplo.com" } });
    fireEvent.change(screen.getByLabelText("Usuario"), { target: { value: "ana" } });
    fireEvent.change(screen.getByLabelText("Contraseña"), { target: { value: "secreta" } });

    rerender(<DatabaseConnectionForm kind="mysql" disabled={false} onChange={vi.fn()} />);
    expect(screen.getByLabelText("Servidor")).toHaveValue("db.ejemplo.com");
    expect(screen.getByLabelText("Usuario")).toHaveValue("ana");
    expect(screen.getByLabelText("Contraseña")).toHaveValue("");
    expect(screen.getByText("Al cambiar a MySQL se conservaron el servidor y el usuario. No se traslada la contraseña.")).toBeInTheDocument();
  });

  it("de SQL Server a PostgreSQL quita la instancia y lo dice", async () => {
    vi.spyOn(bridge, "listDatabaseDrivers").mockResolvedValue(["Driver"]);
    const { rerender } = render(<DatabaseConnectionForm kind="sqlserver" disabled={false} onChange={vi.fn()} />);
    await screen.findByText(READY);
    fireEvent.change(screen.getByLabelText("Servidor"), { target: { value: "equipo\\SQLEXPRESS" } });
    rerender(<DatabaseConnectionForm kind="postgresql" disabled={false} onChange={vi.fn()} />);
    expect(screen.getByLabelText("Servidor")).toHaveValue("equipo");
    expect(screen.getByText(/No se traslada el nombre de instancia/)).toBeInTheDocument();
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
