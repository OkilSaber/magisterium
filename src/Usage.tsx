import { useEffect } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ProviderUsage, UsageMeter } from "./api";
import { T, TKey, useI18n } from "./i18n";
import { presetById } from "./presets";
import { Badge } from "./Settings";
import * as Icon from "./Icons";

/// Part la plus consommée d'un fournisseur : c'est elle qui bloquera en premier.
export function peakUsage(usage?: ProviderUsage): number | null {
  const values = (usage?.meters ?? []).map((m) => m.used).filter((u): u is number => u != null);
  return values.length ? Math.max(...values) : null;
}

export function level(used: number) {
  return used >= 0.85 ? "high" : used >= 0.6 ? "mid" : "low";
}

const WINDOW_KEYS: Record<string, TKey> = {
  session: "usageSession",
  "5h": "usageFiveHour",
  five_hour: "usageFiveHour",
  week: "usageWeek",
  weekly: "usageWeekly",
  balance: "usageBalance",
  key_limit: "usageKeyLimit",
  free_daily: "usageFreeDaily",
  credits: "usageCredits",
};

function meterTitle(t: T, m: UsageMeter) {
  const window = WINDOW_KEYS[m.window] ? t(WINDOW_KEYS[m.window]) : m.window;
  if (!m.group) return window;
  // « Gemini Models · Weekly limit », mais « This week (all models) ».
  return m.group[0] === m.group[0].toLowerCase() ? `${window} (${m.group})` : `${m.group} · ${window}`;
}

function formatAmount(lang: string, value: number, unit: string | null) {
  if (unit && /^[A-Z]{3}$/.test(unit)) {
    return new Intl.NumberFormat(lang, { style: "currency", currency: unit, maximumFractionDigits: 2 }).format(value);
  }
  return value.toLocaleString(lang, { maximumFractionDigits: 0 });
}

/// « dans 2 heures », « in 3 days »… à partir d'une date ISO.
function relative(lang: string, iso: string) {
  const diff = (new Date(iso).getTime() - Date.now()) / 60_000;
  if (!Number.isFinite(diff)) return iso;
  const rtf = new Intl.RelativeTimeFormat(lang, { numeric: "auto" });
  if (Math.abs(diff) < 60) return rtf.format(Math.round(diff), "minute");
  if (Math.abs(diff) < 48 * 60) return rtf.format(Math.round(diff / 60), "hour");
  return rtf.format(Math.round(diff / 1440), "day");
}

function Meter({ m }: { m: UsageMeter }) {
  const { t, lang } = useI18n();
  const hasBar = m.used != null;
  const unitLabel = m.unit === "credits" ? t("usageUnitCredits") : m.unit === "requests" ? t("usageUnitRequests") : "";
  const figures =
    m.amount != null
      ? m.limit != null && m.window !== "balance"
        ? `${formatAmount(lang, m.amount, m.unit)} / ${formatAmount(lang, m.limit, m.unit)} ${unitLabel}`
        : `${formatAmount(lang, m.amount, m.unit)} ${unitLabel}`
      : null;
  return (
    <div className="meter">
      <div className="meter-head">
        <span>{meterTitle(t, m)}</span>
        {hasBar && <span className={`meter-pct ${level(m.used!)}`}>{t("usageUsed", { pct: Math.round(m.used! * 100) })}</span>}
      </div>
      {hasBar && (
        <div className="meter-track">
          <div className={`meter-fill ${level(m.used!)}`} style={{ transform: `scaleX(${Math.max(0.01, m.used!)})` }} />
        </div>
      )}
      <div className="muted small meter-foot">
        {figures && <span dir="ltr">{figures.trim()}</span>}
        {m.resets_at && <span>{t("usageResets", { when: relative(lang, m.resets_at) })}</span>}
        {m.resets_text && <span>{t("usageResets", { when: m.resets_text })}</span>}
      </div>
    </div>
  );
}

export default function UsagePanel({
  usage,
  loading,
  updatedAt,
  onRefresh,
  onClose,
}: {
  usage: ProviderUsage[];
  loading: boolean;
  updatedAt: number | null;
  onRefresh: () => void;
  onClose: () => void;
}) {
  const { t, lang } = useI18n();

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal glass" role="dialog" aria-modal="true" aria-label={t("usageTitle")}>
        <header className="modal-head">
          <h2>{t("usageTitle")}</h2>
          <button className="icon-btn" onClick={onRefresh} disabled={loading} title={t("refreshAll")}>
            <span className={loading ? "spin" : ""}>
              <Icon.Refresh />
            </span>
          </button>
          <button className="icon-btn" onClick={onClose} title={t("close")}>
            <Icon.Close />
          </button>
        </header>
        <div className="modal-body">
          {usage.map((u) => (
            <section key={u.id} className="usage-provider">
              <div className="usage-provider-head">
                <Badge name={u.name} color={u.preset === "cli" ? "#a48bff" : presetById(u.preset).color} />
                <span className="provider-name">{u.name}</span>
                {u.status === "unlimited" && <span className="usage-tag">{t("usageUnlimited")}</span>}
              </div>
              {u.meters.map((m, i) => (
                <Meter key={i} m={m} />
              ))}
              {u.status === "unavailable" && <div className="muted small">{t("usageUnavailable")}</div>}
              {u.status === "error" && <div className="error small">{t("usageError", { e: u.message ?? "" })}</div>}
              {u.dashboard && u.status !== "unlimited" && (
                <button className="link usage-link" onClick={() => openUrl(u.dashboard!)}>
                  {t("usageDashboard")} <Icon.External />
                </button>
              )}
            </section>
          ))}
          {!loading && usage.length === 0 && <div className="muted small">{t("usageNone")}</div>}
          {loading && usage.length === 0 && <div className="thinking">{t("usageLoading")}</div>}
          {updatedAt && (
            <div className="muted small pad">
              {t("usageUpdated", { when: new Date(updatedAt).toLocaleTimeString(lang, { hour: "2-digit", minute: "2-digit" }) })}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
