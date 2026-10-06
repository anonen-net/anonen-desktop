import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { commands } from "@/bindings";
import { useSettings } from "../../hooks/useSettings";
import { ShortcutInput } from "../settings/ShortcutInput";

const SHORTCUT_PROBE_EVENT = "anonen-shortcut-probe";

interface ShortcutOnboardingProps {
  onComplete: () => void;
}

const ShortcutOnboarding: React.FC<ShortcutOnboardingProps> = ({
  onComplete,
}) => {
  const { t } = useTranslation();
  const { getSetting } = useSettings();
  const [pressed, setPressed] = useState(false);
  const binding = getSetting("bindings")?.transcribe?.current_binding ?? "";

  useEffect(() => setPressed(false), [binding]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    commands.startShortcutProbe().catch(console.error);
    listen(SHORTCUT_PROBE_EVENT, () => setPressed(true))
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch(() => {});
    return () => {
      disposed = true;
      unlisten?.();
      commands.stopShortcutProbe().catch(console.error);
    };
  }, []);

  return (
    <div className="h-screen w-screen overflow-y-auto flex flex-col items-center justify-center p-8">
      <div className="w-full max-w-xl flex flex-col gap-6">
        <header className="flex flex-col gap-2 text-center">
          <h1 className="text-2xl font-semibold text-text text-balance">
            {t("onboarding.shortcut.title")}
          </h1>
          <p className="text-text/70 text-pretty">
            {t("onboarding.shortcut.description")}
          </p>
        </header>

        <div className="rounded-lg border-2 border-logo-primary/60 bg-surface shadow-hard-sm px-5 py-3">
          <ShortcutInput
            shortcutId="transcribe"
            descriptionMode="inline"
            layout="stacked"
            align="center"
            title={t("onboarding.shortcut.inputTitle")}
            description={t("onboarding.shortcut.inputDescription")}
          />
        </div>

        <div className="h-6 -mt-3 flex items-center justify-center">
          {pressed && (
            <p className="flex items-center gap-2 text-sm font-medium text-emerald-500">
              <Check className="w-4 h-4 shrink-0" />
              {t("onboarding.shortcut.pressed")}
            </p>
          )}
        </div>

        <p className="text-sm text-muted text-pretty text-center">
          {t("onboarding.shortcut.changeLater")}
        </p>

        <div className="flex justify-center">
          <button
            onClick={onComplete}
            className="px-6 py-2.5 rounded-lg bg-logo-primary hover:bg-logo-primary/90 text-white font-medium transition-colors"
          >
            {t("onboarding.shortcut.next")}
          </button>
        </div>
      </div>
    </div>
  );
};

export default ShortcutOnboarding;
