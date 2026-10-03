import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import { ErrorBoundary } from "./components/ErrorBoundary";
import "./styles.css";

if (typeof performance !== "undefined") {
  performance.mark("columnia:app-bootstrap");
}

const root = document.getElementById("root");

if (!root) {
  throw new Error("No se encontró el contenedor raíz de Columnia.");
}

const appRoot = root;

function AppFailure({ error }: { error: Error }) {
  return (
    <main className="app-failure" role="alert">
      <h1>Columnia tuvo un error inesperado</h1>
      <p>La ventana no se pudo dibujar. Recarga para volver a empezar; los proyectos guardados no se pierden.</p>
      <p><small>Detalle: {error.message}</small></p>
      <button type="button" onClick={() => window.location.reload()}>Recargar</button>
    </main>
  );
}

async function renderApp() {
  await import("./workflow-styles");

  createRoot(appRoot).render(
    <StrictMode>
      <ErrorBoundary fallback={(error) => <AppFailure error={error} />}>
        <App />
      </ErrorBoundary>
    </StrictMode>,
  );
}

// ARQ-03: a failed start shows a message instead of an empty window.
renderApp().catch((error: unknown) => {
  appRoot.textContent = `Columnia no pudo iniciar: ${error instanceof Error ? error.message : String(error)}. Cierra y vuelve a abrir la aplicación.`;
});
