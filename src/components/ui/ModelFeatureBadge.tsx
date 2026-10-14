import React from "react";
import { useTranslation } from "react-i18next";
import { Languages, Scissors, type LucideIcon } from "lucide-react";
import Badge from "./Badge";
import type { ModelFeature } from "../../lib/modelFeatures";

export const MODEL_FEATURE_ICONS: Record<ModelFeature, LucideIcon> = {
  translation: Languages,
  fillerReduction: Scissors,
};

const LABEL_KEYS: Record<ModelFeature, string> = {
  translation: "modelSelector.features.translation",
  fillerReduction: "modelSelector.features.fillerReduction",
};

export const ModelFeatureBadge: React.FC<{
  feature: ModelFeature;
  size?: "md" | "sm";
}> = ({ feature, size = "md" }) => {
  const { t } = useTranslation();
  const Icon = MODEL_FEATURE_ICONS[feature];
  return (
    <Badge variant="secondary" size={size}>
      <Icon
        className={size === "sm" ? "w-2.5 h-2.5 me-0.5" : "w-3 h-3 me-1"}
        aria-hidden="true"
      />
      {t(LABEL_KEYS[feature])}
    </Badge>
  );
};
