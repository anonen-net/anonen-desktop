import React from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "../ui/Button";
import { SettingContainer } from "../ui/SettingContainer";

const PRIVACY_URL = "https://anonen.net/legal/privacy";
const TOKUSHOHO_URL = "https://anonen.net/legal/tokushoho";

export const LegalLinks: React.FC = () => {
  const { t } = useTranslation();

  const open = async (url: string) => {
    try {
      await openUrl(url);
    } catch {
      toast.error(t("settings.about.legal.openFailed"));
    }
  };

  return (
    <SettingContainer
      title={t("settings.about.legal.title")}
      description={t("settings.about.legal.description")}
      grouped={true}
      layout="stacked"
    >
      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          variant="secondary"
          size="sm"
          className="whitespace-nowrap"
          onClick={() => open(PRIVACY_URL)}
        >
          {t("settings.about.legal.privacy")}
        </Button>
        <Button
          type="button"
          variant="secondary"
          size="sm"
          className="whitespace-nowrap"
          onClick={() => open(TOKUSHOHO_URL)}
        >
          {t("settings.about.legal.tokushoho")}
        </Button>
      </div>
    </SettingContainer>
  );
};
