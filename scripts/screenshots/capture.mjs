// Génère les captures du README dans un navigateur sans fenêtre, avec les API
// Tauri simulées (scripts/screenshots/mocks). `npm run screenshots`
// Navigateur : CHROME_PATH, sinon Brave ou Chrome installés dans /Applications.
import { createServer } from "vite";
import puppeteer from "puppeteer-core";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { council } from "./fixtures.ts";

const OUT = fileURLToPath(new URL("../../docs/screenshots/", import.meta.url));
const browserPath =
  process.env.CHROME_PATH ??
  [
    "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  ].find(existsSync);
if (!browserPath) throw new Error("Aucun navigateur Chromium trouvé : définis CHROME_PATH.");

// Fond d'écran simulé (la fenêtre de l'app est translucide) et feux tricolores de macOS.
const WINDOW_CSS = `
  html { background: radial-gradient(80% 60% at 20% 10%, #3b2a7a 0%, transparent 60%),
                     radial-gradient(70% 60% at 90% 90%, #7a3b52 0%, transparent 60%),
                     linear-gradient(160deg, #1a1440, #0c0a1c) !important; }
  .fake-lights { position: fixed; top: 23px; left: 26px; z-index: 100; display: flex; gap: 8px; }
  .fake-lights i { width: 12px; height: 12px; border-radius: 50%; display: block; }
`;

const server = await createServer({
  configFile: fileURLToPath(new URL("./vite.config.ts", import.meta.url)),
});
await server.listen();
const url = `http://localhost:${server.config.server.port}/`;

const browser = await puppeteer.launch({
  executablePath: browserPath,
  headless: true,
  defaultViewport: { width: 1600, height: 1020, deviceScaleFactor: 2 },
});
const page = await browser.newPage();
await page.evaluateOnNewDocument((councilJson) => {
  localStorage.setItem("lang", "en");
  localStorage.setItem("council", councilJson);
  localStorage.setItem("execMode", "auto");
  localStorage.setItem("web", "1");
  localStorage.setItem("panel.history", "1");
  localStorage.setItem("panel.council", "1");
}, JSON.stringify(council));

const settle = (ms = 700) => new Promise((r) => setTimeout(r, ms));
const shot = async (name) => {
  await settle();
  await page.screenshot({ path: `${OUT}${name}.png` });
  console.log(`✓ docs/screenshots/${name}.png`);
};
const click = (selector) => page.$eval(selector, (el) => el.click());

async function open(scene = "") {
  await page.evaluateOnNewDocument((s) => {
    globalThis.__demoScene = s;
  }, scene);
  await page.goto(url, { waitUntil: "networkidle0" });
  await page.addStyleTag({ content: WINDOW_CSS });
  await page.evaluate(() => {
    const lights = document.createElement("div");
    lights.className = "fake-lights";
    lights.innerHTML = ["#ff5f57", "#febc2e", "#28c840"].map((c) => `<i style="background:${c}"></i>`).join("");
    document.body.appendChild(lights);
  });
  await page.waitForSelector(".provider");
}

// 1. Le conseil en plein débat : colonnes des IA + synthèse.
await open();
await shot("empty");
await click(".conv-item");
await page.waitForSelector(".turn:nth-of-type(2) .columns");
await page.evaluate(() => {
  document.querySelector(".turn:last-of-type .user-prompt")?.scrollIntoView({ block: "start" });
  document.querySelector(".thread")?.scrollBy(0, -12);
});
await shot("council");

// 1 bis. Tour initial : DeepSeek (API) cherche sur le web et montre son raisonnement.
await page.evaluate(() => {
  const tab = document.querySelector(".turn:last-of-type .tabs button");
  if (tab instanceof HTMLElement) tab.click();
});
await settle(300);
await page.evaluate(() => {
  document.querySelector(".turn:last-of-type .user-prompt")?.scrollIntoView({ block: "start" });
  document.querySelector(".thread")?.scrollBy(0, -12);
});
await shot("web-tools");

// 2. Lecture façon chat : débats repliés, seules les synthèses restent.
await page.evaluate(() => {
  const toggle = document.querySelector(".turn:last-of-type .toggle");
  if (toggle instanceof HTMLElement) toggle.click();
  document.querySelector(".thread")?.scrollTo(0, 0);
});
await shot("thread");

// 3. Mode focus : les deux panneaux repliés.
await page.$$eval(".topbar .icon-btn", (buttons) => {
  buttons[0].click();
  buttons[buttons.length - 1].click();
});
await settle(500);
await shot("focus");

// 3 bis. Limites d'usage.
await open();
await page.$eval('.topbar .icon-btn[title="Usage limits"]', (el) => el.click());
await page.waitForSelector(".usage-provider");
await shot("usage");

// 4. Réglages : langues, fournisseurs, recherche web.
await open();
await page.$eval('.topbar .icon-btn[title="Settings"]', (el) => el.click());
await page.waitForSelector(".modal");
await shot("settings");

// 5. Ajout d'un fournisseur : les préréglages.
await page.evaluate(() => document.querySelector(".modal .section-title .pill")?.click());
await page.waitForSelector(".preset-grid");
await shot("providers");

// 6. Installation de SearXNG en cours.
await open("install");
await page.$eval('.topbar .icon-btn[title="Settings"]', (el) => el.click());
await page.waitForSelector(".builtin .pill.primary");
await click(".builtin .pill.primary");
await settle(1800);
await page.evaluate(() => document.querySelector(".builtin")?.scrollIntoView({ block: "center" }));
await shot("searxng");

await browser.close();
await server.close();

// Captures en 2x redimensionnées à 2000 px : nettes sur écran Retina, plus légères.
const { execFileSync } = await import("node:child_process");
const { readdirSync } = await import("node:fs");
for (const f of readdirSync(OUT).filter((f) => f.endsWith(".png"))) {
  execFileSync("sips", ["-Z", "2000", `${OUT}${f}`], { stdio: "ignore" });
}
