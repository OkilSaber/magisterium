import { Channel, invoke } from "@tauri-apps/api/core";

export type ProviderId = string;
export type ExecMode = "plan" | "edit" | "auto" | "full";
export type Mode = "compare" | "debate";
export type ApiKind = "openai" | "anthropic";

export interface ModelInfo {
  id: string;
  label: string;
  /// Déjà chargé en mémoire (serveurs LM Studio uniquement).
  loaded?: boolean;
}

export interface ProviderInfo {
  id: ProviderId;
  name: string;
  /// « cli » pour une CLI détectée, sinon le préréglage API d'origine.
  preset: string;
  available: boolean;
  status: string | null;
  models: ModelInfo[];
  uses_tools: boolean;
  efforts: string[];
}

export interface ProviderConfig {
  id: string;
  name: string;
  kind: ApiKind;
  preset: string;
  base_url: string;
  has_key: boolean;
}

export interface AgentSpec {
  key: string;
  label: string;
  provider: ProviderId;
  model: string;
  effort: string;
}

export interface Exchange {
  prompt: string;
  answer: string;
}

export interface ConversationSummary {
  id: string;
  title: string;
  updated_at: number;
}

export interface RunConfig {
  prompt: string;
  agents: AgentSpec[];
  mode: Mode;
  rounds: number;
  synthesizer: AgentSpec | null;
  workdir: string;
  exec_mode: ExecMode;
  lang: string;
  history: Exchange[];
  /// Outils web pour les modèles API et locaux.
  web: boolean;
  today: string;
}

export type WebEngine = "none" | "builtin" | "tavily" | "searxng";

export interface WebConfig {
  engine: WebEngine;
  searxng_url: string;
  has_tavily_key: boolean;
  builtin_enabled: boolean;
}

export interface SearxngStatus {
  installed: boolean;
  enabled: boolean;
  installing: boolean;
  starting: boolean;
  port: number | null;
  error: string | null;
  size_bytes: number;
}

export type InstallStep = "uv" | "python" | "searxng" | "deps" | "config" | "verify";

export interface InstallProgress {
  step: InstallStep;
  index: number;
  total: number;
  state: "running" | "done" | "skipped" | "error";
  bytes: number | null;
  total_bytes: number | null;
  message: string | null;
}

export type RunEvent =
  | { kind: "phase"; round: number; label: string }
  | { kind: "start"; agent: string; round: number }
  | { kind: "delta"; agent: string; round: number; text: string }
  | { kind: "thinking"; agent: string; round: number; text: string }
  | { kind: "tool"; agent: string; round: number; name: string; detail: string }
  | { kind: "done"; agent: string; round: number; text: string }
  | { kind: "error"; agent: string; round: number; message: string }
  | { kind: "finished"; cancelled: boolean };

export const SYNTH_KEY = "synthese";

export const listProviders = () => invoke<ProviderInfo[]>("list_providers");

export const refreshProvider = (provider: ProviderId) =>
  invoke<ProviderInfo>("refresh_provider", { provider });

export const getProviderConfigs = () => invoke<ProviderConfig[]>("get_provider_configs");

/// `apiKey` : `undefined` garde la clé enregistrée, `""` la supprime.
export const saveProvider = (provider: ProviderConfig, apiKey?: string) =>
  invoke<ProviderConfig>("save_provider", { provider, apiKey: apiKey ?? null });

export const deleteProvider = (id: string) => invoke<void>("delete_provider", { id });

export const testProvider = (provider: ProviderConfig, apiKey?: string) =>
  invoke<ModelInfo[]>("test_provider", { provider, apiKey: apiKey ?? null });

export function startRun(config: RunConfig, onEvent: (e: RunEvent) => void) {
  const channel = new Channel<RunEvent>();
  channel.onmessage = onEvent;
  return invoke<string>("start_run", { config, onEvent: channel });
}

export const cancelRun = (runId: string) => invoke<void>("cancel_run", { runId });

export const getWebConfig = () => invoke<WebConfig>("get_web_config");

/// `tavilyKey` : `undefined` garde la clé enregistrée, `""` la supprime.
export const saveWebConfig = (web: WebConfig, tavilyKey?: string) =>
  invoke<WebConfig>("save_web_config", { web, tavilyKey: tavilyKey ?? null });

export const testWebSearch = (web: WebConfig, tavilyKey?: string) =>
  invoke<number>("test_web_search", { web, tavilyKey: tavilyKey ?? null });

export const searxngStatus = () => invoke<SearxngStatus>("searxng_status");
export const searxngInstall = () => invoke<SearxngStatus>("searxng_install");
export const searxngSetEnabled = (enabled: boolean) => invoke<SearxngStatus>("searxng_set_enabled", { enabled });
export const searxngUninstall = () => invoke<SearxngStatus>("searxng_uninstall");

export interface UsageMeter {
  group: string | null;
  window: string;
  used: number | null;
  amount: number | null;
  limit: number | null;
  unit: string | null;
  resets_at: string | null;
  resets_text: string | null;
}

export interface ProviderUsage {
  id: string;
  name: string;
  preset: string;
  status: "ok" | "unlimited" | "unavailable" | "error";
  meters: UsageMeter[];
  message: string | null;
  dashboard: string | null;
}

export const getUsage = () => invoke<ProviderUsage[]>("get_usage");

export const listConversations = () => invoke<ConversationSummary[]>("list_conversations");

export const loadConversation = <T,>(id: string) => invoke<T>("load_conversation", { id });

export const saveConversation = (conversation: unknown) =>
  invoke<void>("save_conversation", { conversation });

export const deleteConversation = (id: string) => invoke<void>("delete_conversation", { id });
