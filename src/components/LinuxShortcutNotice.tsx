import React from "react";
import { useTranslation } from "react-i18next";
import { TriangleAlert } from "lucide-react";
import { useOsType } from "@/hooks/useOsType";
import { useSettings } from "../hooks/useSettings";

const LinuxShortcutNotice: React.FC = () => {
  const { t } = useTranslation();
  const osType = useOsType();
  const { getSetting } = useSettings();

  if (osType !== "linux") return null;
  if (getSetting("keyboard_implementation") !== "tauri") return null;

  return (
    <div className="rounded-lg border-2 border-amber-500/50 bg-surface px-5 py-4 flex gap-3">
      <TriangleAlert className="w-4 h-4 shrink-0 mt-1 text-amber-500" />
      <div className="flex flex-col gap-1">
        <p className="font-medium text-text">{t("linuxShortcut.title")}</p>
        <p className="text-sm text-muted text-pretty">
          {t("linuxShortcut.body")}
        </p>
      </div>
    </div>
  );
};

export default LinuxShortcutNotice;
