import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";

const mock = (name: string) => fileURLToPath(new URL(`./mocks/${name}.ts`, import.meta.url));

// Même front que l'app, mais avec les API Tauri simulées pour le navigateur.
export default defineConfig({
  root: fileURLToPath(new URL("../..", import.meta.url)),
  plugins: [react()],
  resolve: {
    alias: {
      "@tauri-apps/api/core": mock("core"),
      "@tauri-apps/api/event": mock("event"),
      "@tauri-apps/plugin-dialog": mock("dialog"),
      "@tauri-apps/plugin-opener": mock("opener"),
    },
  },
  server: { port: 5199, strictPort: true },
  logLevel: "warn",
});
