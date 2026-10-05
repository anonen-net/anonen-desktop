import React from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ClipboardCopy, ExternalLink } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { buildDiagnosticsReport } from "@/lib/diagnostics";
import { Button } from "../ui/Button";

const SUPPORT_URL = "https://anonen.net/support/";

interface SupportActionsProps {
  variant?: "secondary" | "ghost";
  size?: "sm" | "md";

  className?: string;
}

export const SupportActions: React.FC<SupportActionsProps> = ({
  variant = "secondary",
  size = "md",
  className = "",
}) => {
  const { t } = useTranslation();

  const openContact = async () => {
    try {
      await openUrl(SUPPORT_URL);
    } catch {
      toast.error(t("settings.support.openFailed"));
    }
  };

  const copyDiagnostics = async () => {
    try {
      await navigator.clipboard.writeText(await buildDiagnosticsReport());
      toast.success(t("settings.support.diagnosticsCopied"));
    } catch {
      toast.error(t("settings.support.copyFailed"));
    }
  };

  return (
    <div className={`flex flex-wrap gap-2 ${className}`}>
      <Button type="button" variant={variant} size={size} onClick={openContact}>
        <ExternalLink className="w-4 h-4 inline-block me-2" />
        {t("settings.support.openContact")}
      </Button>
      <Button
        type="button"
        variant={variant}
        size={size}
        onClick={copyDiagnostics}
      >
        <ClipboardCopy className="w-4 h-4 inline-block me-2" />
        {t("settings.support.copyDiagnostics")}
      </Button>
    </div>
  );
};
