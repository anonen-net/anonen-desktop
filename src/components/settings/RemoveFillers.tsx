import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";
import { MODEL_FEATURE_ICONS } from "../ui/ModelFeatureBadge";

interface RemoveFillersProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const RemoveFillers: React.FC<RemoveFillersProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const FeatureIcon = MODEL_FEATURE_ICONS.fillerReduction;

    const enabled = getSetting("cloud_remove_fillers") ?? false;

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(next) => updateSetting("cloud_remove_fillers", next)}
        isUpdating={isUpdating("cloud_remove_fillers")}
        label={t("settings.modelSettings.removeFillers.label")}
        description={t("settings.modelSettings.removeFillers.description")}
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
