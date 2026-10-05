import React, { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { TriangleAlert } from "lucide-react";
import { commands } from "@/bindings";
import { InputVolumeSlider } from "./settings/InputVolumeSlider";

const MicClippingNotice: React.FC = () => {
  const { t } = useTranslation();
  const [clipping, setClipping] = useState(false);

  const refresh = useCallback(() => {
    commands
      .getLastInputLevel()
      .then((level) => setClipping(level?.clipping === true))

      .catch(() => setClipping(false));
  }, []);

  useEffect(() => {
    refresh();

    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, [refresh]);

  if (!clipping) return null;

  return (
    <div className="w-full rounded-lg border-2 border-amber-500/50 bg-surface px-5 py-4 flex gap-3">
      <TriangleAlert className="w-4 h-4 shrink-0 mt-1 text-amber-500" />
      <div className="flex flex-col gap-3 min-w-0 flex-1">
        <div className="flex flex-col gap-1">
          <p className="font-medium text-text">{t("micClipping.title")}</p>
          <p className="text-sm text-muted text-pretty">
            {t("micClipping.body")}
          </p>
        </div>

        <InputVolumeSlider descriptionMode="inline" />
      </div>
    </div>
  );
};

export default MicClippingNotice;
