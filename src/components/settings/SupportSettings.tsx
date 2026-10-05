import React from "react";
import { useTranslation } from "react-i18next";
import { InfoTip } from "../ui/InfoTip";
import { SettingContainer } from "../ui/SettingContainer";
import { SupportActions } from "./SupportActions";

export const SupportSettings: React.FC = () => {
  const { t } = useTranslation();

  return (
    <SettingContainer
      title={t("settings.support.title")}
      description={t("settings.support.intro")}
      descriptionMode="inline"
      grouped={true}
      layout="stacked"
    >
      <div className="pt-1 flex flex-wrap items-center gap-2">
        <SupportActions />

        <InfoTip
          text={t("settings.support.description")}
          label={t("settings.support.diagnosticsInfo")}
          width={320}
        />
      </div>
    </SettingContainer>
  );
};
