import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";
import { MODEL_FEATURE_ICONS } from "../ui/ModelFeatureBadge";

interface TranslateToEnglishProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const TranslateToEnglish: React.FC<TranslateToEnglishProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const FeatureIcon = MODEL_FEATURE_ICONS.translation;

    const translateToEnglish = getSetting("translate_to_english") || false;

    return (
      <ToggleSwitch
        checked={translateToEnglish}
        onChange={(enabled) => updateSetting("translate_to_english", enabled)}
        isUpdating={isUpdating("translate_to_english")}
        label={t("settings.advanced.translateToEnglish.label")}
        description={t("settings.advanced.translateToEnglish.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
        titleIcon={
          <FeatureIcon
            className="w-3.5 h-3.5 text-text/60"
            aria-hidden="true"
          />
        }
      />
    );
  },
);
