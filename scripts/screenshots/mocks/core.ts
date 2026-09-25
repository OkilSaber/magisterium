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

/// Tour « en direct » : le débat avance, une IA réfléchit encore, le verdict attend.
async function liveRun(config: any, channel: Channel<any>) {
  const send = (e: object) => channel.onmessage(e);
  const [a1, a2, a3] = config.agents.map((a: { key: string }) => a.key);
  await wait(50);
  send({ kind: "phase", round: 1, label: "" });
  for (const agent of [a1, a2, a3]) send({ kind: "start", agent, round: 1 });
  send({
    kind: "done",
    agent: a2,
    round: 1,
    text: "## Short answer\n\n**Yes, with a caveat.** SQLite handles far more than most people expect: a single writer, but thousands of concurrent readers, and databases of hundreds of GB.\n\n| Workload | SQLite | Postgres |\n|---|---|---|\n| Read-heavy web app | ✅ great | ✅ great |\n| Many concurrent writers | ⚠️ WAL helps | ✅ |\n| Multi-server | ❌ | ✅ |",
  });
  send({ kind: "tool", agent: a3, round: 1, name: "web_search", detail: "sqlite production concurrency limits 2026" });
  send({ kind: "tool", agent: a3, round: 1, name: "fetch_url", detail: "https://www.sqlite.org/whentouse.html" });
  send({ kind: "thinking", agent: a3, round: 1, text: "The official 'appropriate uses' page is the best source here. It says SQLite works well for most low to medium traffic websites…" });
  const partial = "## It depends on your write load\n\nFor a side project or an internal tool, **SQLite in WAL mode** is a fantastic default:\n\n- zero ops, a single file to back up\n- reads scale with cores";
  for (const chunk of partial.match(/.{1,12}/gs) ?? []) {
    send({ kind: "delta", agent: a1, round: 1, text: chunk });
    await wait(5);
  }
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
  start_run: ({ config, onEvent }) => {
    liveRun(config, onEvent);
    return "demo-run";
  },
  cancel_run: () => null,
};

export async function invoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  const handler = handlers[cmd];
  if (!handler) throw new Error(`commande non simulée : ${cmd}`);
  return (await handler(args)) as T;
}
