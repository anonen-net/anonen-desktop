import React from "react";
import { useTranslation } from "react-i18next";
import { ShortcutInput } from "../settings/ShortcutInput";

interface ShortcutOnboardingProps {
  onComplete: () => void;
}

const ShortcutOnboarding: React.FC<ShortcutOnboardingProps> = ({
  onComplete,
}) => {
  const { t } = useTranslation();

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
