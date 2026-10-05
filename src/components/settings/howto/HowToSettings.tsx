import React from "react";
import { useTranslation } from "react-i18next";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { useSettings } from "../../../hooks/useSettings";
import { HowToSteps } from "../../onboarding/HowToSteps";

export const HowToSettings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting } = useSettings();
  const hold = getSetting("push_to_talk") === true;
  const bindings = getSetting("bindings");
  const binding = bindings?.transcribe?.current_binding ?? "";
  const cancelBinding = bindings?.cancel?.current_binding ?? "";

  const rows: Array<{ label: string; body: string }> = [
    {
      label: t("settings.howto.whereItGoes.label"),
      body: t("settings.howto.whereItGoes.body"),
    },
  ];

  return (
    <div className="max-w-3xl mx-auto w-full flex flex-col gap-4">
      <SettingsGroup title={t("settings.howto.title")}>
        <div className="px-4 py-3 flex flex-col gap-4">
          <HowToSteps
            binding={binding}
            cancelBinding={cancelBinding}
            hold={hold}
            compact
          />
          <div className="flex flex-col gap-3 border-t border-border pt-3">
            {rows.map((row) => (
              <div key={row.label} className="flex flex-col gap-0.5">
                <p className="text-xs text-muted">{row.label}</p>
                <p className="text-sm text-text text-pretty">{row.body}</p>
              </div>
            ))}
          </div>
        </div>
      </SettingsGroup>
    </div>
  );
};
