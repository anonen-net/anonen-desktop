import React from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../../ui/Button";
import {
  useThemeStore,
  THEME_DEFAULTS,
  type ThemeMode,
} from "../../../stores/themeStore";

const MODES: ThemeMode[] = ["system", "light", "dark"];

export const ThemeColorPicker: React.FC = () => {
  const { t } = useTranslation();
  const { colors, mode, setColors, setMode } = useThemeStore();

  const isDefault =
    colors.accentColor === THEME_DEFAULTS.accentColor &&
    colors.accentColorLight === THEME_DEFAULTS.accentColorLight;

  return (
    <div className="px-4 py-3 space-y-3">
      <div className="flex items-center justify-between">
        <div className="text-sm font-medium">
          {t("settings.theme.mode.label")}
        </div>
        <div className="flex items-center gap-1">
          {MODES.map((m) => (
            <Button
              key={m}
              size="sm"
              variant={mode === m ? "primary-soft" : "ghost"}
              onClick={() => setMode(m)}
            >
              {t(`settings.theme.mode.${m}`)}
            </Button>
          ))}
        </div>
      </div>
      <div className="flex items-center justify-between">
        <div className="text-sm font-medium">
          {t("settings.theme.accentColor")}
        </div>
        <div className="flex items-center gap-3">
          {!isDefault && (
            <button
              type="button"
              onClick={() =>
                setColors({
                  accentColor: THEME_DEFAULTS.accentColor,
                  accentColorLight: THEME_DEFAULTS.accentColorLight,
                })
              }
              className="text-xs text-muted/80 hover:text-text/70 transition-colors cursor-pointer"
            >
              {t("common.reset")}
            </button>
          )}
          <label className="flex items-center gap-1.5 cursor-pointer">
            <span className="text-xs text-muted">
              {t("settings.theme.dark")}
            </span>
            <input
              type="color"
              value={colors.accentColor}
              onChange={(e) => setColors({ accentColor: e.target.value })}
              className="w-7 h-7 rounded cursor-pointer border border-mid-gray/30 bg-transparent [&::-webkit-color-swatch-wrapper]:p-0.5 [&::-webkit-color-swatch]:rounded"
            />
          </label>
          <label className="flex items-center gap-1.5 cursor-pointer">
            <span className="text-xs text-muted">
              {t("settings.theme.light")}
            </span>
            <input
              type="color"
              value={colors.accentColorLight}
              onChange={(e) => setColors({ accentColorLight: e.target.value })}
              className="w-7 h-7 rounded cursor-pointer border border-mid-gray/30 bg-transparent [&::-webkit-color-swatch-wrapper]:p-0.5 [&::-webkit-color-swatch]:rounded"
            />
          </label>
        </div>
      </div>
    </div>
  );
};
