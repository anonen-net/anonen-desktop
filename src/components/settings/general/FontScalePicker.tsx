import React from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../../ui/Button";
import {
  useFontScaleStore,
  FONT_SCALES,
  type FontScale,
} from "../../../stores/fontScaleStore";

export const FontScalePicker: React.FC = () => {
  const { t } = useTranslation();
  const { scale, setScale } = useFontScaleStore();

  return (
    <div className="px-4 py-3">
      <div className="flex items-center justify-between gap-2">
        <div className="text-sm font-medium shrink-0">
          {t("settings.fontScale.label")}
        </div>
        <div className="flex items-center gap-1 shrink-0">
          {FONT_SCALES.map((value: FontScale) => (
            <Button
              key={value}
              size="sm"
              variant={scale === value ? "primary-soft" : "ghost"}
              onClick={() => setScale(value)}
              title={`${Math.round(value * 100)}%`}
            >
              {Math.round(value * 100)}%
            </Button>
          ))}
        </div>
      </div>
      <p className="text-xs text-muted mt-1.5">
        {t("settings.fontScale.description")}
      </p>
    </div>
  );
};
