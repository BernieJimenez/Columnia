import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it } from "vitest";

import { ModalDialog } from "./ModalDialog";

afterEach(cleanup);

function DialogHarness() {
  const [open, setOpen] = useState(false);

  return (
    <>
      <button type="button" onClick={() => setOpen(true)}>Abrir diálogo</button>
      {open && (
        <ModalDialog
          role="dialog"
          labelledBy="test-dialog-title"
          describedBy="test-dialog-description"
          onDismiss={() => setOpen(false)}
        >
          <h2 id="test-dialog-title">Editar selección</h2>
          <p id="test-dialog-description">Revisa los valores antes de continuar.</p>
          <button type="button">Primera acción</button>
          <button type="button">Última acción</button>
        </ModalDialog>
      )}
    </>
  );
}

describe("ModalDialog", () => {
  it("mantiene el foco, cierra con Escape y lo restaura al control anterior", () => {
    render(<DialogHarness />);
    const trigger = screen.getByRole("button", { name: "Abrir diálogo" });
    trigger.focus();
    fireEvent.click(trigger);

    const dialog = screen.getByRole("dialog", {
      name: "Editar selección",
      description: "Revisa los valores antes de continuar.",
    });
    const first = within(dialog).getByRole("button", { name: "Primera acción" });
    const last = within(dialog).getByRole("button", { name: "Última acción" });
    expect(first).toHaveFocus();

    last.focus();
    fireEvent.keyDown(last, { key: "Tab" });
    expect(first).toHaveFocus();
    fireEvent.keyDown(first, { key: "Tab", shiftKey: true });
    expect(last).toHaveFocus();

    fireEvent.keyDown(dialog, { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });

  it("devuelve el foco al disparador aunque se haya deshabilitado antes de abrir el diálogo", () => {
    function BusyTriggerHarness() {
      const [open, setOpen] = useState(false);
      const [busy, setBusy] = useState(false);
      return (
        <>
          <button type="button" disabled={busy} onClick={() => setBusy(true)}>Seleccionar dataset</button>
          <button type="button" onClick={() => setOpen(true)}>Mostrar revisión</button>
          {open && (
            <ModalDialog role="dialog" labelledBy="busy-title" onDismiss={() => { setOpen(false); setBusy(false); }}>
              <h2 id="busy-title">Revisar encabezados</h2>
              <button type="button">Cancelar</button>
            </ModalDialog>
          )}
        </>
      );
    }

    render(<BusyTriggerHarness />);
    const trigger = screen.getByRole("button", { name: "Seleccionar dataset" });
    trigger.focus();
    // Browsers drop focus to <body> when the focused trigger becomes disabled;
    // jsdom needs the blur before disabling to reach the same state.
    trigger.blur();
    fireEvent.click(trigger);
    expect(trigger).toBeDisabled();
    expect(document.body).toHaveFocus();

    // Simulates the async inspection result opening the dialog without moving focus.
    fireEvent.click(screen.getByRole("button", { name: "Mostrar revisión" }));
    const dialog = screen.getByRole("dialog", { name: "Revisar encabezados" });
    fireEvent.keyDown(dialog, { key: "Escape" });

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });

  it("permite llegar a la acción e incluye el resumen, pero omite campos de un disclosure cerrado", () => {
    render(
      <ModalDialog
        role="dialog"
        labelledBy="import-dialog-title"
        onDismiss={() => undefined}
      >
        <h2 id="import-dialog-title">Revisar importación</h2>
        <details>
          <summary tabIndex={0}>Opciones de importación</summary>
          <label htmlFor="date-convention">Fechas</label>
          <select id="date-convention" defaultValue="unresolved">
            <option value="unresolved">Sin definir</option>
            <option value="ymd">Año, mes y día</option>
          </select>
        </details>
        <button type="button">Cargar archivo</button>
      </ModalDialog>,
    );

    const summary = screen.getByText("Opciones de importación");
    const action = screen.getByRole("button", { name: "Cargar archivo" });
    const hiddenSelect = screen.getByRole("combobox", { name: "Fechas" });
    expect(summary).toHaveFocus();

    fireEvent.keyDown(summary, { key: "Tab" });
    expect(action).toHaveFocus();
    expect(hiddenSelect).not.toHaveFocus();

    fireEvent.keyDown(action, { key: "Tab" });
    expect(summary).toHaveFocus();
  });
});
