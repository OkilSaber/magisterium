import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { ask, open } from "@tauri-apps/plugin-dialog";
import {
  AgentSpec,
  ConversationSummary,
  ExecMode,
  Mode,
  ProviderId,
  ProviderInfo,
  RunEvent,
  SYNTH_KEY,
  ProviderUsage,
  WebConfig,
  cancelRun,
  getUsage,
  deleteConversation,
  getWebConfig,
  listConversations,
  listProviders,
  loadConversation,
  refreshProvider,
  saveConversation,
  startRun,
} from "./api";
import { Conversation, Output, OutputStatus, Turn, historyOf, markInterrupted, newConversation, stopTurn } from "./conversation";
import TurnView from "./TurnView";
import UsagePanel, { level, peakUsage } from "./Usage";
import Settings, { Badge } from "./Settings";
import { T, useI18n } from "./i18n";
import { presetById } from "./presets";
import * as Icon from "./Icons";
import "./App.css";

interface Selection {
  key: string;
  provider: ProviderId;
  model: string;
  effort: string;
}

let nextKey = 1;
const newKey = () => `agent-${nextKey++}`;

function modelLabel(t: T, m: { id: string; label: string; loaded?: boolean }) {
  if (m.id === "") return t("modelDefault");
  // ● : déjà en mémoire, répond sans temps de chargement.
  return m.loaded ? `● ${m.label}` : m.label;
}

function labelFor(t: T, providers: ProviderInfo[], s: { provider: ProviderId; model: string }) {
  const p = providers.find((x) => x.id === s.provider);
  const m = p?.models.find((x) => x.id === s.model);
  // Nom stable dans l'historique : sans le marqueur « chargé » de la liste.
  const model = m ? (m.id === "" ? t("modelDefault") : m.label) : s.model || t("modelDefault");
  return `${p?.name ?? s.provider} · ${model}`;
}

/// Libellés uniques, même si le même modèle est ajouté deux fois.
function toSpecs(t: T, providers: ProviderInfo[], selections: Selection[]): AgentSpec[] {
  const seen: Record<string, number> = {};
  return selections.map((s) => {
    const base = labelFor(t, providers, s);
    seen[base] = (seen[base] ?? 0) + 1;
    return {
      key: s.key,
      provider: s.provider,
      model: s.model,
      effort: s.effort,
      label: seen[base] > 1 ? `${base} #${seen[base]}` : base,
    };
  });
}

function defaultModel(p: ProviderInfo) {
  return p.models[0]?.id ?? "";
}

/// Composition du conseil, retrouvée à la réouverture de l'app.
interface SavedCouncil {
  selections: Omit<Selection, "key">[];
  mode: Mode;
  rounds: number;
  synthOn: boolean;
  synth: { provider: ProviderId; model: string } | null;
}

function loadCouncil(): SavedCouncil | null {
  try {
    return JSON.parse(localStorage.getItem("council") ?? "null");
  } catch {
    return null;
  }
}

/// Garde le modèle enregistré s'il existe encore, sinon retombe sur le premier.
function keepModel(p: ProviderInfo, model: string) {
  return p.models.some((m) => m.id === model) ? model : defaultModel(p);
}

/// État booléen persistant (panneaux ouverts/fermés).
function useStoredFlag(key: string, initial: boolean) {
  const [value, setValue] = useState(() => {
    const raw = localStorage.getItem(key);
    return raw === null ? initial : raw === "1";
  });
  useEffect(() => localStorage.setItem(key, value ? "1" : "0"), [key, value]);
  return [value, setValue] as const;
}

