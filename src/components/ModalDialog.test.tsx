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
});
