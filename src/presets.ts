import { ApiKind } from "./api";

export interface Preset {
  id: string;
  name: string;
  kind: ApiKind;
  url: string;
  /// Clé API : obligatoire, facultative, ou inutile (serveur local).
  key: "required" | "optional" | "none";
  group: "local" | "cloud";
  color: string;
}

export const PRESETS: Preset[] = [
  { id: "custom", name: "", kind: "openai", url: "http://localhost:8080/v1", key: "optional", group: "local", color: "#8b93a7" },
  { id: "lmstudio", name: "LM Studio", kind: "openai", url: "http://localhost:1234/v1", key: "none", group: "local", color: "#6b7cff" },
  { id: "ollama", name: "Ollama", kind: "openai", url: "http://localhost:11434/v1", key: "none", group: "local", color: "#e5e5e5" },
  { id: "anthropic", name: "Anthropic", kind: "anthropic", url: "https://api.anthropic.com/v1", key: "required", group: "cloud", color: "#d97757" },
  { id: "openai", name: "OpenAI", kind: "openai", url: "https://api.openai.com/v1", key: "required", group: "cloud", color: "#10a37f" },
  { id: "gemini", name: "Google Gemini", kind: "openai", url: "https://generativelanguage.googleapis.com/v1beta/openai", key: "required", group: "cloud", color: "#4f8cff" },
  { id: "deepseek", name: "DeepSeek", kind: "openai", url: "https://api.deepseek.com/v1", key: "required", group: "cloud", color: "#4d6bfe" },
  { id: "openrouter", name: "OpenRouter", kind: "openai", url: "https://openrouter.ai/api/v1", key: "required", group: "cloud", color: "#8f7cff" },
  { id: "mistral", name: "Mistral", kind: "openai", url: "https://api.mistral.ai/v1", key: "required", group: "cloud", color: "#ff7000" },
  { id: "groq", name: "Groq", kind: "openai", url: "https://api.groq.com/openai/v1", key: "required", group: "cloud", color: "#f55036" },
  { id: "xai", name: "xAI (Grok)", kind: "openai", url: "https://api.x.ai/v1", key: "required", group: "cloud", color: "#cfcfcf" },
  { id: "together", name: "Together AI", kind: "openai", url: "https://api.together.xyz/v1", key: "required", group: "cloud", color: "#0f6fff" },
  { id: "ollama-cloud", name: "Ollama Cloud", kind: "openai", url: "https://ollama.com/v1", key: "required", group: "cloud", color: "#bdbdbd" },
];

export const presetById = (id: string) => PRESETS.find((p) => p.id === id) ?? PRESETS[0];

/// Pastille colorée avec l'initiale du fournisseur.
export function monogram(name: string) {
  return (name.trim()[0] ?? "?").toUpperCase();
}
