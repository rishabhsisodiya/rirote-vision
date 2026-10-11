import "@fontsource/bebas-neue/400.css";
import "@fontsource/space-mono/400.css";
import "@fontsource/space-mono/700.css";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/ui.css";

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import { logToApp } from "./lib/ipc";

// Failures nobody handled still leave a trace in the app's log.
window.addEventListener("error", (event) => logToApp(`error: ${event.message}`));
window.addEventListener("unhandledrejection", (event) => logToApp(`unhandled: ${String(event.reason)}`));

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
