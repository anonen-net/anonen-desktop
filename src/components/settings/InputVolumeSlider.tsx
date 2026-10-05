import React, { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Slider } from "../ui/Slider";
import { commands } from "@/bindings";

interface InputVolumeSliderProps {
  grouped?: boolean;
  descriptionMode?: "inline" | "tooltip";
}

export const InputVolumeSlider: React.FC<InputVolumeSliderProps> = ({
  grouped = false,
  descriptionMode = "tooltip",
}) => {
  const { t } = useTranslation();
  const [volume, setVolume] = useState<number | null>(null);
  const writeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const load = useCallback(() => {
    commands
      .getInputVolume()
      .then((res) => setVolume(res.status === "ok" ? res.data : null))
      .catch(() => setVolume(null));
  }, []);

  useEffect(() => {
    load();

    window.addEventListener("focus", load);
    return () => {
      window.removeEventListener("focus", load);
      if (writeTimer.current) clearTimeout(writeTimer.current);
    };
  }, [load]);

  if (volume === null) return null;

  const onChange = (value: number) => {
    setVolume(value);
    if (writeTimer.current) clearTimeout(writeTimer.current);
    writeTimer.current = setTimeout(() => {
      commands.setInputVolume(value).catch(() => load());
    }, 150);
  };

  return (
    <Slider
      value={volume}
      onChange={onChange}
      min={0}
      max={1}
      step={0.01}
      label={t("settings.recording.inputVolume.title")}
      description={t("settings.recording.inputVolume.description")}
      descriptionMode={descriptionMode}
      grouped={grouped}
      formatValue={(value) => `${Math.round(value * 100)}%`}
    />
  );
};
