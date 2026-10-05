import React, { useEffect, useState } from "react";
import LinuxShortcutNotice from "../../LinuxShortcutNotice";
import { useTranslation } from "react-i18next";
import { getVersion } from "@tauri-apps/api/app";
import { ask } from "@tauri-apps/plugin-dialog";
import { commands } from "@/bindings";

import { SettingsGroup } from "../../ui/SettingsGroup";
import { SettingContainer } from "../../ui/SettingContainer";

import { ShortcutInput } from "../ShortcutInput";
import { InputVolumeSlider } from "../InputVolumeSlider";
import { ModelSettingsCard } from "./ModelSettingsCard";
import { AnonenCloudPanel } from "@/components/settings/AnonenCloudPanel";
import { AnonenUsagePanel } from "@/components/settings/AnonenUsagePanel";

import { MicrophoneSelector } from "../MicrophoneSelector";
import { MuteWhileRecording } from "../MuteWhileRecording";
import { AudioFeedback } from "../AudioFeedback";
import { OutputDeviceSelector } from "../OutputDeviceSelector";

import { StartHidden } from "../StartHidden";
import { AutostartToggle } from "../AutostartToggle";
import { ShowTrayIcon } from "../ShowTrayIcon";
import { ShowOverlay } from "../ShowOverlay";
import { UpdateChecksToggle } from "../UpdateChecksToggle";

import { HistoryRetentionSelector } from "../HistoryRetention";
import { ThirdPartyNotices } from "../ThirdPartyNotices";

import { AppLanguageSelector } from "../AppLanguageSelector";
import { LegalLinks } from "../LegalLinks";
import { SupportSettings } from "../SupportSettings";

import { ThemeColorPicker } from "./ThemeColorPicker";
import { FontScalePicker } from "./FontScalePicker";

import { useSettings } from "../../../hooks/useSettings";
import { useThemeStore } from "../../../stores/themeStore";

export const GeneralSettings: React.FC = () => {
  const { t } = useTranslation();
  const { audioFeedbackEnabled, getSetting } = useSettings();

  const [version, setVersion] = useState("");
  useEffect(() => {
    const fetchVersion = async () => {
      try {
        const appVersion = await getVersion();
        setVersion(appVersion);
      } catch (error) {
        console.error("Failed to get app version:", error);

        setVersion("");
      }
    };
    fetchVersion();
  }, []);

  const resetTheme = useThemeStore((s) => s.reset);

  const handleResetAll = async () => {
    const confirmed = await ask(t("settings.resetAll.confirm"), {
      title: t("settings.resetAll.title"),
      kind: "warning",
    });
    if (!confirmed) return;
    resetTheme();
    await commands.resetAllSettings();
    window.location.reload();
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <AnonenCloudPanel />

      <LinuxShortcutNotice />
      <SettingsGroup title={t("settings.general.title")}>
        <ShortcutInput shortcutId="transcribe" grouped={true} />
      </SettingsGroup>

      <ModelSettingsCard />

      <SettingsGroup title={t("settings.recording.title")}>
        <MicrophoneSelector descriptionMode="tooltip" grouped={true} />

        <InputVolumeSlider grouped={true} />
        <MuteWhileRecording descriptionMode="tooltip" grouped={true} />
      </SettingsGroup>

      <SettingsGroup title={t("settings.recordingSound.title")}>
        <AudioFeedback descriptionMode="tooltip" grouped={true} />
        <OutputDeviceSelector
          descriptionMode="tooltip"
          grouped={true}
          disabled={!audioFeedbackEnabled}
        />
      </SettingsGroup>

      <SettingsGroup title={t("settings.advanced.groups.app")}>
        <StartHidden descriptionMode="tooltip" grouped={true} />
        <AutostartToggle descriptionMode="tooltip" grouped={true} />
        <ShowTrayIcon descriptionMode="tooltip" grouped={true} />
        <ShowOverlay descriptionMode="tooltip" grouped={true} />

        <UpdateChecksToggle descriptionMode="tooltip" grouped={true} />
      </SettingsGroup>

      <SettingsGroup title={t("settings.advanced.groups.history")}>
        <HistoryRetentionSelector descriptionMode="tooltip" grouped={true} />
      </SettingsGroup>

      <AnonenUsagePanel />

      <SettingsGroup title={t("settings.about.title")}>
        <AppLanguageSelector descriptionMode="tooltip" grouped={true} />
        <ThemeColorPicker />
        <FontScalePicker />
        <SupportSettings />

        <LegalLinks />

        <ThirdPartyNotices grouped={true} />
      </SettingsGroup>

      <div className="flex flex-col items-center gap-2 pt-2 pb-4">
        <button
          type="button"
          onClick={handleResetAll}
          className="text-sm text-muted/80 hover:text-red-500 transition-colors cursor-pointer"
        >
          {t("settings.resetAll.button")}
        </button>
        {version && (
          <p className="text-xs text-muted/60">
            {t("settings.general.version", { version })}
          </p>
        )}

        <p className="text-[10px] text-muted/40 font-mono">
          {__GIT_HASH__}
          {__GIT_COMMIT_DATE__ ? ` · ${__GIT_COMMIT_DATE__}` : ""}
        </p>
      </div>
    </div>
  );
};
