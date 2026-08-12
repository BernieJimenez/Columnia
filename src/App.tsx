import { useEffect, useState } from "react";

import { getAppInfo, type AppInfo } from "./bridge";

type AppStatus =
  | { kind: "loading" }
  | { kind: "ready"; info: AppInfo }
  | { kind: "browser" }
  | { kind: "error"; message: string };

function isTauriRuntime(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export function App() {
  const [status, setStatus] = useState<AppStatus>({ kind: "loading" });

  useEffect(() => {
    if (!isTauriRuntime()) {
      setStatus({ kind: "browser" });
      return;
    }

    let active = true;
    getAppInfo()
      .then((info) => active && setStatus({ kind: "ready", info }))
      .catch((error: unknown) => {
        if (active) {
          const message = error instanceof Error ? error.message : String(error);
          setStatus({ kind: "error", message });
        }
      });

    return () => {
      active = false;
    };
  }, []);

  return (
    <main className="shell">
      <section className="intro" aria-labelledby="app-title">
        <p className="eyebrow">Nueva estación local de datos</p>
        <h1 id="app-title">Columnia</h1>
        <p className="summary">
          Preparación de datos confiable, privada y diseñada para Windows, macOS y Linux.
        </p>

        <div className="status" role="status" aria-live="polite">
          {status.kind === "loading" && "Conectando con el motor local…"}
          {status.kind === "browser" &&
            "Vista web lista. Abre Columnia con Tauri para conectar el motor Rust."}
          {status.kind === "ready" &&
            `${status.info.name} ${status.info.version} · ${status.info.platform}`}
          {status.kind === "error" && `No se pudo iniciar el motor local: ${status.message}`}
        </div>
      </section>

      <section className="milestone" aria-labelledby="milestone-title">
        <p className="step">Hito 01</p>
        <h2 id="milestone-title">El contrato de escritorio está preparado</h2>
        <p>
          Esta primera superficie valida la comunicación tipada entre React y Rust. La carga de
          datasets se incorporará después de medir Polars y DuckDB con archivos reales.
        </p>
      </section>
    </main>
  );
}

