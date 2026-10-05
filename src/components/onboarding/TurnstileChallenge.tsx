import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

const CAPTCHA_ORIGIN = "https://anonen.net";

const captchaUrl = (): string => {
  const theme =
    document.documentElement.dataset.theme === "dark" ? "dark" : "light";
  return `${CAPTCHA_ORIGIN}/captcha/?embed=1&theme=${theme}`;
};

const READY_TYPE = "anonen-captcha-ready";
const READY_ACK_TYPE = "anonen-captcha-ready-ack";
const TOKEN_TYPE = "anonen-captcha-token";

const READY_RETRY_MS = 500;

const HANDSHAKE_TIMEOUT_MS = 12000;

interface TurnstileChallengeProps {
  onTokenChange: (token: string | null) => void;
  resetKey: number;
}

const TurnstileChallenge: React.FC<TurnstileChallengeProps> = ({
  onTokenChange,
  resetKey,
}) => {
  const { t } = useTranslation();
  const frameRef = useRef<HTMLIFrameElement>(null);
  const onTokenChangeRef = useRef(onTokenChange);
  const [loadFailed, setLoadFailed] = useState(false);
  const [retryKey, setRetryKey] = useState(0);

  useEffect(() => {
    onTokenChangeRef.current = onTokenChange;
  }, [onTokenChange]);

  const frameKey = `${resetKey}:${retryKey}`;

  useEffect(() => {
    const frame = frameRef.current;
    if (!frame) return;

    let disposed = false;
    let acked = false;
    setLoadFailed(false);
    onTokenChangeRef.current(null);

    const sendReady = () => {
      frame.contentWindow?.postMessage(
        { type: READY_TYPE, version: 1 },
        CAPTCHA_ORIGIN,
      );
    };

    const handleMessage = (event: MessageEvent) => {
      if (disposed) return;
      if (event.origin !== CAPTCHA_ORIGIN) return;
      if (event.source !== frame.contentWindow) return;
      const data = event.data;
      if (typeof data !== "object" || data === null) return;

      if (data.type === READY_ACK_TYPE) {
        acked = true;
        setLoadFailed(false);
        return;
      }
      if (data.type !== TOKEN_TYPE) return;
      const token =
        typeof data.token === "string" && data.token.length > 0
          ? data.token
          : null;
      onTokenChangeRef.current(token);
    };

    window.addEventListener("message", handleMessage);
    frame.addEventListener("load", sendReady);
    sendReady();

    const retry = window.setInterval(() => {
      if (acked) {
        window.clearInterval(retry);
        return;
      }
      sendReady();
    }, READY_RETRY_MS);

    const timeout = window.setTimeout(() => {
      if (disposed || acked) return;
      window.clearInterval(retry);
      setLoadFailed(true);
      onTokenChangeRef.current(null);
    }, HANDSHAKE_TIMEOUT_MS);

    return () => {
      disposed = true;
      window.clearInterval(retry);
      window.clearTimeout(timeout);
      window.removeEventListener("message", handleMessage);
      frame.removeEventListener("load", sendReady);
      onTokenChangeRef.current(null);
    };
  }, [frameKey]);

  return (
    <div className="w-full min-h-[70px]">
      <iframe
        key={frameKey}
        ref={frameRef}
        src={captchaUrl()}
        title={t("anonenCloud.captchaFrameTitle")}
        sandbox="allow-scripts allow-same-origin allow-forms"
        referrerPolicy="no-referrer"
        allow=""
        className="w-full h-[92px] border-0 bg-transparent"
      />
      {loadFailed && (
        <div className="text-center space-y-1" role="status">
          <p className="text-xs text-red-400">
            {t("anonenCloud.errors.captchaUnavailable")}
          </p>
          <button
            type="button"
            className="text-xs underline text-text/70 hover:text-text"
            onClick={() => {
              setLoadFailed(false);
              setRetryKey((key) => key + 1);
            }}
          >
            {t("anonenCloud.retryCaptcha")}
          </button>
        </div>
      )}
    </div>
  );
};

export default TurnstileChallenge;
