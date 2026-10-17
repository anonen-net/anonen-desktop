import React, { useEffect, useState, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import { Mail, Loader2, Check, History } from "lucide-react";
import { commands, type AnonenCloudUsageSnapshot } from "@/bindings";
import { isActiveSubscriptionStatus } from "@/stores/anonenCloudStore";
import {
  NO_ACTIVE_SUBSCRIPTION,
  SIGNED_OUT_CHECK_MS,
  quietRecheckVerdict,
  shouldRecheckOnReturn,
  subscriptionWallKey,
  takeSessionEndedNote,
  withinMs,
  type QuietLookupAnswer,
} from "@/lib/failureNotice";
import { Input } from "@/components/ui/Input";
import { Button } from "@/components/ui/Button";
import { DialogFrame } from "@/components/ui/ConfirmDialog";
import TurnstileChallenge from "./TurnstileChallenge";
import { SupportActions } from "@/components/settings/SupportActions";

const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

function isRateLimited(err: unknown): boolean {
  const msg = err instanceof Error ? err.message : String(err);
  return msg.includes("429") || msg.includes("rate_limit");
}

interface AuthOnboardingProps {
  onComplete: () => void;
  onViewHistory?: () => void;
}

function hasValidSubscription(usage: AnonenCloudUsageSnapshot | null): boolean {
  return isActiveSubscriptionStatus(usage?.subscription?.status ?? null);
}

type Phase = "sign_in" | "checking" | "no_subscription" | "done";

async function signedOut(): Promise<boolean> {
  const status = await withinMs(
    commands.anonenCloudAuthStatus(),
    SIGNED_OUT_CHECK_MS,
    null,
  );
  return status !== null && status.status === "ok" && !status.data.signed_in;
}

async function quietLookup(): Promise<{
  answer: QuietLookupAnswer;
  signedOut: boolean;
}> {
  const fresh = await commands.anonenCloudFetchUsage();
  if (fresh.status === "ok") {
    return {
      answer: { ok: true, active: hasValidSubscription(fresh.data) },
      signedOut: false,
    };
  }
  const gone = fresh.error !== NO_ACTIVE_SUBSCRIPTION && (await signedOut());
  return { answer: { ok: false, error: fresh.error }, signedOut: gone };
}

const AuthOnboarding: React.FC<AuthOnboardingProps> = ({
  onComplete,
  onViewHistory,
}) => {
  const { t } = useTranslation();
  const [phase, setPhase] = useState<Phase>("sign_in");
  const [email, setEmail] = useState("");
  const [token, setToken] = useState("");
  const [otpSent, setOtpSent] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [cooldownSecs, setCooldownSecs] = useState(0);
  const [captchaOpen, setCaptchaOpen] = useState(false);

  const requestingOtp = useRef(false);

  const pendingEmail = useRef("");

  const [wallKey, setWallKey] = useState(subscriptionWallKey(null));

  const setupAndProceed = useCallback(async () => {
    setPhase("done");
    try {
      const { useAnonenCloudStore } = await import("@/stores/anonenCloudStore");
      await useAnonenCloudStore.getState().fetchCloudModels();
    } catch {}
    try {
      const { useModelStore } = await import("@/stores/modelStore");

      await useModelStore.getState().loadCurrentModel();
      const store = useModelStore.getState();

      const currentIsValid =
        store.currentModel !== "" &&
        store.models.some((m) => m.id === store.currentModel);
      if (!currentIsValid) {
        if (store.currentModel.startsWith("anonen-cloud:")) {
        } else {
        }
      }
    } catch {}
    onComplete();
  }, [onComplete]);

  const returnToSignIn = useCallback(() => {
    setOtpSent(false);
    setToken("");
    setCooldownSecs(0);
    setError(t("anonenCloud.errors.sessionEnded"));
    setPhase("sign_in");
  }, [t]);

  const checkSubscription = useCallback(async () => {
    setPhase("checking");
    try {
      const cached = await commands.anonenCloudCurrentUsage();
      if (cached.status === "ok" && cached.data) {
        if (hasValidSubscription(cached.data)) {
          await setupAndProceed();
          return;
        }
      }
      const fresh = await commands.anonenCloudFetchUsage();
      if (fresh.status === "ok") {
        if (hasValidSubscription(fresh.data)) {
          await setupAndProceed();
          return;
        }

        setWallKey(subscriptionWallKey(null));
        setPhase("no_subscription");
        return;
      }

      if (fresh.error !== NO_ACTIVE_SUBSCRIPTION && (await signedOut())) {
        returnToSignIn();
        return;
      }

      const persisted = await commands.anonenCloudCachedSubscriptionStatus();
      if (
        persisted.status === "ok" &&
        isActiveSubscriptionStatus(persisted.data)
      ) {
        await setupAndProceed();
        return;
      }

      setWallKey(subscriptionWallKey(fresh.error));
      setPhase("no_subscription");
    } catch (e) {
      setWallKey(subscriptionWallKey(String(e)));
      setPhase("no_subscription");
    }
  }, [setupAndProceed, returnToSignIn]);

  useEffect(() => {
    const checkExisting = async () => {
      const sessionEnded = takeSessionEndedNote();
      try {
        const auth = await commands.anonenCloudAuthStatus();
        if (auth.status === "ok" && auth.data.signed_in) {
          await checkSubscription();
          return;
        }
      } catch {}

      if (sessionEnded) {
        setError(t("anonenCloud.errors.sessionEnded"));
      }
    };
    checkExisting();
  }, []);

  const latest = useRef({ phase, busy, setupAndProceed, returnToSignIn });
  useEffect(() => {
    latest.current = { phase, busy, setupAndProceed, returnToSignIn };
  });
  const recheck = useRef({
    inFlight: false,
    lastCheckAt: null as number | null,
  });
  useEffect(() => {
    if (phase === "no_subscription") recheck.current.lastCheckAt = Date.now();
  }, [phase]);
  useEffect(() => {
    let unmounted = false;
    const onReturn = async () => {
      const now = Date.now();
      const state = recheck.current;
      if (
        !shouldRecheckOnReturn({
          phase: latest.current.phase,
          busy: latest.current.busy,
          inFlight: state.inFlight,
          lastCheckAt: state.lastCheckAt,
          now,
        })
      ) {
        return;
      }
      state.inFlight = true;
      state.lastCheckAt = now;
      try {
        const { answer, signedOut: gone } = await quietLookup();
        const verdict = quietRecheckVerdict(answer, gone);

        if (unmounted || latest.current.phase !== "no_subscription") return;
        if (verdict === "proceed") await latest.current.setupAndProceed();
        else if (verdict === "signIn") latest.current.returnToSignIn();
      } catch {
      } finally {
        state.inFlight = false;
      }
    };
    const onVisible = () => {
      if (document.visibilityState === "visible") void onReturn();
    };
    window.addEventListener("focus", onReturn);
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      unmounted = true;
      window.removeEventListener("focus", onReturn);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, []);

  useEffect(() => {
    if (cooldownSecs <= 0) return;
    const id = setTimeout(() => setCooldownSecs((s) => s - 1), 1000);
    return () => clearTimeout(id);
  }, [cooldownSecs]);

  useEffect(() => {
    if (phase !== "sign_in") setCaptchaOpen(false);
  }, [phase]);

  const handleRequestOtp = () => {
    const trimmed = email.trim();
    if (!trimmed) {
      setError(t("anonenCloud.errors.emailRequired"));
      return;
    }
    if (!EMAIL_PATTERN.test(trimmed)) {
      setError(t("anonenCloud.errors.emailInvalid"));
      return;
    }
    setError(null);
    pendingEmail.current = trimmed;
    setCaptchaOpen(true);
  };

  const handleCaptchaToken = (captchaToken: string | null) => {
    if (!captchaToken || requestingOtp.current) return;
    const to = pendingEmail.current;
    setCaptchaOpen(false);
    if (!EMAIL_PATTERN.test(to)) return;
    requestingOtp.current = true;
    void sendOtp(captchaToken, to);
  };

  const sendOtp = async (captchaToken: string, to: string) => {
    setBusy(true);
    setError(null);
    try {
      const result = await commands.anonenCloudRequestOtp(to, captchaToken);
      if (result.status !== "ok") throw new Error(result.error);
      setOtpSent(true);
      setCooldownSecs(60);
    } catch (e) {
      if (isRateLimited(e)) {
        setError(t("anonenCloud.errors.rateLimited"));
        setCooldownSecs(60);
      } else {
        console.error("OTP request failed:", e);
        setError(t("anonenCloud.errors.otpRequestFailed"));
      }
    } finally {
      setBusy(false);
      requestingOtp.current = false;
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
      const result = await commands.anonenCloudVerifyOtp(
        email.trim(),
        token.trim(),
      );
      if (result.status !== "ok") throw new Error(result.error);
      await checkSubscription();
    } catch (e) {
      if (isRateLimited(e)) {
        setError(t("anonenCloud.errors.rateLimited"));
      } else {
        console.error("OTP verify failed:", e);
        setError(t("anonenCloud.errors.verifyFailed"));
      }
    } finally {
      setBusy(false);
    }
  };

  if (phase === "checking" || phase === "done") {
    return (
      <div className="h-screen w-screen flex flex-col items-center justify-center gap-4">
        <Loader2 className="w-8 h-8 animate-spin text-logo-primary" />
        <p className="text-sm text-muted">
          {t("authOnboarding.checkingSubscription")}
        </p>
      </div>
    );
  }

  if (phase === "no_subscription") {
    return (
      <div className="jp-phrase-wrap min-h-screen w-screen flex flex-col p-6 gap-4 items-center justify-center overflow-y-auto">
        <h1 className="text-2xl font-bold">{t("app.name")}</h1>
        <div className="max-w-sm w-full flex flex-col items-center gap-3 text-center">
          <div className="p-3 rounded-full bg-amber-500/20">
            <Check className="w-8 h-8 text-amber-400" />
          </div>
          <p className="text-sm text-text/70 text-balance">{t(wallKey)}</p>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => checkSubscription()}
            disabled={busy}
          >
            {t("authOnboarding.retrySubscription")}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              setPhase("sign_in");
              setOtpSent(false);
              setToken("");
              setEmail("");
            }}
          >
            {t("authOnboarding.switchAccount")}
          </Button>
          {onViewHistory && (
            <button
              onClick={onViewHistory}
              className="inline-flex items-center gap-1.5 text-xs text-text/50 hover:text-text/70 transition-colors mt-1"
            >
              <History className="w-3.5 h-3.5" />
              {t("authOnboarding.viewHistory")}
            </button>
          )}

          <WallSupport />
        </div>
      </div>
    );
  }

  return (
    <div className="jp-phrase-wrap min-h-screen w-screen flex flex-col p-6 gap-4 items-center justify-center overflow-y-auto">
      <h1 className="text-2xl font-bold">{t("app.name")}</h1>

      {captchaOpen && (
        <DialogFrame
          labelledBy="sign-in-captcha-title"
          onEscape={() => setCaptchaOpen(false)}
        >
          <h2 id="sign-in-captcha-title" className="text-base font-semibold">
            {t("anonenCloud.captchaDialog.title")}
          </h2>
          <p className="text-sm text-text/70">
            {t("anonenCloud.captchaDialog.body")}
          </p>
          <TurnstileChallenge onTokenChange={handleCaptchaToken} resetKey={0} />
          <div className="flex justify-end">
            <button
              type="button"
              className="px-3 py-1.5 text-sm rounded border border-mid-gray/20 hover:bg-mid-gray/10 transition-colors"
              onClick={() => setCaptchaOpen(false)}
            >
              {t("common.close")}
            </button>
          </div>
        </DialogFrame>
      )}

      <div className="max-w-sm w-full flex flex-col items-center gap-3">
        <div className="text-center">
          <h2 className="text-base font-semibold text-text mb-1">
            {t("authOnboarding.title")}
          </h2>
          <p className="text-text/70 text-xs text-balance">
            {t("anonenCloud.signInPrompt")}
          </p>
        </div>

        <div className="w-full space-y-3">
          <div className="flex gap-2">
            <div className="flex-1 flex items-center gap-2">
              <Mail className="w-4 h-4 text-muted shrink-0" />
              <Input
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !otpSent) handleRequestOtp();
                }}
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
            <div className="space-y-2">
              <div className="flex gap-2">
                <div className="flex-1">
                  <Input
                    type="text"
                    value={token}
                    onChange={(e) => setToken(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleVerifyOtp();
                    }}
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
              </div>
              <div className="flex gap-2 justify-center">
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => handleRequestOtp()}
                  disabled={busy || cooldownSecs > 0}
                >
                  {cooldownSecs > 0
                    ? t("anonenCloud.resendCooldown", {
                        seconds: cooldownSecs,
                      })
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
            </div>
          )}
        </div>

        {error && (
          <div className="w-full rounded-md bg-red-500/10 border border-red-500/30 px-3 py-2 text-xs text-red-400">
            {error}
          </div>
        )}

        <p className="text-xs text-text/50 text-center leading-relaxed text-balance">
          {t("authOnboarding.noAccountGuide")}
        </p>

        {onViewHistory && (
          <button
            onClick={onViewHistory}
            className="inline-flex items-center gap-1.5 text-xs text-text/50 hover:text-text/70 transition-colors"
          >
            <History className="w-3.5 h-3.5" />
            {t("authOnboarding.viewHistory")}
          </button>
        )}

        <WallSupport />
      </div>
    </div>
  );
};

const WallSupport: React.FC = () => {
  const { t } = useTranslation();
  return (
    <div className="w-full mt-2 pt-3 border-t border-border flex flex-col items-center gap-2">
      <p className="text-xs text-text/50 text-center">
        {t("settings.support.title")}
      </p>
      <SupportActions variant="ghost" size="sm" className="justify-center" />
    </div>
  );
};

export default AuthOnboarding;
