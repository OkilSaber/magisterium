import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { I18nProvider } from "./i18n";

// Les feux tricolores de macOS se posent sur l'interface : on leur réserve de la
// place seulement sur Mac (ailleurs, la barre de titre est native).
document.documentElement.classList.add(navigator.userAgent.includes("Mac") ? "platform-mac" : "platform-other");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <I18nProvider>
      <App />
    </I18nProvider>
  </React.StrictMode>,
);
