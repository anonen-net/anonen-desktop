import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { SettingContainer } from "../ui/SettingContainer";
import { Button } from "../ui/Button";

import noticesText from "../../../NOTICE.md?raw";

import thirdPartyText from "../../../THIRD-PARTY-NOTICES.md?raw";

export const ThirdPartyNotices: React.FC<{ grouped?: boolean }> = ({
  grouped = false,
}) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);

  return (
    <SettingContainer
      title={t("settings.licenses.title")}
      description={t("settings.licenses.description")}
      grouped={grouped}
      layout="stacked"
    >
      <div className="flex flex-col gap-3 w-full">
        <div className="text-sm text-mid-gray">
          {t("settings.about.acknowledgments.handy.details")}
        </div>
        <div>
          <Button
            variant="secondary"
            size="sm"
            onClick={() => setOpen((v) => !v)}
          >
            {open ? t("settings.licenses.hide") : t("settings.licenses.show")}
          </Button>
        </div>
        {open && (
          <div className="max-h-80 overflow-auto rounded-md border border-border bg-background p-3">
            <pre className="text-xs leading-relaxed text-mid-gray whitespace-pre-wrap break-words font-mono">
              {noticesText}
              {"\n\n"}
              {thirdPartyText}
            </pre>
          </div>
        )}
      </div>
    </SettingContainer>
  );
};
