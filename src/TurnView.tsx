import { KeyboardEvent, useEffect, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { AgentSpec, SYNTH_KEY } from "./api";
import { Output, Round, Turn } from "./conversation";
import { Search, Sparkle } from "./Icons";
import { TKey, useI18n } from "./i18n";
import { Badge } from "./Settings";

type Layout = "tabs" | "side";

/// « Claude Code CLI · Opus » → nom du fournisseur et modèle, affichés sur deux lignes.
function splitLabel(label: string) {
  const [name, ...rest] = label.split(" · ");
  return { name, model: rest.join(" · ") };
}

function StatusDot({ output }: { output?: Output }) {
  return <span className={`status-dot ${output?.status ?? "pending"}`} aria-hidden />;
}

export default function TurnView({
  turn,
  isLast,
  colorFor,
}: {
  turn: Turn;
  isLast: boolean;
  /// Couleur de la pastille d'un fournisseur (préréglage, violet pour les CLI).
  colorFor: (providerId: string) => string;
}) {
  const { t } = useI18n();
  const debateRounds = turn.rounds.filter((r) => r.round !== turn.synth_round);
  const synthRound = turn.rounds.find((r) => r.round === turn.synth_round);
  const totalRounds = Math.max(1, turn.synth_round - 1);
  const [activeRound, setActiveRound] = useState(debateRounds.length);
  const [open, setOpen] = useState(isLast || !turn.synthesizer);
  const [layout, setLayout] = useState<Layout>(() => (localStorage.getItem("turn.layout") as Layout) ?? "tabs");
  const [selected, setSelected] = useState(turn.agents[0]?.key);

  // Suit le tour de débat en cours pendant le streaming.
  useEffect(() => {
    setActiveRound(debateRounds.length);
  }, [debateRounds.length]);

  // Un tour qui n'est plus le dernier se replie sur son verdict.
  useEffect(() => {
    if (!isLast && turn.synthesizer) setOpen(false);
  }, [isLast]);

  function changeLayout(next: Layout) {
    setLayout(next);
    localStorage.setItem("turn.layout", next);
  }

  const shownRound = debateRounds.find((r) => r.round === activeRound) ?? debateRounds[debateRounds.length - 1];
  const roundLabel = (round: number) => (round === 1 ? t("roundInitial") : t("roundN", { n: round }));

  return (
    <section className={`turn ${turn.status === "running" ? "fresh" : ""}`}>
      <div className="user-prompt">{turn.prompt}</div>

      {turn.synthesizer && (
        <Verdict
          turn={turn}
          synthesizer={turn.synthesizer}
          output={synthRound?.outputs[SYNTH_KEY]}
          currentRound={shownRound}
          roundCount={debateRounds.length}
          totalRounds={totalRounds}
        />
      )}

      {debateRounds.length > 0 && (
        <div className={`debate ${open ? "open" : ""}`}>
          <button className="debate-head" onClick={() => setOpen((o) => !o)} aria-expanded={open}>
            <span className="chevron" aria-hidden>
              ›
            </span>
            {t(debateRounds.length === 1 ? "debateHeaderOne" : "debateHeader", { n: turn.agents.length, r: debateRounds.length })}
          </button>

          {open && (
            <div className="debate-body">
              <div className="debate-toolbar">
                {debateRounds.length > 1 ? (
                  <div className="tabs">
                    {debateRounds.map((r) => (
                      <button key={r.round} className={shownRound?.round === r.round ? "on" : ""} onClick={() => setActiveRound(r.round)}>
                        {roundLabel(r.round)}
                      </button>
                    ))}
                  </div>
                ) : (
                  <span />
                )}
                {turn.agents.length > 1 && (
                  <button className="layout-toggle" onClick={() => changeLayout(layout === "tabs" ? "side" : "tabs")}>
                    {layout === "tabs" ? `⇆ ${t("layoutSideBySide")}` : `☰ ${t("layoutOneByOne")}`}
                  </button>
                )}
              </div>

              {shownRound &&
                (layout === "tabs" || turn.agents.length === 1 ? (
                  <MemberTabs
                    agents={turn.agents}
                    round={shownRound}
                    selected={selected}
                    onSelect={setSelected}
                    colorFor={colorFor}
                  />
                ) : (
                  <SideBySide
                    agents={turn.agents}
                    round={shownRound}
                    colorFor={colorFor}
                    onReadFull={(key) => {
                      setSelected(key);
                      changeLayout("tabs");
                    }}
                  />
                ))}
            </div>
          )}
        </div>
      )}

      {turn.status === "cancelled" && <div className="muted small">{t("interrupted")}</div>}
    </section>
  );
}

/// La synthèse, en tête du tour. Tant qu'elle n'a pas commencé, montre où en est le débat.
function Verdict({
  turn,
  synthesizer,
  output,
  currentRound,
  roundCount,
  totalRounds,
}: {
  turn: Turn;
  synthesizer: AgentSpec;
  output?: Output;
  currentRound?: Round;
  roundCount: number;
  totalRounds: number;
}) {
  const { t } = useI18n();

  if (!output) {
    if (turn.status !== "running") return null;
    const done = turn.agents.filter((a) => {
      const s = currentRound?.outputs[a.key]?.status;
      return s && s !== "running";
    }).length;
    const n = Math.max(1, turn.agents.length);
    // Avancement global : tours déjà finis + part du tour en cours.
    const progress = Math.min(1, (Math.max(0, roundCount - 1) + done / n) / totalRounds);
    return (
      <article className="card verdict verdict-pending">
        <header>
          <span className="card-title">
            <Sparkle />
            {t("verdict")} · {synthesizer.label}
          </span>
        </header>
        <div className="card-body">
          <div className="deliberating">{t("deliberating")}</div>
          <span className="meter-track">
            <span className="meter-fill low" style={{ transform: `scaleX(${Math.max(0.03, progress)})` }} />
          </span>
          <div className="muted small">
            {t("deliberationProgress", { r: Math.max(1, roundCount), total: totalRounds, done, n: turn.agents.length })}
          </div>
        </div>
      </article>
    );
  }

  return (
    <article className={`card verdict ${output.status}`}>
      <header>
        <span className="card-title">
          <Sparkle />
          {t("verdict")} · {synthesizer.label}
        </span>
        <StatusBadge output={output} />
      </header>
      <div className="card-body">
        <Answer output={output} />
      </div>
    </article>
  );
}

function MemberTabs({
  agents,
  round,
  selected,
  onSelect,
  colorFor,
}: {
  agents: AgentSpec[];
  round: Round;
  selected?: string;
  onSelect: (key: string) => void;
  colorFor: (providerId: string) => string;
}) {
  const current = agents.find((a) => a.key === selected) ?? agents[0];
  const { t } = useI18n();

  function onKeyDown(e: KeyboardEvent) {
    if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
    e.preventDefault();
    const rtl = document.documentElement.dir === "rtl";
    const step = (e.key === "ArrowRight") !== rtl ? 1 : -1;
    const index = agents.findIndex((a) => a.key === current.key);
    const next = agents[(index + step + agents.length) % agents.length];
    onSelect(next.key);
    document.getElementById(`member-tab-${next.key}`)?.focus();
  }

  return (
    <div className="members">
      <div className="member-tabs" role="tablist" onKeyDown={onKeyDown}>
        {agents.map((a) => {
          const { name, model } = splitLabel(a.label);
          const output = round.outputs[a.key];
          const searches = output?.tools?.length ?? 0;
          const on = a.key === current.key;
          return (
            <button
              key={a.key}
              id={`member-tab-${a.key}`}
              role="tab"
              aria-selected={on}
              tabIndex={on ? 0 : -1}
              className={`member-tab ${on ? "on" : ""}`}
              onClick={() => onSelect(a.key)}
            >
              <Badge name={name} color={colorFor(a.provider)} />
              <span className="member-names">
                <span className="member-name">{name}</span>
                {model && <span className="member-model">{model}</span>}
              </span>
              {searches > 0 && (
                <span className="member-web" title={t("webUses", { n: searches })}>
                  <Search /> {searches}
                </span>
              )}
              <StatusDot output={output} />
            </button>
          );
        })}
      </div>
      <div className="member-panel" role="tabpanel">
        <Answer output={round.outputs[current.key]} />
      </div>
    </div>
  );
}

function SideBySide({
  agents,
  round,
  colorFor,
  onReadFull,
}: {
  agents: AgentSpec[];
  round: Round;
  colorFor: (providerId: string) => string;
  onReadFull: (key: string) => void;
}) {
  const { t } = useI18n();
  return (
    <div className="side-by-side">
      {agents.map((a) => {
        const { name, model } = splitLabel(a.label);
        const output = round.outputs[a.key];
        return (
          <article key={a.key} className="side-card">
            <header>
              <Badge name={name} color={colorFor(a.provider)} />
              <span className="member-names">
                <span className="member-name">{name}</span>
                {model && <span className="member-model">{model}</span>}
              </span>
              <StatusDot output={output} />
            </header>
            <div className="side-card-body">
              <Answer output={output} />
            </div>
            <div className="side-card-fade">
              <button className="pill" onClick={() => onReadFull(a.key)}>
                {t("readFull")}
              </button>
            </div>
          </article>
        );
      })}
    </div>
  );
}

function StatusBadge({ output }: { output?: Output }) {
  const { t } = useI18n();
  const status = output?.status ?? "running";
  return (
    <span className={`badge ${status}`}>
      {
        {
          running: t("statusRunning"),
          done: t("statusDone"),
          error: t("statusError"),
          stopped: t("statusStopped"),
        }[status]
      }
    </span>
  );
}

/// Contenu d'une réponse : outils utilisés, raisonnement, texte, erreur.
function Answer({ output }: { output?: Output }) {
  const { t } = useI18n();
  const status = output?.status ?? "running";
  return (
    <div className="answer">
      {output?.tools && output.tools.length > 0 && <ToolUses tools={output.tools} />}
      {output?.thinking && <Reasoning text={output.thinking} answering={!!output.text} />}
      {output?.text ? (
        <ReactMarkdown remarkPlugins={[remarkGfm]}>{output.text}</ReactMarkdown>
      ) : status === "running" && !output?.thinking ? (
        <div className="thinking">{t("thinking")}</div>
      ) : null}
      {output?.error && <pre className="error">{output.error}</pre>}
    </div>
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

const TOOL_KINDS: Record<string, TKey> = {
  web_search: "toolSearch",
  fetch_url: "toolFetch",
  list_dir: "toolListDir",
  read_file: "toolReadFile",
  search_files: "toolSearchFiles",
};

/// Recherches, pages et fichiers consultés par le modèle, dans l'ordre.
function ToolUses({ tools }: { tools: { name: string; detail: string }[] }) {
  const { t } = useI18n();
  return (
    <ul className="tool-uses">
      {tools.map((tool, i) => (
        <li key={i} title={tool.detail}>
          <span className="tool-kind">{t(TOOL_KINDS[tool.name] ?? "toolFetch")}</span>
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