export default function App() {
  const { t, lang } = useI18n();
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [loadingProviders, setLoadingProviders] = useState(true);
  const [refreshing, setRefreshing] = useState<ProviderId | null>(null);
  const [selections, setSelections] = useState<Selection[]>([]);
  const [prompt, setPrompt] = useState("");
  const saved = useMemo(loadCouncil, []);
  const [mode, setMode] = useState<Mode>(saved?.mode ?? "compare");
  const [rounds, setRounds] = useState(saved?.rounds ?? 2);
  const [synthOn, setSynthOn] = useState(saved?.synthOn ?? true);
  const [synth, setSynth] = useState<{ provider: ProviderId; model: string } | null>(null);
  const [workdir, setWorkdir] = useState(() => localStorage.getItem("workdir") ?? "");
  const [execMode, setExecMode] = useState<ExecMode>(() => (localStorage.getItem("execMode") as ExecMode) ?? "edit");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [webOn, setWebOn] = useState(() => localStorage.getItem("web") !== "0");
  const [webConfig, setWebConfig] = useState<WebConfig | null>(null);
  const [usage, setUsage] = useState<ProviderUsage[]>([]);
  const [usageLoading, setUsageLoading] = useState(false);
  const [usageUpdatedAt, setUsageUpdatedAt] = useState<number | null>(null);
  const [usageOpen, setUsageOpen] = useState(false);

  const [runId, setRunId] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [conv, setConv] = useState<Conversation | null>(null);
  const [convs, setConvs] = useState<ConversationSummary[]>([]);
  // Incrémenté à chaque événement qui mérite une sauvegarde (pas les deltas).
  const [saveTick, setSaveTick] = useState(0);

  const [historyOpen, setHistoryOpen] = useStoredFlag("panel.history", true);
  const [councilOpen, setCouncilOpen] = useStoredFlag("panel.council", true);
  // Les bascules au clavier sont instantanées : pas d'animation sur un raccourci.
  const [instant, setInstant] = useState(false);

  const threadEnd = useRef<HTMLDivElement>(null);
  const composerRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const initialized = useRef(false);

  const available = useMemo(() => providers.filter((p) => p.available), [providers]);

  async function refreshProviders() {
    setLoadingProviders(true);
    try {
      const list = await listProviders();
      setProviders(list);
      if (!initialized.current) {
        initialized.current = true;
        const byId = (id: ProviderId) => list.find((p) => p.id === id);
        // Conseil précédent, sans les fournisseurs qui ont disparu depuis.
        const restored = (saved?.selections ?? []).flatMap((s) => {
          const p = byId(s.provider);
          return p ? [{ ...s, key: newKey(), model: keepModel(p, s.model) }] : [];
        });
        setSelections(
          restored.length
            ? restored
            : list
                .filter((p) => p.available && p.uses_tools)
                .map((p) => ({ key: newKey(), provider: p.id, model: defaultModel(p), effort: "" })),
        );
        const savedSynth = saved?.synth && byId(saved.synth.provider);
        const first = list.find((p) => p.available);
        if (saved?.synth && savedSynth) setSynth({ provider: savedSynth.id, model: keepModel(savedSynth, saved.synth.model) });
        else if (first) setSynth({ provider: first.id, model: defaultModel(first) });
      }
    } finally {
      setLoadingProviders(false);
    }
  }

  async function refreshOne(id: ProviderId) {
    setRefreshing(id);
    try {
      const info = await refreshProvider(id);
      setProviders((list) => list.map((p) => (p.id === id ? info : p)));
    } catch (e) {
      setNotice(String(e));
    } finally {
      setRefreshing(null);
    }
  }

  const refreshConversations = () => listConversations().then(setConvs).catch((e) => setNotice(String(e)));

  const refreshWebConfig = () => getWebConfig().then(setWebConfig).catch(() => {});

  async function refreshUsage() {
    setUsageLoading(true);
    try {
      setUsage(await getUsage());
      setUsageUpdatedAt(Date.now());
    } catch {
      // Les limites sont un bonus : une erreur ici ne doit rien bloquer.
    } finally {
      setUsageLoading(false);
    }
  }

  function openUsage() {
    setUsageOpen(true);
    // Rafraîchi à l'ouverture si les chiffres datent de plus d'une minute.
    if (!usageUpdatedAt || Date.now() - usageUpdatedAt > 60_000) refreshUsage();
  }

  useEffect(() => {
    refreshProviders();
    refreshConversations();
    refreshWebConfig();
    refreshUsage();
  }, []);

  useEffect(() => {
    localStorage.setItem("web", webOn ? "1" : "0");
  }, [webOn]);

  useEffect(() => {
    localStorage.setItem("workdir", workdir);
  }, [workdir]);

  useEffect(() => {
    localStorage.setItem("execMode", execMode);
  }, [execMode]);

  useEffect(() => {
    // Rien à enregistrer tant que le conseil n'a pas été restauré.
    if (!initialized.current) return;
    const council: SavedCouncil = {
      selections: selections.map(({ key: _key, ...rest }) => rest),
      mode,
      rounds,
      synthOn,
      synth,
    };
    localStorage.setItem("council", JSON.stringify(council));
  }, [selections, mode, rounds, synthOn, synth]);

  useEffect(() => {
    if (!conv || saveTick === 0) return;
    saveConversation({ ...conv, updated_at: Date.now() })
      .then(refreshConversations)
      .catch((e) => setNotice(t("saveError", { e: String(e) })));
  }, [saveTick]);

  useEffect(() => {
    threadEnd.current?.scrollIntoView({ behavior: "smooth" });
  }, [conv?.id, conv?.turns.length]);

  // Le fil défile sous le compositeur flottant : on réserve sa hauteur.
  useLayoutEffect(() => {
    const el = composerRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() =>
      document.documentElement.style.setProperty("--composer-h", `${el.offsetHeight}px`),
    );
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!e.metaKey || e.shiftKey || e.altKey) return;
      if (e.key === "b") {
        e.preventDefault();
        setInstant(true);
        setHistoryOpen((o) => !o);
      } else if (e.key === "j") {
        e.preventDefault();
        setInstant(true);
        setCouncilOpen((o) => !o);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  function togglePanel(which: "history" | "council") {
    setInstant(false);
    (which === "history" ? setHistoryOpen : setCouncilOpen)((o) => !o);
  }

  // Le champ grandit avec le texte, jusqu'à 240 px.
  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 240)}px`;
  }, [prompt]);

  async function openConversation(id: string) {
    setNotice(null);
    try {
      setConv(markInterrupted(await loadConversation<Conversation>(id)));
    } catch (e) {
      setNotice(String(e));
    }
  }

  async function removeConversation(id: string) {
    await deleteConversation(id).catch((e) => setNotice(String(e)));
    if (conv?.id === id) setConv(null);
    refreshConversations();
  }

  function addAgent(p: ProviderInfo) {
    setSelections((s) => [...s, { key: newKey(), provider: p.id, model: defaultModel(p), effort: "" }]);
  }

  function updateAgent(key: string, patch: Partial<Selection>) {
    setSelections((s) => s.map((x) => (x.key === key ? { ...x, ...patch } : x)));
  }

  function removeAgent(key: string) {
    setSelections((s) => s.filter((x) => x.key !== key));
  }

  async function pickWorkdir() {
    const dir = await open({ directory: true, defaultPath: workdir || undefined });
    if (typeof dir === "string") setWorkdir(dir);
  }

  /// Applique une modification au tour en cours (le dernier de la discussion).
  function updateLastTurn(fn: (t: Turn) => Turn) {
    setConv((c) => (c && c.turns.length ? { ...c, turns: [...c.turns.slice(0, -1), fn(c.turns[c.turns.length - 1])] } : c));
  }

  function onEvent(e: RunEvent) {
    if (e.kind !== "delta" && e.kind !== "thinking") setSaveTick((t) => t + 1);
    if (e.kind === "finished") {
      setRunId(null);
      refreshUsage();
      updateLastTurn((t) => (e.cancelled ? stopTurn(t) : { ...t, status: "done" }));
      return;
    }
    if (e.kind === "phase") {
      updateLastTurn((t) => ({ ...t, rounds: [...t.rounds, { round: e.round, label: e.label, outputs: {} }] }));
      return;
    }
    updateLastTurn((t) => ({
      ...t,
      rounds: t.rounds.map((r) => {
        if (r.round !== e.round) return r;
        const prev = r.outputs[e.agent] ?? { text: "", status: "running" as OutputStatus };
        let next: Output;
        switch (e.kind) {
          case "start":
            next = { text: "", status: "running" };
            break;
          case "delta":
            next = { ...prev, text: prev.text + e.text };
            break;
          case "thinking":
            next = { ...prev, thinking: (prev.thinking ?? "") + e.text };
            break;
          case "tool":
            next = { ...prev, tools: [...(prev.tools ?? []), { name: e.name, detail: e.detail }] };
            break;
          case "done":
            next = { ...prev, text: e.text, status: "done" };
            break;
          case "error":
            next = { ...prev, status: "error", error: e.message };
            break;
        }
        return { ...r, outputs: { ...r.outputs, [e.agent]: next } };
      }),
    }));
  }

  async function launch() {
    setNotice(null);
    const text = prompt.trim();
    // Une IA dont le fournisseur a disparu (supprimé, CLI désinstallée) ne part pas.
    const agents = toSpecs(t, providers, usable);
    const synthesizer =
      synthOn && synth && providers.some((p) => p.id === synth.provider)
        ? { key: SYNTH_KEY, provider: synth.provider, model: synth.model, effort: "", label: labelFor(t, providers, synth) }
        : null;
    const turn: Turn = {
      id: crypto.randomUUID(),
      prompt: text,
      created_at: Date.now(),
      mode,
      agents,
      synthesizer,
      synth_round: mode === "compare" ? 2 : rounds + 1,
      rounds: [],
      status: "running",
    };
    const history = historyOf(conv);
    const base = conv ?? newConversation(text);
    setConv({ ...base, turns: [...base.turns, turn] });
    setPrompt("");
    try {
      const id = await startRun(
        {
          prompt: text,
          agents,
          mode,
          rounds,
          synthesizer,
          workdir,
          exec_mode: execMode,
          lang,
          history,
          web: webOn,
          today: new Date().toLocaleDateString("sv-SE"),
        },
        onEvent,
      );
      setRunId(id);
    } catch (e) {
      // Rien n'a été lancé : on retire le tour et on rend le prompt.
      setConv(base.turns.length ? base : null);
      setPrompt(text);
      setNotice(String(e));
    }
  }

  const colorFor = (id: ProviderId) => {
    const p = providers.find((x) => x.id === id);
    return !p || p.preset === "cli" ? "#a48bff" : presetById(p.preset).color;
  };
  const running = runId !== null;
  const webReady =
    !!webConfig &&
    ((webConfig.engine === "tavily" && webConfig.has_tavily_key) ||
      (webConfig.engine === "searxng" && webConfig.searxng_url !== "") ||
      (webConfig.engine === "builtin" && webConfig.builtin_enabled));
  // Une IA dont le fournisseur a disparu (supprimé, CLI désinstallée) ne compte pas.
  const usable = selections.filter((s) => providers.some((p) => p.id === s.provider));
  const canLaunch = !running && prompt.trim() !== "" && usable.length > 0;
  const appClass = [
    "app",
    historyOpen ? "" : "history-closed",
    councilOpen ? "" : "council-closed",
    instant ? "instant" : "",
  ].join(" ");

  return (
    <div className={appClass}>
      <div className="backdrop" aria-hidden />

      <nav className="panel panel-left glass" aria-hidden={!historyOpen} inert={!historyOpen}>
        <div className="brand" data-tauri-drag-region>
          <img src="/logo.png" alt="" />
          <span>Magisterium</span>
        </div>
        <button className="new-conv" onClick={() => setConv(null)} disabled={running}>
          <Icon.Plus /> {t("newConv")}
        </button>
        <div className="conv-list">
          {convs.map((c) => (
            <div
              key={c.id}
              className={`conv-item ${conv?.id === c.id ? "on" : ""} ${running ? "locked" : ""}`}
              onClick={() => !running && openConversation(c.id)}
            >
              <span className="conv-title">{c.title}</span>
              <span className="conv-date">{formatDate(c.updated_at, lang)}</span>
              {!running && (
                <button
                  className="icon-btn conv-delete"
                  title={t("delete")}
                  onClick={async (e) => {
                    e.stopPropagation();
                    if (await ask(t("deleteConfirm", { title: c.title }), { title: "Magisterium", kind: "warning" })) {
                      removeConversation(c.id);
                    }
                  }}
                >
                  <Icon.Close />
                </button>
              )}
            </div>
          ))}
          {convs.length === 0 && <div className="muted small pad">{t("noConv")}</div>}
        </div>
      </nav>

      <main>
        <header className="topbar" data-tauri-drag-region>
          <button className="icon-btn" onClick={() => togglePanel("history")} title={t("toggleHistory")}>
            <Icon.SidebarLeft />
          </button>
          <span className="topbar-title" data-tauri-drag-region>
            {conv?.title ?? t("newConv")}
          </span>
          <button className="icon-btn" onClick={openUsage} title={t("usageTitle")}>
            <Icon.Gauge />
          </button>
          <button className="icon-btn" onClick={() => setSettingsOpen(true)} title={t("settings")}>
            <Icon.Gear />
          </button>
          <button className="icon-btn" onClick={() => togglePanel("council")} title={t("toggleCouncil")}>
            <Icon.SidebarRight />
          </button>
        </header>

        <div className="thread">
          <div className="thread-inner">
            {conv?.turns.map((t, i) => (
              <TurnView key={t.id} turn={t} isLast={i === conv.turns.length - 1} colorFor={colorFor} />
            ))}
            {!conv && (
              <div className="empty">
                <img src="/logo.png" alt="" />
                <h2>{t("emptyTitle")}</h2>
                <p>{t("emptyText")}</p>
              </div>
            )}
            <div ref={threadEnd} />
          </div>
        </div>

        <div className="composer-wrap">
          <div className="composer glass" ref={composerRef}>
            <textarea
              ref={textareaRef}
              autoFocus
              rows={1}
              placeholder={conv ? t("placeholderFollow") : t("placeholderNew")}
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && (e.metaKey || e.ctrlKey) && canLaunch) launch();
              }}
            />
            <div className="composer-bar">
              <span className="muted small">
                {t("summaryAgents", { n: usable.length })} ·{" "}
                {mode === "compare" ? t("summaryCompare") : t("summaryDebate", { r: rounds })}
                {synthOn && synth ? ` · ${t("summarySynth")}` : ""}
              </span>
              {notice && <span className="notice">{notice}</span>}
              {running ? (
                <button className="send stop" onClick={() => runId && cancelRun(runId)} title={t("stop")}>
                  <Icon.Stop />
                </button>
              ) : (
                <button className="send" onClick={launch} disabled={!canLaunch} title={t("send")}>
                  <Icon.ArrowUp />
                </button>
              )}
            </div>
          </div>
        </div>
      </main>

      <aside className="panel panel-right glass" aria-hidden={!councilOpen} inert={!councilOpen}>
        <section>
          <div className="section-title">
            <h3>{t("council")}</h3>
            <button className="icon-btn" onClick={refreshProviders} disabled={loadingProviders} title={t("refreshAll")}>
              <span className={loadingProviders ? "spin" : ""}>
                <Icon.Refresh />
              </span>
            </button>
          </div>
          {providers.map((p) => (
            <div key={p.id} className={`provider ${p.available ? "" : "off"}`}>
              <div className="provider-head">
                <Badge name={p.name} color={p.preset === "cli" ? "#a48bff" : presetById(p.preset).color} />
                <span className="provider-name">{p.name}</span>
                {p.preset !== "cli" && (
                  <button
                    className="icon-btn"
                    onClick={() => refreshOne(p.id)}
                    disabled={refreshing === p.id}
                    title={t("refreshModels")}
                  >
                    <span className={refreshing === p.id ? "spin" : ""}>
                      <Icon.Refresh />
                    </span>
                  </button>
                )}
                {p.available && (
                  <button className="icon-btn accent" onClick={() => addAgent(p)} disabled={running} title={t("addToCouncil")}>
                    <Icon.Plus />
                  </button>
                )}
              </div>
              {p.status && <div className="muted small provider-status">{p.status}</div>}
              <MiniUsage usage={usage.find((u) => u.id === p.id)} onOpen={openUsage} />
              {selections
                .filter((s) => s.provider === p.id)
                .map((s) => (
                  <div key={s.key} className="agent-row">
                    <div className="agent-selects">
                      <select value={s.model} onChange={(e) => updateAgent(s.key, { model: e.target.value })} disabled={running}>
                        {p.models.map((m) => (
                          <option key={m.id} value={m.id}>
                            {modelLabel(t, m)}
                          </option>
                        ))}
                      </select>
                      {p.efforts.length > 0 && (
                        <select value={s.effort} onChange={(e) => updateAgent(s.key, { effort: e.target.value })} disabled={running}>
                          <option value="">{t("effortDefault")}</option>
                          {p.efforts.map((level) => (
                            <option key={level} value={level}>
                              {t(`effort_${level}` as Parameters<T>[0])}
                            </option>
                          ))}
                        </select>
                      )}
                    </div>
                    <button className="icon-btn" onClick={() => removeAgent(s.key)} disabled={running} title={t("remove")}>
                      <Icon.Close />
                    </button>
                  </div>
                ))}
            </div>
          ))}
          {!loadingProviders && providers.length === 0 && <div className="muted small pad">{t("noProviders")}</div>}
          <button className="new-conv add-provider" onClick={() => setSettingsOpen(true)}>
            <Icon.Plus /> {t("addProvider")}
          </button>
        </section>

        <section>
          <h3>{t("deliberation")}</h3>
          <Segmented
            value={mode}
            options={[
              { value: "compare", label: t("compare") },
              { value: "debate", label: t("debate") },
            ]}
            onChange={setMode}
            disabled={running}
          />
          {mode === "debate" && (
            <label className="row">
              <span>{t("rounds")}</span>
              <input
                type="number"
                min={2}
                max={5}
                value={rounds}
                onChange={(e) => setRounds(Math.min(5, Math.max(2, Number(e.target.value))))}
                disabled={running}
              />
            </label>
          )}
          <div className="row">
            <span>{t("webSearch")}</span>
            <Switch checked={webOn} onChange={setWebOn} disabled={running} />
          </div>
          {webOn && (
            <div className="muted small pad-x">
              {webReady ? (
                t("webHelp")
              ) : (
                <>
                  {webConfig?.engine === "builtin" ? t("webBuiltinOff") : t("webNoEngine")}{" "}
                  <button className="link" onClick={() => setSettingsOpen(true)}>
                    {t("webConfigure")}
                  </button>
                </>
              )}
            </div>
          )}
          <div className="row">
            <span>{t("synthesis")}</span>
            <Switch checked={synthOn} onChange={setSynthOn} disabled={running} />
          </div>
          {synthOn && (
            <select
              value={synth ? `${synth.provider}|${synth.model}` : ""}
              onChange={(e) => {
                const [provider, ...rest] = e.target.value.split("|");
                setSynth({ provider: provider as ProviderId, model: rest.join("|") });
              }}
              disabled={running}
            >
              {available.flatMap((p) =>
                p.models.map((m) => (
                  <option key={`${p.id}|${m.id}`} value={`${p.id}|${m.id}`}>
                    {p.name} · {modelLabel(t, m)}
                  </option>
                )),
              )}
            </select>
          )}
        </section>

        <section>
          <h3>{t("workspace")}</h3>
          <div className="path-row">
            <button className={`path ${workdir ? "" : "unset"}`} onClick={pickWorkdir} disabled={running} title={workdir}>
              <Icon.Folder />
              <span className="path-text">{workdir ? <bdi>{workdir}</bdi> : t("chooseFolder")}</span>
            </button>
            {workdir && (
              <button className="icon-btn" onClick={() => setWorkdir("")} disabled={running} title={t("clearFolder")}>
                <Icon.Close />
              </button>
            )}
          </div>
          <h3 className="sub">{t("execMode")}</h3>
          <Segmented
            value={execMode}
            options={[
              { value: "plan", label: t("modePlan") },
              { value: "edit", label: t("modeEdit") },
              { value: "auto", label: t("modeAuto") },
              { value: "full", label: t("modeFull"), danger: true },
            ]}
            onChange={setExecMode}
            disabled={running}
          />
          <div className={`small ${execMode === "full" ? "warning" : "muted"} pad-x`}>
            {t(({ plan: "helpPlan", edit: "helpEdit", auto: "helpAuto", full: "helpFull" } as const)[execMode])}
          </div>
          {providers.some((p) => p.preset !== "cli") && <div className="muted small pad-x">{t("modesCliOnly")}</div>}
        </section>
      </aside>

      {usageOpen && (
        <UsagePanel
          usage={usage}
          loading={usageLoading}
          updatedAt={usageUpdatedAt}
          onRefresh={refreshUsage}
          onClose={() => setUsageOpen(false)}
        />
      )}

      {settingsOpen && (
        <Settings
          providers={providers}
          onClose={() => {
            setSettingsOpen(false);
            refreshWebConfig();
          }}
          onChanged={refreshProviders}
        />
      )}
    </div>
  );
}

/// Jauge compacte dans la carte du fournisseur : la limite la plus proche d'être atteinte.
function MiniUsage({ usage, onOpen }: { usage?: ProviderUsage; onOpen: () => void }) {
  const { t } = useI18n();
  const peak = peakUsage(usage);
  if (peak == null) return null;
  return (
    <button className="mini-usage" onClick={onOpen} title={t("usageTitle")}>
      <span className="meter-track">
        <span className={`meter-fill ${level(peak)}`} style={{ transform: `scaleX(${Math.max(0.01, peak)})` }} />
      </span>
      <span className={`meter-pct ${level(peak)}`}>{t("usageUsed", { pct: Math.round(peak * 100) })}</span>
    </button>
  );
}

function Segmented<T extends string>({
  value,
  options,
  onChange,
  disabled,
}: {
  value: T;
  options: { value: T; label: string; danger?: boolean }[];
  onChange: (v: T) => void;
  disabled?: boolean;
}) {
  const index = Math.max(0, options.findIndex((o) => o.value === value));
  return (
    <div className="segmented" style={{ ["--count" as string]: options.length }}>
      <span
        className={`segmented-thumb ${options[index]?.danger ? "danger" : ""}`}
        style={{ transform: `translateX(${index * 100}%)` }}
      />
      {options.map((o) => (
        <button key={o.value} className={o.value === value ? "on" : ""} onClick={() => onChange(o.value)} disabled={disabled}>
          {o.label}
        </button>
      ))}
    </div>
  );
}

function Switch({ checked, onChange, disabled }: { checked: boolean; onChange: (v: boolean) => void; disabled?: boolean }) {
  return (
    <button
      role="switch"
      aria-checked={checked}
      className={`switch ${checked ? "on" : ""}`}
      onClick={() => onChange(!checked)}
      disabled={disabled}
    >
      <span />
    </button>
  );
}

function formatDate(ms: number, lang: string) {
  const d = new Date(ms);
  const sameDay = d.toDateString() === new Date().toDateString();
  return sameDay
    ? d.toLocaleTimeString(lang, { hour: "2-digit", minute: "2-digit" })
    : d.toLocaleDateString(lang, { day: "numeric", month: "short" });
}
