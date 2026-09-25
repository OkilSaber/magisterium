// Remplace `@tauri-apps/api/core` pour les captures : aucune commande Rust n'est appelée.
import * as fx from "../fixtures";
import { emit } from "./event";

export class Channel<T> {
  onmessage: (message: T) => void = () => {};
}

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

/// L'installation de SearXNG s'arrête en plein téléchargement de Python.
async function fakeInstall(): Promise<never> {
  const step = (index: number, state: string, extra: object = {}) =>
    emit("searxng-progress", {
      step: ["uv", "python", "searxng", "deps", "config", "verify"][index],
      index,
      total: 6,
      state,
      bytes: null,
      total_bytes: null,
      message: null,
      ...extra,
    });
  emit("searxng-status", { installed: false, enabled: false, installing: true, starting: false, port: null, error: null, size_bytes: 0 });
  step(0, "done");
  const total = 25_153_879;
  for (let i = 1; i <= 12; i++) {
    step(1, "running", { bytes: Math.round((total * 0.62 * i) / 12), total_bytes: total });
    await wait(120);
  }
  return new Promise<never>(() => {});
}

/// La capture d'installation montre SearXNG pas encore installé ; les autres, actif.
const installScene = () => (globalThis as { __demoScene?: string }).__demoScene === "install";

const handlers: Record<string, (args: any) => unknown> = {
  list_providers: () => fx.providers,
  refresh_provider: ({ provider }) => fx.providers.find((p) => p.id === provider),
  get_provider_configs: () => fx.providerConfigs,
  get_web_config: () => ({ engine: "builtin", searxng_url: "", has_tavily_key: false, builtin_enabled: !installScene() }),
  searxng_status: () =>
    installScene()
      ? { installed: false, enabled: false, installing: false, starting: false, port: null, error: null, size_bytes: 0 }
      : { installed: true, enabled: true, installing: false, starting: false, port: 8931, error: null, size_bytes: 219_000_000 },
  searxng_install: () => fakeInstall(),
  get_usage: () => fx.usage,
  list_conversations: () => fx.conversations,
  load_conversation: () => fx.conversation,
  save_conversation: () => null,
  delete_conversation: () => null,
  start_run: () => "demo-run",
  cancel_run: () => null,
};

export async function invoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  const handler = handlers[cmd];
  if (!handler) throw new Error(`commande non simulée : ${cmd}`);
  return (await handler(args)) as T;
}
