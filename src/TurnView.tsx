import { useEffect, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { SYNTH_KEY } from "./api";
import { Output, Turn } from "./conversation";
import { Sparkle } from "./Icons";
import { useI18n } from "./i18n";

export default function TurnView({ turn, isLast }: { turn: Turn; isLast: boolean }) {
  const { t } = useI18n();
  const roundLabel = (round: number) => (round === 1 ? t("roundInitial") : t("roundN", { n: round }));
  const debateRounds = turn.rounds.filter((r) => r.round !== turn.synth_round);
  const synthRound = turn.rounds.find((r) => r.round === turn.synth_round);
  const [activeRound, setActiveRound] = useState(debateRounds.length);
  const [expanded, setExpanded] = useState(isLast);

  // Suit le tour en cours pendant le streaming.
  useEffect(() => {
    setActiveRound(debateRounds.length);
  }, [debateRounds.length]);

  // Un tour qui n'est plus le dernier se replie sur sa synthèse.
  useEffect(() => {
    if (!isLast) setExpanded(false);
  }, [isLast]);

  const shownRound =
    debateRounds.find((r) => r.round === activeRound) ?? debateRounds[debateRounds.length - 1];
  const showDebate = expanded || !synthRound;

  return (
    <section className={`turn ${turn.status === "running" ? "fresh" : ""}`}>
      <div className="user-prompt">{turn.prompt}</div>

      {synthRound && (
        <button className="toggle" onClick={() => setExpanded((e) => !e)}>
          {expanded ? t("hideDebate") : t("showDebate", { n: turn.agents.length, r: debateRounds.length })}
        </button>
      )}

      {showDebate && debateRounds.length > 1 && (
        <div className="tabs">
          {debateRounds.map((r) => (
            <button key={r.round} className={shownRound?.round === r.round ? "on" : ""} onClick={() => setActiveRound(r.round)}>
              {roundLabel(r.round)}
            </button>
          ))}
        </div>
      )}

      {showDebate && shownRound && (
        <div className="columns" style={{ gridTemplateColumns: `repeat(${Math.min(turn.agents.length, 4)}, minmax(0, 1fr))` }}>
          {turn.agents.map((a) => (
            <OutputCard key={a.key} title={a.label} output={shownRound.outputs[a.key]} />
          ))}
        </div>
      )}

      {synthRound && turn.synthesizer && (
        <div className="synthesis">
          <OutputCard title={`${t("synthesisTitle")} · ${turn.synthesizer.label}`} output={synthRound.outputs[SYNTH_KEY]} icon={<Sparkle />} />
        </div>
      )}

      {turn.status === "cancelled" && <div className="muted small">{t("interrupted")}</div>}
    </section>
  );
}

function OutputCard({ title, output, icon }: { title: string; output?: Output; icon?: React.ReactNode }) {
  const { t } = useI18n();
  const status = output?.status ?? "running";
  return (
    <article className={`card ${status}`}>
      <header>
        <span className="card-title">
          {icon}
          {title}
        </span>
        <span className="badge">
          {
            {
              running: t("statusRunning"),
              done: t("statusDone"),
              error: t("statusError"),
              stopped: t("statusStopped"),
            }[status]
          }
        </span>
      </header>
      <div className="card-body">
        {output?.tools && output.tools.length > 0 && <ToolUses tools={output.tools} />}
        {output?.thinking && <Reasoning text={output.thinking} answering={!!output.text} />}
        {output?.text ? (
          <ReactMarkdown remarkPlugins={[remarkGfm]}>{output.text}</ReactMarkdown>
        ) : status === "running" && !output?.thinking ? (
          <div className="thinking">{t("thinking")}</div>
        ) : null}
        {output?.error && <pre className="error">{output.error}</pre>}
      </div>
    </article>
  );
}

/// Raisonnement du modèle : ouvert pendant la réflexion, replié dès que la
/// réponse arrive, sauf si l'utilisateur l'a lui-même ouvert ou fermé.
function Reasoning({ text, answering }: { text: string; answering: boolean }) {
  const { t } = useI18n();
  const [manual, setManual] = useState<boolean | null>(null);
  const open = manual ?? !answering;
  return (
    <div className={`reasoning ${open ? "open" : ""}`}>
      <button className="reasoning-toggle" onClick={() => setManual(!open)}>
        <span className="chevron" aria-hidden>
          ›
        </span>
        {t("reasoning", { n: text.length.toLocaleString() })}
      </button>
      {open && <div className="reasoning-text">{text}</div>}
    </div>
  );
}

/// Recherches et pages consultées par le modèle, dans l'ordre.
function ToolUses({ tools }: { tools: { name: string; detail: string }[] }) {
  const { t } = useI18n();
  return (
    <ul className="tool-uses">
      {tools.map((tool, i) => (
        <li key={i} title={tool.detail}>
          <span className="tool-kind">{tool.name === "web_search" ? t("toolSearch") : t("toolFetch")}</span>
          <span className="tool-detail">{tool.name === "fetch_url" ? shortUrl(tool.detail) : tool.detail}</span>
        </li>
      ))}
    </ul>
  );
}

function shortUrl(url: string) {
  try {
    const u = new URL(url);
    return `${u.hostname.replace(/^www\./, "")}${u.pathname === "/" ? "" : u.pathname}`;
  } catch {
    return url;
  }
}
