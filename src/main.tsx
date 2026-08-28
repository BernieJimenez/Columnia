import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import "./styles.css";

if (typeof performance !== "undefined") {
  performance.mark("columnia:app-bootstrap");
}

const root = document.getElementById("root");

if (!root) {
  throw new Error("No se encontró el contenedor raíz de Columnia.");
}

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
