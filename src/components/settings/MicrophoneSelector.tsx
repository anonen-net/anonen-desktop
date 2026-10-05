import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Dropdown } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";
import { ResetButton } from "../ui/ResetButton";
import { useSettings } from "../../hooks/useSettings";
import { commands } from "../../bindings";

const LOW_VOLUME_RMS_THRESHOLD = 0.03;

interface MicrophoneSelectorProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const MicrophoneSelector: React.FC<MicrophoneSelectorProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const {
      getSetting,
      updateSetting,
      resetSetting,
      isUpdating,
      isLoading,
      audioDevices,
      refreshAudioDevices,
    } = useSettings();

    const [lastRmsLevel, setLastRmsLevel] = useState<number | null>(null);

    const isLowVolume =
      lastRmsLevel !== null && lastRmsLevel < LOW_VOLUME_RMS_THRESHOLD;
    const volumeDb =
      lastRmsLevel && lastRmsLevel > 0
        ? Math.round(20 * Math.log10(lastRmsLevel))
        : null;

    useEffect(() => {
      commands
        .getLastRecordingLevel()
        .then(setLastRmsLevel)
        .catch(() => {});
    }, [audioDevices]);

    const selectedMicrophone =
      getSetting("selected_microphone") === "default"
        ? "Default"
        : getSetting("selected_microphone") || "Default";

    const handleMicrophoneSelect = async (deviceName: string) => {
      await updateSetting("selected_microphone", deviceName);
    };

    const handleReset = async () => {
      await resetSetting("selected_microphone");
    };

    const microphoneOptions = audioDevices.map((device) => ({
      value: device.name,
      label: device.name,
    }));

    return (
      <div>
        <SettingContainer
          title={t("settings.sound.microphone.title")}
          description={t("settings.sound.microphone.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <div className="flex items-center space-x-1">
            <Dropdown
              options={microphoneOptions}
              selectedValue={selectedMicrophone}
              onSelect={handleMicrophoneSelect}
              placeholder={
                isLoading || audioDevices.length === 0
                  ? t("settings.sound.microphone.loading")
                  : t("settings.sound.microphone.placeholder")
              }
              disabled={
                isUpdating("selected_microphone") ||
                isLoading ||
                audioDevices.length === 0
              }
              onRefresh={refreshAudioDevices}
            />
            <ResetButton
              onClick={handleReset}
              disabled={isUpdating("selected_microphone") || isLoading}
            />
          </div>
        </SettingContainer>
        {isLowVolume && (
          <div className="mx-4 mt-1 border-2 border-border rounded-md bg-yellow-500/10 px-3 py-2 text-xs text-yellow-700 dark:text-yellow-200">
            <div className="mb-1 font-medium">
              {t("settings.sound.microphone.lowVolumeTitle")}
            </div>
            <div>
              {t("settings.sound.microphone.lowVolumeWarning", {
                db: volumeDb ?? "?",
              })}
            </div>
          </div>
        )}
      </div>
    );
  },
);
