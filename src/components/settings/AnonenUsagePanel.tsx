import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { RefreshCw } from "lucide-react";
import { useAnonenCloudStore } from "@/stores/anonenCloudStore";
import type { AsrGatewayUsage } from "@/bindings";
import {
  formatDuration,
  formatResetDateTime as formatResetDateTimeIn,
  isTrialCapped,
  weekCapFullS,
} from "@/lib/cloudUsage";

const WeekBar: React.FC<{ usage: AsrGatewayUsage }> = ({ usage }) => {
  const { t } = useTranslation();
  const full = Math.max(1, weekCapFullS(usage));
  const trial = isTrialCapped(usage);
  const usablePct = Math.min(100, (usage.week_cap_s / full) * 100);
  const usedPct = Math.min(usablePct, (usage.week_used_s / full) * 100);
  return (
    <>
      <div className="relative w-full h-1.5 bg-mid-gray/10 rounded overflow-hidden">
        {trial && (
          <div
            className="absolute inset-y-0 right-0"
            style={{
              width: `${100 - usablePct}%`,
              backgroundImage:
                "repeating-linear-gradient(135deg, transparent 0 3px, rgba(128,128,128,0.45) 3px 4px)",
            }}
          />
        )}
        <div
          className="h-1.5 bg-logo-primary rounded"
          style={{ width: `${usedPct}%` }}
        />
        {trial && (
          <div
            className="absolute inset-y-0 w-px bg-mid-gray/70"
            style={{ left: `${usablePct}%` }}
          />
        )}
      </div>
      {trial && (
        <div className="text-muted">
          {t("anonenCloud.trialCapNote", {
            share:
              Math.abs(usablePct - 50) < 1
                ? t("anonenCloud.trialShareHalf")
                : t("anonenCloud.trialSharePct", {
                    pct: Math.round(usablePct),
                  }),
          })}
        </div>
      )}
    </>
  );
};

export const AnonenUsagePanel: React.FC = () => {
  const { t, i18n } = useTranslation();
  const { authStatus, usage, refreshUsage } = useAnonenCloudStore();
  const [busy, setBusy] = useState(false);

  if (!authStatus.signed_in) return null;

  const formatResetDateTime = (iso: string) =>
    formatResetDateTimeIn(iso, i18n.language);

  const handleRefresh = async () => {
    setBusy(true);
    try {
      await refreshUsage();
    } catch (e) {
      console.error("Usage refresh failed:", e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="rounded-md border-2 border-border bg-surface px-4 py-3 space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-xs text-muted">
          {t("anonenCloud.usageSection")}
        </span>
        <button
          type="button"
          onClick={handleRefresh}
          disabled={busy}
          className="text-xs text-muted hover:text-text inline-flex items-center gap-1 disabled:opacity-50"
        >
          <RefreshCw className={`w-3 h-3 ${busy ? "animate-spin" : ""}`} />
          {t("anonenCloud.refresh")}
        </button>
      </div>

      {usage ? (
        <div className="text-xs space-y-1">
          <div className="flex justify-between">
            <span className="text-muted">{t("anonenCloud.weekly")}</span>
            <span>
              {t("anonenCloud.remainingPercent", {
                percent: Math.max(
                  0,
                  Math.round(
                    (1 -
                      usage.usage.week_used_s /
                        Math.max(1, usage.usage.week_cap_s)) *
                      100,
                  ),
                ),
              })}
            </span>
          </div>
          <WeekBar usage={usage.usage} />

          <div className="text-muted">
            {t("anonenCloud.weekResets", {
              when: formatResetDateTime(usage.usage.week_resets_at),
            })}
          </div>

          {usage.usage.month_cap_s - usage.usage.month_used_s <
            usage.usage.week_cap_s - usage.usage.week_used_s && (
            <div className="text-yellow-400">
              {t("anonenCloud.longWindowBinds", {
                remaining: formatDuration(
                  Math.max(
                    0,
                    usage.usage.month_cap_s - usage.usage.month_used_s,
                  ),
                ),
              })}
            </div>
          )}
        </div>
      ) : (
        <p className="text-xs text-muted">{t("anonenCloud.noUsageYet")}</p>
      )}
    </div>
  );
};
