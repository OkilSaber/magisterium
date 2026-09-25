import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { I18nProvider } from "./i18n";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";

// Les feux tricolores de macOS se posent sur l'interface : on leur réserve de la
// place seulement sur Mac (ailleurs, la barre de titre est native).
document.documentElement.classList.add(navigator.userAgent.includes("Mac") ? "platform-mac" : "platform-other");

// Un lien ne doit jamais remplacer l'app dans la fenêtre : le web part dans le
// navigateur par défaut, un fichier local s'affiche dans le Finder.
document.addEventListener("click", (e) => {
  const link = (e.target as Element | null)?.closest?.("a[href]");
  if (!(link instanceof HTMLAnchorElement)) return;
  const href = link.getAttribute("href") ?? "";
  if (href.startsWith("#")) return;
  e.preventDefault();
  if (href.startsWith("file://")) {
    revealItemInDir(decodeURIComponent(new URL(href).pathname)).catch(() => {});
  } else if (/^(https?:|mailto:)/i.test(href)) {
    openUrl(href).catch(() => {});
  }
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <I18nProvider>
      <App />
    </I18nProvider>
  </React.StrictMode>,
);
