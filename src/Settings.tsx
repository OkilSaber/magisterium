import { useEffect, useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import {
  ApiKind,
  ProviderConfig,
  ProviderInfo,
  InstallProgress,
  SearxngStatus,
  WebConfig,
  WebEngine,
  deleteProvider,
  getProviderConfigs,
  getWebConfig,
  saveProvider,
  saveWebConfig,
  testProvider,
  searxngInstall,
  searxngSetEnabled,
  searxngStatus,
  searxngUninstall,
  testWebSearch,
} from "./api";
import { listen } from "@tauri-apps/api/event";
import { LANGS, Lang, useI18n } from "./i18n";
import { PRESETS, Preset, monogram, presetById } from "./presets";
import * as Icon from "./Icons";

type View = { kind: "list" } | { kind: "pick" } | { kind: "form"; config: ProviderConfig; preset: Preset };

export default function Settings({
  providers,
  onClose,
  onChanged,
}: {
  providers: ProviderInfo[];
  onClose: () => void;
  /// Un fournisseur a été ajouté, modifié ou supprimé.
  onChanged: () => void;
}) {
  const { t, lang, setLang } = useI18n();
  const [view, setView] = useState<View>({ kind: "list" });
  const [configs, setConfigs] = useState<ProviderConfig[]>([]);

  const reload = () => getProviderConfigs().then(setConfigs);

  useEffect(() => {
    reload();
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  function startNew(preset: Preset) {
    setView({
      kind: "form",
      preset,
      config: {
        id: "",
        name: preset.id === "custom" ? "" : preset.name,
        kind: preset.kind,
        preset: preset.id,
        base_url: preset.url,
        has_key: false,
      },
    });
  }

  async function done() {
    await reload();
    onChanged();
    setView({ kind: "list" });
  }

  const clis = providers.filter((p) => p.preset === "cli");

  return (
    <div className="scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal glass" role="dialog" aria-modal="true" aria-label={t("settings")}>
        <header className="modal-head">
          {view.kind !== "list" && (
            <button className="icon-btn" onClick={() => setView(view.kind === "form" && !view.config.id ? { kind: "pick" } : { kind: "list" })} title={t("back")}>
              <span className="flip-rtl">
                <Icon.ChevronLeft />
              </span>
            </button>
          )}
          <h2>
            {view.kind === "list" ? t("settings") : view.kind === "pick" ? t("addProvider") : view.config.name || t("presetCustom")}
          </h2>
          <button className="icon-btn" onClick={onClose} title={t("close")}>
            <Icon.Close />
          </button>
        </header>

        {view.kind === "list" && (
          <div className="modal-body">
            <section>
              <h3>{t("language")}</h3>
              <div className="lang-grid">
                {LANGS.map((l) => (
                  <button key={l.id} className={`lang ${l.id === lang ? "on" : ""}`} onClick={() => setLang(l.id as Lang)}>
                    {l.label}
                  </button>
                ))}
              </div>
            </section>

            <section>
              <div className="section-title">
                <h3>{t("providers")}</h3>
                <button className="pill" onClick={() => setView({ kind: "pick" })}>
                  <Icon.Plus /> {t("addProvider")}
                </button>
              </div>
              {clis.map((p) => (
                <div key={p.id} className="provider-line">
                  <Badge name={p.name} color="#a48bff" />
                  <div className="provider-line-text">
                    <span>{p.name}</span>
                    <span className="muted small">{t("detected")}</span>
                  </div>
                </div>
              ))}
              {configs.map((c) => {
                const info = providers.find((p) => p.id === c.id);
                return (
                  <button
                    key={c.id}
                    className="provider-line clickable"
                    onClick={() => setView({ kind: "form", config: c, preset: presetById(c.preset) })}
                  >
                    <Badge name={c.name} color={presetById(c.preset).color} />
                    <div className="provider-line-text">
                      <span>{c.name}</span>
                      <span className="muted small">{c.base_url}</span>
                    </div>
                    <span className={`dot ${info?.available ? "ok" : ""}`} />
                  </button>
                );
              })}
              {configs.length === 0 && <div className="muted small pad">{t("noApiProviders")}</div>}
              <div className="muted small pad">{t("keychainNote")}</div>
            </section>

            <WebSettings />
          </div>
        )}

        {view.kind === "pick" && (
          <div className="modal-body">
            {(["local", "cloud"] as const).map((group) => (
              <section key={group}>
                <h3>{group === "local" ? t("groupLocal") : t("groupCloud")}</h3>
                <div className="preset-grid">
                  {PRESETS.filter((p) => p.group === group).map((p) => (
                    <button key={p.id} className="preset" onClick={() => startNew(p)}>
                      <Badge name={p.id === "custom" ? "+" : p.name} color={p.color} />
                      <div className="provider-line-text">
                        <span>{p.id === "custom" ? t("presetCustom") : p.name}</span>
                        {p.id === "custom" && <span className="muted small">{t("presetCustomDesc")}</span>}
                      </div>
                    </button>
                  ))}
                </div>
              </section>
            ))}
          </div>
        )}

        {view.kind === "form" && <ProviderForm key={view.config.id || view.preset.id} initial={view.config} preset={view.preset} onDone={done} />}
      </div>
    </div>
  );
}

function ProviderForm({ initial, preset, onDone }: { initial: ProviderConfig; preset: Preset; onDone: () => void }) {
  const { t } = useI18n();
  const [config, setConfig] = useState(initial);
  const [key, setKey] = useState("");
  const [removeKey, setRemoveKey] = useState(false);
  const [test, setTest] = useState<{ state: "idle" | "running" | "ok" | "error"; message?: string }>({ state: "idle" });
  const [saving, setSaving] = useState(false);

  const isCustom = preset.id === "custom";
  const keyRequired = preset.key === "required" && !config.has_key;
  const canSave = config.name.trim() !== "" && config.base_url.trim() !== "" && (!keyRequired || key.trim() !== "");
  /// Clé à envoyer : nouvelle clé, suppression, ou `undefined` pour garder l'actuelle.
  const keyArg = removeKey ? "" : key.trim() !== "" ? key.trim() : undefined;

  async function runTest() {
    setTest({ state: "running" });
    try {
      const models = await testProvider(config, removeKey ? undefined : keyArg);
      setTest({ state: "ok", message: t("formTestOk", { n: models.length }) });
    } catch (e) {
      setTest({ state: "error", message: String(e) });
    }
  }

  async function save() {
    setSaving(true);
    try {
      await saveProvider(config, keyArg);
      onDone();
    } catch (e) {
      setTest({ state: "error", message: String(e) });
      setSaving(false);
    }
  }

  async function remove() {
    if (await ask(t("deleteProviderConfirm", { name: config.name }), { title: "Magisterium", kind: "warning" })) {
      await deleteProvider(config.id);
      onDone();
    }
  }

  const set = (patch: Partial<ProviderConfig>) => {
    setConfig((c) => ({ ...c, ...patch }));
    setTest({ state: "idle" });
  };

  return (
    <div className="modal-body form">
      <label className="field">
        <span>{t("formName")}</span>
        <input value={config.name} onChange={(e) => set({ name: e.target.value })} autoFocus={isCustom} />
      </label>

      <label className="field">
        <span>{t("formUrl")}</span>
        <input value={config.base_url} onChange={(e) => set({ base_url: e.target.value })} spellCheck={false} dir="ltr" />
      </label>

      {isCustom && (
        <label className="field">
          <span>{t("formProtocol")}</span>
          <select value={config.kind} onChange={(e) => set({ kind: e.target.value as ApiKind })}>
            <option value="openai">{t("protocolOpenai")}</option>
            <option value="anthropic">{t("protocolAnthropic")}</option>
          </select>
        </label>
      )}

      {preset.key !== "none" && (
        <label className="field">
          <span>{preset.key === "required" ? t("formKey") : t("formKeyOptional")}</span>
          <input
            type="password"
            value={key}
            onChange={(e) => {
              setKey(e.target.value);
              setRemoveKey(false);
              setTest({ state: "idle" });
            }}
            placeholder={config.has_key ? "••••••••••••" : "sk-…"}
            autoFocus={!isCustom}
            spellCheck={false}
            dir="ltr"
          />
          {config.has_key && (
            <span className="field-help">
              {t("formKeySaved")}{" "}
              <button className="link" onClick={() => setRemoveKey((r) => !r)}>
                {removeKey ? "✓ " : ""}
                {t("formRemoveKey")}
              </button>
            </span>
          )}
        </label>
      )}

      {test.state !== "idle" && (
        <div className={`test-result ${test.state}`}>
          {test.state === "running" ? <span className="spin"><Icon.Refresh /></span> : null}
          {test.message}
        </div>
      )}

      <div className="form-actions">
        {config.id && (
          <button className="link danger" onClick={remove}>
            {t("formDelete")}
          </button>
        )}
        <span className="spacer" />
        <button className="pill" onClick={runTest} disabled={test.state === "running" || config.base_url.trim() === ""}>
          {t("formTest")}
        </button>
        <button className="pill primary" onClick={save} disabled={!canSave || saving}>
          {t("formSave")}
        </button>
      </div>
    </div>
  );
}

export function Badge({ name, color }: { name: string; color: string }) {
  return (
    <span className="badge-mono" style={{ ["--c" as string]: color }}>
      {monogram(name)}
    </span>
  );
}

function WebSettings() {
  const { t } = useI18n();
  const [web, setWeb] = useState<WebConfig | null>(null);
  const [key, setKey] = useState("");
  const [status, setStatus] = useState<{ state: "idle" | "running" | "ok" | "error"; message?: string }>({ state: "idle" });

  useEffect(() => {
    getWebConfig().then(setWeb);
  }, []);

  if (!web) return null;

  const set = (patch: Partial<WebConfig>) => {
    setWeb({ ...web, ...patch });
    setStatus({ state: "idle" });
  };
  const keyArg = key.trim() !== "" ? key.trim() : undefined;

  async function test() {
    setStatus({ state: "running" });
    try {
      const n = await testWebSearch(web!, keyArg);
      setStatus({ state: "ok", message: t("webTestOk", { n }) });
    } catch (e) {
      setStatus({ state: "error", message: String(e) });
    }
  }

  async function save() {
    try {
      setWeb(await saveWebConfig(web!, keyArg));
      setKey("");
      setStatus({ state: "ok", message: t("webSaved") });
    } catch (e) {
      setStatus({ state: "error", message: String(e) });
    }
  }

  const engines: { id: WebEngine; label: string }[] = [
    { id: "none", label: t("engineNone") },
    { id: "builtin", label: t("engineBuiltin") },
    { id: "tavily", label: "Tavily" },
    { id: "searxng", label: "SearXNG" },
  ];

  return (
    <section className="form">
      <h3>{t("webSection")}</h3>
      <div className="lang-grid engines">
        {engines.map((e) => (
          <button key={e.id} className={`lang ${web.engine === e.id ? "on" : ""}`} onClick={() => set({ engine: e.id })}>
            {e.label}
          </button>
        ))}
      </div>

      {web.engine === "builtin" && <BuiltinSearxng />}

      {web.engine === "tavily" && (
        <label className="field">
          <span>{t("formKey")}</span>
          <input
            type="password"
            value={key}
            onChange={(e) => {
              setKey(e.target.value);
              setStatus({ state: "idle" });
            }}
            placeholder={web.has_tavily_key ? "••••••••••••" : "tvly-…"}
            spellCheck={false}
            dir="ltr"
          />
          <span className="field-help">{web.has_tavily_key ? t("formKeySaved") : t("tavilyHelp")}</span>
        </label>
      )}

      {web.engine === "searxng" && (
        <label className="field">
          <span>{t("formUrl")}</span>
          <input
            value={web.searxng_url}
            onChange={(e) => set({ searxng_url: e.target.value })}
            placeholder="http://192.168.1.10:8080"
            spellCheck={false}
            dir="ltr"
          />
          <span className="field-help">{t("searxngHelp")}</span>
        </label>
      )}

      {status.state !== "idle" && (
        <div className={`test-result ${status.state}`}>
          {status.state === "running" ? (
            <span className="spin">
              <Icon.Refresh />
            </span>
          ) : null}
          {status.message}
        </div>
      )}

      <div className="form-actions">
        <span className="spacer" />
        {web.engine !== "none" && (
          <button className="pill" onClick={test} disabled={status.state === "running"}>
            {t("formTest")}
          </button>
        )}
        <button className="pill primary" onClick={save}>
          {t("formSave")}
        </button>
      </div>
    </section>
  );
}

const STEPS = ["uv", "python", "searxng", "deps", "config", "verify"] as const;

/// SearXNG intégré : installation à la demande (étapes + progression), puis
/// activation / désactivation.
function BuiltinSearxng() {
  const { t, lang } = useI18n();
  const [status, setStatus] = useState<SearxngStatus | null>(null);
  const [steps, setSteps] = useState<Record<number, InstallProgress>>({});
  const [speed, setSpeed] = useState<Record<number, number>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    searxngStatus().then(setStatus);
    // Vitesse lissée à partir des deux dernières mesures d'octets de chaque étape.
    const last: Record<number, { bytes: number; at: number }> = {};
    const offProgress = listen<InstallProgress>("searxng-progress", ({ payload: p }) => {
      setSteps((s) => ({ ...s, [p.index]: p }));
      if (p.bytes != null) {
        const now = performance.now();
        const prev = last[p.index];
        if (prev && now > prev.at) {
          const bps = ((p.bytes - prev.bytes) * 1000) / (now - prev.at);
          setSpeed((s) => ({ ...s, [p.index]: s[p.index] ? s[p.index] * 0.7 + bps * 0.3 : bps }));
        }
        last[p.index] = { bytes: p.bytes, at: now };
      }
    });
    const offStatus = listen<SearxngStatus>("searxng-status", ({ payload }) => setStatus(payload));
    return () => {
      offProgress.then((f) => f());
      offStatus.then((f) => f());
    };
  }, []);

  if (!status) return null;

  const mb = (n: number) => (n / 1_000_000).toLocaleString(lang, { maximumFractionDigits: 1 });

  async function install() {
    setBusy(true);
    setError(null);
    setSteps({});
    try {
      setStatus(await searxngInstall());
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function toggle(enabled: boolean) {
    setBusy(true);
    try {
      setStatus(await searxngSetEnabled(enabled));
    } finally {
      setBusy(false);
    }
  }

  async function uninstall() {
    if (!(await ask(t("builtinUninstallConfirm", { mb: mb(status!.size_bytes) }), { title: "Magisterium", kind: "warning" }))) return;
    setBusy(true);
    try {
      setStatus(await searxngUninstall());
      setSteps({});
    } finally {
      setBusy(false);
    }
  }

  const showSteps = status.installing || Object.keys(steps).length > 0;

  return (
    <div className="builtin">
      {!status.installed && !status.installing && (
        <>
          <p className="field-help">{t("builtinIntro")}</p>
          {!showSteps && (
            <button className="pill primary" onClick={install} disabled={busy}>
              {t("builtinInstall")}
            </button>
          )}
        </>
      )}

      {showSteps && (
        <ol className="install-steps">
          {STEPS.map((id, i) => {
            const p = steps[i];
            const state = p?.state ?? "pending";
            const running = state === "running";
            const ratio = p?.bytes != null && p.total_bytes ? Math.min(1, p.bytes / p.total_bytes) : null;
            return (
              <li key={id} className={`install-step ${state}`}>
                <span className="step-icon" aria-hidden>
                  {state === "done" || state === "skipped" ? "✓" : state === "error" ? "✕" : running ? "" : "○"}
                  {running && <span className="spin"><Icon.Refresh /></span>}
                </span>
                <div className="step-body">
                  <div className="step-title">
                    <span>{t(`step_${id}` as const)}</span>
                    {state === "skipped" && <span className="muted small">{t("stepSkipped")}</span>}
                  </div>
                  {running && (
                    <>
                      <div className={`progress ${ratio == null ? "indeterminate" : ""}`}>
                        <div className="progress-fill" style={ratio == null ? undefined : { transform: `scaleX(${ratio})` }} />
                      </div>
                      <div className="muted small step-meta" dir="ltr">
                        {p?.bytes != null
                          ? `${mb(p.bytes)}${p.total_bytes ? ` / ${mb(p.total_bytes)}` : ""} ${t("mb")}${speed[i] ? ` · ${mb(speed[i])} ${t("mb")}/s` : ""}`
                          : p?.message ?? ""}
                      </div>
                    </>
                  )}
                  {state === "error" && p?.message && <div className="error small">{p.message}</div>}
                </div>
              </li>
            );
          })}
        </ol>
      )}

      {!status.installing && error && (
        <button className="pill primary" onClick={install} disabled={busy}>
          {t("builtinRetry")}
        </button>
      )}

      {status.installed && !status.installing && (
        <div className="builtin-installed">
          <div className="row">
            <span>
              {status.starting
                ? t("builtinStarting")
                : status.port
                  ? t("builtinReady", { port: status.port })
                  : status.enabled
                    ? t("builtinStopped")
                    : t("builtinDisabled")}
            </span>
            <Switch checked={status.enabled} onChange={toggle} disabled={busy || status.starting} />
          </div>
          {status.error && <div className="error small">{status.error}</div>}
          <div className="form-actions">
            <span className="muted small">{t("builtinSize", { mb: mb(status.size_bytes) })}</span>
            <span className="spacer" />
            <button className="link danger" onClick={uninstall} disabled={busy}>
              {t("builtinUninstall")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function Switch({ checked, onChange, disabled }: { checked: boolean; onChange: (v: boolean) => void; disabled?: boolean }) {
  return (
    <button role="switch" aria-checked={checked} className={`switch ${checked ? "on" : ""}`} onClick={() => onChange(!checked)} disabled={disabled}>
      <span />
    </button>
  );
}
