import { AgentSpec, Exchange, Mode, SYNTH_KEY } from "./api";

export type OutputStatus = "running" | "done" | "error" | "stopped";

export interface Output {
  text: string;
  status: OutputStatus;
  error?: string;
  /// Raisonnement du modèle, quand le fournisseur le transmet.
  thinking?: string;
  /// Recherches et pages lues par le modèle.
  tools?: { name: string; detail: string }[];
}

export interface Round {
  round: number;
  label: string;
  outputs: Record<string, Output>;
}

export interface Turn {
  id: string;
  prompt: string;
  created_at: number;
  mode: Mode;
  agents: AgentSpec[];
  synthesizer: AgentSpec | null;
  /// Numéro du tour de synthèse (dernier tour de débat + 1).
  synth_round: number;
  rounds: Round[];
  status: "running" | "done" | "cancelled";
}

export interface Conversation {
  id: string;
  title: string;
  created_at: number;
  updated_at: number;
  turns: Turn[];
}

export function newConversation(firstPrompt: string): Conversation {
  const now = Date.now();
  const title = firstPrompt.trim().replace(/\s+/g, " ");
  return {
    id: crypto.randomUUID(),
    title: title.length > 60 ? `${title.slice(0, 57)}…` : title,
    created_at: now,
    updated_at: now,
    turns: [],
  };
}

/// Ce que le conseil a répondu à un tour : la synthèse, sinon les réponses
/// finales de chaque IA. `null` si le tour n'a rien produit.
export function exchangeOf(turn: Turn): Exchange | null {
  const synth = turn.rounds.find((r) => r.round === turn.synth_round)?.outputs[SYNTH_KEY];
  if (synth?.status === "done" && synth.text) {
    return { prompt: turn.prompt, answer: synth.text };
  }
  const last = [...turn.rounds].reverse().find((r) => r.round !== turn.synth_round);
  const answers = turn.agents
    .map((a) => ({ a, out: last?.outputs[a.key] }))
    .filter(({ out }) => out?.status === "done" && out.text)
    .map(({ a, out }) => `### ${a.label}\n\n${out!.text}`);
  return answers.length ? { prompt: turn.prompt, answer: answers.join("\n\n") } : null;
}

export function historyOf(conv: Conversation | null): Exchange[] {
  return (conv?.turns ?? []).map(exchangeOf).filter((e): e is Exchange => e !== null);
}

/// Arrête un tour : les IA qui n'avaient pas fini passent en « arrêté ».
export function stopTurn(t: Turn): Turn {
  return {
    ...t,
    status: "cancelled",
    rounds: t.rounds.map((r) => ({
      ...r,
      outputs: Object.fromEntries(
        Object.entries(r.outputs).map(([k, o]) => [k, o.status === "running" ? { ...o, status: "stopped" } : o]),
      ),
    })),
  };
}

/// Une discussion rouverte après un crash ou une fermeture en plein tour.
export function markInterrupted(conv: Conversation): Conversation {
  return { ...conv, turns: conv.turns.map((t) => (t.status === "running" ? stopTurn(t) : t)) };
}
