import React, { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Dropdown } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";
import { useSettings } from "../../hooks/useSettings";
import {
  commands,
  type HistoryAudioUsage,
  type HistoryRetention,
} from "@/bindings";

export const RECENT_LIMIT = 20;

const BYTES_PER_MB = 1_000_000;

export function useHistoryRetentionOptions() {
  const { t } = useTranslation();
  return [
    { value: "none", label: t("settings.historyRetention.options.none") },
    {
      value: "recent",
      label: t("settings.historyRetention.options.recent", {
        count: RECENT_LIMIT,
      }),
    },
    {
      value: "unlimited",
      label: t("settings.historyRetention.options.unlimited"),
    },
  ];
}

interface HistoryRetentionProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const HistoryRetentionSelector: React.FC<HistoryRetentionProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const options = useHistoryRetentionOptions();

    const selected = getSetting("history_retention") ?? "recent";

    const [usage, setUsage] = useState<HistoryAudioUsage | null>(null);
    const refreshUsage = useCallback(async () => {
      const result = await commands.getHistoryAudioUsage();
      if (result.status === "ok") setUsage(result.data);
    }, []);
    useEffect(() => {
      void refreshUsage();
    }, [refreshUsage]);

    const handleSelect = async (value: string) => {
      await updateSetting("history_retention", value as HistoryRetention);
      await refreshUsage();
    };

    const usageLabel = (u: HistoryAudioUsage): string => {
      if (u.count === 0) return t("settings.historyRetention.usage.none");
      if (u.bytes < BYTES_PER_MB) {
        return t("settings.historyRetention.usage.underOneMb", {
          count: u.count,
        });
      }
      return t("settings.historyRetention.usage.megabytes", {
        count: u.count,
        mb: Math.round(u.bytes / BYTES_PER_MB),
      });
    };

    return (
      <SettingContainer
        title={t("settings.historyRetention.title")}
        description={t("settings.historyRetention.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      >
        <div className="flex flex-col items-end gap-1">
          <Dropdown
            options={options}
            selectedValue={selected}
            onSelect={handleSelect}
            disabled={isUpdating("history_retention")}
          />
          {usage && (
            <span className="text-xs text-mid-gray">{usageLabel(usage)}</span>
          )}
        </div>
      </SettingContainer>
    );
  });

HistoryRetentionSelector.displayName = "HistoryRetentionSelector";
