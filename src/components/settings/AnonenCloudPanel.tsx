import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { AlertTriangle, Cloud, Mail, LogOut, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { useAnonenCloudStore } from "@/stores/anonenCloudStore";
import {
  formatDuration,
  formatResetDate as formatResetDateIn,
  formatResetDateTime as formatResetDateTimeIn,
} from "@/lib/cloudUsage";

export const AnonenCloudPanel: React.FC = () => {
  const { t, i18n } = useTranslation();
  const { authStatus, usage, requestOtp, verifyOtp, logout, refreshUsage } =
    useAnonenCloudStore();

  const [email, setEmail] = useState("");
  const [token, setToken] = useState("");
  const [otpSent, setOtpSent] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [cooldownSecs, setCooldownSecs] = useState(0);

  useEffect(() => {
    if (!authStatus.signed_in) {
      setOtpSent(false);
      setToken("");
    }
  }, [authStatus.signed_in]);

  useEffect(() => {
    if (cooldownSecs <= 0) return;
    const id = setTimeout(() => setCooldownSecs((s) => s - 1), 1000);
    return () => clearTimeout(id);
  }, [cooldownSecs]);

  const handleRequestOtp = async () => {
    const trimmed = email.trim();
    if (!trimmed) {
      setError(t("anonenCloud.errors.emailRequired"));
      return;
    }
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(trimmed)) {
      setError(t("anonenCloud.errors.emailInvalid"));
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await requestOtp(trimmed);
      setOtpSent(true);
      setCooldownSecs(60);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (msg.includes("429") || msg.includes("rate_limit")) {
        setError(t("anonenCloud.errors.rateLimited"));
        setCooldownSecs(60);
      } else {
        console.error("OTP request failed:", e);
        setError(t("anonenCloud.errors.otpRequestFailed"));
      }
    } finally {
      setBusy(false);
    }
  };

  const handleVerifyOtp = async () => {
    if (!token.trim()) {
      setError(t("anonenCloud.errors.tokenRequired"));
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await verifyOtp(email.trim(), token.trim());
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (msg.includes("429") || msg.includes("rate_limit")) {
        setError(t("anonenCloud.errors.rateLimited"));
      } else {
        console.error("OTP verify failed:", e);
        setError(t("anonenCloud.errors.verifyFailed"));
      }
    } finally {
      setBusy(false);
    }
  };

  const handleLogout = async () => {
    setBusy(true);
    try {
      await logout();
      setEmail("");
      setToken("");
      setOtpSent(false);
    } finally {
      setBusy(false);
    }
  };

  const handleRefreshUsage = async () => {
    setBusy(true);
    setError(null);
    try {
      await refreshUsage();
    } catch (e) {
      console.error("Usage refresh failed:", e);
      setError(t("anonenCloud.errors.usageRefreshFailed"));
    } finally {
      setBusy(false);
    }
  };

  const formatResetDateTime = (iso: string) =>
    formatResetDateTimeIn(iso, i18n.language);
  const formatResetDate = (iso: string) =>
    formatResetDateIn(iso, i18n.language);

  return (
    <div className="rounded-md border-2 border-border bg-surface px-4 py-3 space-y-3">
      <div className="flex items-center gap-2">
        <Cloud className="w-4 h-4 text-logo-primary" />
        <span className="text-sm font-medium">{t("anonenCloud.title")}</span>
      </div>

      {!authStatus.signed_in ? (
        <div className="space-y-2">
          <p className="text-xs text-muted">{t("anonenCloud.signInPrompt")}</p>
          <div className="flex gap-2">
            <div className="flex-1 flex items-center gap-2">
              <Mail className="w-4 h-4 text-muted shrink-0" />
              <Input
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder={t("anonenCloud.emailPlaceholder")}
                variant="compact"
                disabled={busy || otpSent}
              />
            </div>
            {!otpSent && (
              <Button
                size="sm"
                variant="primary"
                onClick={handleRequestOtp}
                disabled={busy}
              >
                {t("anonenCloud.sendOtp")}
              </Button>
            )}
          </div>

          {otpSent && (
            <div className="flex gap-2">
              <div className="flex-1">
                <Input
                  type="text"
                  value={token}
                  onChange={(e) => setToken(e.target.value)}
                  placeholder={t("anonenCloud.otpPlaceholder")}
                  variant="compact"
                  disabled={busy}
                  inputMode="numeric"
                  autoFocus
                />
              </div>
              <Button
                size="sm"
                variant="primary"
                onClick={handleVerifyOtp}
                disabled={busy}
              >
                {t("anonenCloud.verify")}
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => {
                  handleRequestOtp();
                }}
                disabled={busy || cooldownSecs > 0}
              >
                {cooldownSecs > 0
                  ? t("anonenCloud.resendCooldown", { seconds: cooldownSecs })
                  : t("anonenCloud.resendOtp")}
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => {
                  setOtpSent(false);
                  setToken("");
                }}
                disabled={busy}
              >
                {t("anonenCloud.back")}
              </Button>
            </div>
          )}
        </div>
      ) : (
        <div className="space-y-2">
          <div className="flex items-center justify-between gap-2 text-sm">
            <span className="text-text/70 truncate min-w-0">
              {authStatus.email
                ? t("anonenCloud.signedInAs", { email: authStatus.email })
                : t("anonenCloud.signedIn")}
            </span>
            <Button
              size="sm"
              variant="ghost"
              onClick={handleLogout}
              disabled={busy}
              className="inline-flex items-center whitespace-nowrap shrink-0"
            >
              <LogOut className="w-3.5 h-3.5 mr-1 shrink-0" />
              {t("anonenCloud.logout")}
            </Button>
          </div>

          {usage && (
            <div className="text-xs space-y-1">
              {usage.subscription && (
                <div className="text-muted">
                  {t("anonenCloud.subscriptionStatus", {
                    status: t(
                      `anonenCloud.subscriptionStatuses.${usage.subscription.status}`,
                      { defaultValue: usage.subscription.status },
                    ),
                  })}
                </div>
              )}

              {usage.subscription?.status === "past_due" && (
                <div className="flex items-start gap-2 rounded-md bg-yellow-500/10 border border-yellow-500/30 px-2 py-1.5">
                  <AlertTriangle className="w-3.5 h-3.5 shrink-0 mt-0.5 text-yellow-500" />
                  <span className="text-yellow-400">
                    {t("anonenCloud.pastDueWarning")}
                  </span>
                </div>
              )}
            </div>
          )}
        </div>
      )}

      {error && (
        <div className="rounded-md bg-red-500/10 border border-red-500/30 px-2 py-1 text-xs text-red-400">
          {error}
        </div>
      )}
    </div>
  );
};
