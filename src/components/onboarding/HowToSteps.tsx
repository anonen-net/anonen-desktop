import React from "react";
import { Trans, useTranslation } from "react-i18next";
import { Kbd } from "../ui/Kbd";

export type HowToActive = "press" | "speak" | "stop" | null;

interface HowToStepsProps {
  binding: string;
  cancelBinding: string;

  hold: boolean;

  active?: HowToActive;

  sampleText?: string;

  compact?: boolean;
}

export const HowToSteps: React.FC<HowToStepsProps> = ({
  binding,
  cancelBinding,
  hold,
  active = null,
  sampleText,
  compact = false,
}) => {
  const { t } = useTranslation();
  const size = compact ? "sm" : "md";
  const key = <Kbd binding={binding} size={size} />;
  const steps: Array<{
    id: Exclude<HowToActive, null>;
    label: React.ReactNode;
    caption?: string;

    highlight?: string;
  }> = [
    {
      id: "press",
      label: (
        <Trans
          i18nKey={hold ? "howtoSteps.pressHold" : "howtoSteps.press"}
          components={{ key }}
        />
      ),
      caption: t("howtoSteps.pressCaption"),
    },
    {
      id: "speak",
      label: t("howtoSteps.speak"),
      caption: sampleText ? t("howtoSteps.speakSample") : undefined,
      highlight: sampleText,
    },
    {
      id: "stop",
      label: (
        <Trans
          i18nKey={hold ? "howtoSteps.stopHold" : "howtoSteps.stop"}
          components={{ key }}
        />
      ),
      caption: t("howtoSteps.stopCaption"),
    },
  ];
  const pad = compact ? "px-3 py-2" : "px-4 py-3";

  return (
    <div className="flex flex-col gap-2">
      <ol className="flex flex-col gap-2">
        {steps.map((step, index) => {
          const isActive = active === step.id;
          return (
            <li
              key={step.id}
              className={`flex items-start gap-3 rounded-lg border-2 ${pad} transition-colors ${
                isActive
                  ? "border-logo-primary bg-logo-primary/10"
                  : "border-border bg-surface"
              }`}
            >
              <span
                className={`shrink-0 w-6 h-6 rounded-full flex items-center justify-center text-xs font-bold ${
                  isActive
                    ? "bg-logo-primary text-white"
                    : "bg-background border border-border text-muted"
                }`}
              >
                {index + 1}
              </span>
              <div className="flex flex-col gap-1 min-w-0">
                <span
                  className={`${compact ? "text-sm" : "text-base"} font-medium text-text leading-7`}
                >
                  {step.label}
                </span>
                {step.caption && (
                  <span className="text-xs text-muted text-pretty">
                    {step.caption}
                  </span>
                )}
                {step.highlight && (
                  <span className="text-lg font-semibold text-text text-pretty">
                    {t("howtoSteps.speakQuote", { text: step.highlight })}
                  </span>
                )}
              </div>
            </li>
          );
        })}
      </ol>
      <p
        className={`${compact ? "text-xs" : "text-sm"} text-muted flex items-center gap-2 px-1`}
      >
        <span>{t("howtoSteps.cancel")}</span>
        <Kbd binding={cancelBinding} size="sm" />
      </p>
    </div>
  );
};
