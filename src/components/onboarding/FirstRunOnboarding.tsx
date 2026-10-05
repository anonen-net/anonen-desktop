import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { events } from "@/bindings";
import { useSettings } from "../../hooks/useSettings";
import LinuxShortcutNotice from "../LinuxShortcutNotice";
import { HowToSteps, type HowToActive } from "./HowToSteps";

interface FirstRunOnboardingProps {
  onComplete: () => void;
}

const RECORDING_STATE_EVENT = "anonen-recording-state";

type Phase = "idle" | "recording" | "working";

export const phaseOf = (state: string): Phase => {
  switch (state) {
    case "preparing":
    case "recording":
    case "low-volume":
    case "limit-warning":
      return "recording";
    case "transcribing":
    case "processing":
      return "working";
    default:
      return "idle";
  }
};

export const SPEAK_STEP_MS = 2000;

export const activeStepOf = (
  phase: Phase,
  recordingStep: "speak" | "stop",
): HowToActive => {
  switch (phase) {
    case "idle":
      return "press";
    case "recording":
      return recordingStep;
    default:
      return null;
  }
};

const FirstRunOnboarding: React.FC<FirstRunOnboardingProps> = ({
  onComplete,
}) => {
  const { t } = useTranslation();
  const { getSetting } = useSettings();
  const [heard, setHeard] = useState<string | null>(null);
  const [phase, setPhase] = useState<Phase>("idle");
  const [recordingStep, setRecordingStep] = useState<"speak" | "stop">("speak");

  useEffect(() => {
    if (phase !== "recording") return;
    setRecordingStep("speak");
    const id = window.setTimeout(() => setRecordingStep("stop"), SPEAK_STEP_MS);
    return () => window.clearTimeout(id);
  }, [phase]);

  const binding = getSetting("bindings")?.transcribe?.current_binding ?? "";
  const cancelBinding = getSetting("bindings")?.cancel?.current_binding ?? "";

  const hold = getSetting("push_to_talk") === true;

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    listen<string>(RECORDING_STATE_EVENT, (event) => {
      setPhase(phaseOf(event.payload));
    })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch(() => {});
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    events.historyUpdatePayload
      .listen((event) => {
        if (event.payload.action !== "added") return;
        const text = event.payload.entry.transcription_text.trim();
        if (text.length === 0) return;
        setHeard(text);
      })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch(() => {});
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  return (
    <div className="h-screen w-screen overflow-y-auto flex flex-col items-center justify-center p-8">
      <div className="w-full max-w-xl flex flex-col gap-6">
        <header className="flex flex-col gap-2 text-center">
          <h1 className="text-2xl font-semibold text-text text-balance">
            {t("onboarding.firstRun.title")}
          </h1>
          <p className="text-text/70 text-pretty">
            {t("onboarding.firstRun.description")}
          </p>
        </header>

        <LinuxShortcutNotice />

        <div className="h-6 flex items-center justify-center">
          {phase === "recording" && (
            <p className="flex items-center gap-2 text-sm font-medium text-emerald-500">
              <span className="w-2.5 h-2.5 rounded-full bg-emerald-500 animate-pulse" />
              {t("onboarding.firstRun.recording")}
            </p>
          )}
          {phase === "working" && (
            <p className="text-sm font-medium text-text">
              {t("onboarding.firstRun.working")}
            </p>
          )}
        </div>

        <HowToSteps
          binding={binding}
          cancelBinding={cancelBinding}
          hold={hold}
          active={
            heard !== null && phase === "idle"
              ? null
              : activeStepOf(phase, recordingStep)
          }
          sampleText={t("onboarding.firstRun.sampleText")}
        />

        {heard !== null && (
          <div className="rounded-lg border-2 border-emerald-500/50 bg-surface px-5 py-4 flex flex-col gap-2">
            <p className="flex items-center gap-2 text-emerald-500 font-medium">
              <Check className="w-4 h-4 shrink-0" />
              {t("onboarding.firstRun.success")}
            </p>
            <p className="text-text break-words">{heard}</p>
            <p className="text-xs text-muted text-pretty">
              {t("onboarding.firstRun.whereItGoes")}
            </p>
          </div>
        )}

        <div className="flex justify-center">
          <button
            onClick={onComplete}
            className={
              heard !== null
                ? "px-6 py-2.5 rounded-lg bg-logo-primary hover:bg-logo-primary/90 text-white font-medium transition-colors"
                : "px-6 py-2.5 rounded-lg border-2 border-border font-medium text-muted hover:text-text transition-colors"
            }
          >
            {heard !== null
              ? t("onboarding.firstRun.finish")
              : t("onboarding.firstRun.skip")}
          </button>
        </div>
      </div>
    </div>
  );
};

export default FirstRunOnboarding;
