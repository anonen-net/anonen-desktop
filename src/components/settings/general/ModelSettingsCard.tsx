import React from "react";
import { useTranslation } from "react-i18next";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { LanguageSelector } from "../LanguageSelector";
import { TranslateToEnglish } from "../TranslateToEnglish";
import { RemoveFillers } from "../RemoveFillers";
import { useModelStore } from "../../../stores/modelStore";
import { useAnonenCloudStore } from "../../../stores/anonenCloudStore";
import type { ModelInfo } from "@/bindings";
import { modelFeatures } from "../../../lib/modelFeatures";

export const ModelSettingsCard: React.FC = () => {
  const { t } = useTranslation();
  const { currentModel, models } = useModelStore();

  const currentModelInfo = models.find((m: ModelInfo) => m.id === currentModel);

  const cloudModels = useAnonenCloudStore((s) => s.cloudModels);
  const features = modelFeatures(
    {
      id: currentModel,
      supports_translation: currentModelInfo?.supports_translation ?? false,
    },
    cloudModels,
  );
  const supportsTranslation = features.includes("translation");
  const supportsFillerRemoval = features.includes("fillerReduction");

  return (
    <SettingsGroup title={t("settings.modelSettings.title")}>
      <LanguageSelector descriptionMode="tooltip" grouped={true} />
      {supportsTranslation && (
        <TranslateToEnglish descriptionMode="tooltip" grouped={true} />
      )}
      {supportsFillerRemoval && (
        <RemoveFillers descriptionMode="tooltip" grouped={true} />
      )}
    </SettingsGroup>
  );
};
