import { listen } from "@tauri-apps/api/event";
import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  MicrophoneIcon,
  TranscriptionIcon,
  CancelIcon,
  LowVolumeIcon,
} from "../components/icons";
import "./RecordingOverlay.css";
import { commands } from "@/bindings";
import i18n, { syncLanguageFromSettings } from "@/i18n";
import { getLanguageDirection } from "@/lib/utils/rtl";

type OverlayState =
  | "preparing"
  | "recording"
  | "transcribing"
  | "processing"
  | "low-volume"
  | "limit-warning";

const LIMIT_WARNING_FLASH_MS = 4000;

const RecordingOverlay: React.FC = () => {
  const { t } = useTranslation();
  const [isVisible, setIsVisible] = useState(false);
  const [state, setState] = useState<OverlayState>("recording");
  const [levels, setLevels] = useState<number[]>(Array(16).fill(0));
  const smoothedLevelsRef = useRef<number[]>(Array(16).fill(0));
  const limitWarningTimerRef = useRef<number | null>(null);
  const direction = getLanguageDirection(i18n.language);

  useEffect(() => {
    const clearLimitWarningTimer = () => {
      if (limitWarningTimerRef.current !== null) {
        window.clearTimeout(limitWarningTimerRef.current);
        limitWarningTimerRef.current = null;
      }
    };

    const setupEventListeners = async () => {
      const unlistenShow = await listen("show-overlay", (event) => {
        const overlayState = event.payload as OverlayState;
        clearLimitWarningTimer();
        setState(overlayState);
        setIsVisible(true);
        if (overlayState === "limit-warning") {
          limitWarningTimerRef.current = window.setTimeout(() => {
            limitWarningTimerRef.current = null;
            setState("recording");
          }, LIMIT_WARNING_FLASH_MS);
        }

        void syncLanguageFromSettings();
      });

      const unlistenHide = await listen("hide-overlay", () => {
        clearLimitWarningTimer();
        setIsVisible(false);
      });

      const unlistenLevel = await listen<number[]>("mic-level", (event) => {
        const newLevels = event.payload as number[];

        const smoothed = smoothedLevelsRef.current.map((prev, i) => {
          const target = newLevels[i] || 0;
          return prev * 0.7 + target * 0.3;
        });

        smoothedLevelsRef.current = smoothed;
        setLevels(smoothed.slice(0, 9));
      });

      return () => {
        unlistenShow();
        unlistenHide();
        unlistenLevel();
      };
    };

    setupEventListeners();
  }, []);

  const getIcon = () => {
    if (state === "low-volume") {
      return <LowVolumeIcon />;
    } else if (
      state === "recording" ||
      state === "preparing" ||
      state === "limit-warning"
    ) {
      return <MicrophoneIcon />;
    } else {
      return <TranscriptionIcon />;
    }
  };

  return (
    <div
      dir={direction}
      className={`recording-overlay ${isVisible ? "fade-in" : ""}`}
    >
      {isVisible && (
        <>
          <div className="overlay-left">{getIcon()}</div>

          <div className="overlay-middle">
            {state === "recording" && (
              <div className="bars-container">
                {levels.map((v, i) => (
                  <div
                    key={i}
                    className="bar"
                    style={{
                      height: `${Math.min(20, 4 + Math.pow(v, 0.7) * 16)}px`,
                      transition:
                        "height 60ms ease-out, opacity 120ms ease-out",
                      opacity: Math.max(0.2, v * 1.7),
                    }}
                  />
                ))}
              </div>
            )}
            {state === "preparing" && (
              <div className="transcribing-text">{t("overlay.preparing")}</div>
            )}
            {state === "transcribing" && (
              <div className="transcribing-text">
                {t("overlay.transcribing")}
              </div>
            )}
            {state === "processing" && (
              <div className="transcribing-text">{t("overlay.processing")}</div>
            )}
            {state === "low-volume" && (
              <div className="low-volume-text">{t("overlay.lowVolume")}</div>
            )}
            {state === "limit-warning" && (
              <div className="limit-warning-text">
                {t("overlay.limitWarning")}
              </div>
            )}
          </div>

          <div className="overlay-right">
            {(state === "recording" ||
              state === "preparing" ||
              state === "limit-warning") && (
              <div
                className="cancel-button"
                onClick={() => {
                  commands.cancelOperation();
                }}
              >
                <CancelIcon />
              </div>
            )}
          </div>
        </>
      )}
    </div>
  );
};

export default RecordingOverlay;
