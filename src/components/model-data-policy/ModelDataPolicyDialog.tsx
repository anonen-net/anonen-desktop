import React from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { useAnonenCloudStore } from "@/stores/anonenCloudStore";
import { useModelStore } from "@/stores/modelStore";
import {
  isPolicyUnknown,
  retentionLine,
  trainingLine,
} from "@/lib/modelDataPolicy";
import { getLanguageDisplayText } from "@/lib/utils/languageDisplay";

export function ModelDataPolicyDialog() {
  const { t } = useTranslation();
  const cardId = useAnonenCloudStore((s) => s.pendingPolicyCardId);
  const requiresAck = useAnonenCloudStore((s) => s.pendingPolicyRequiresAck);
  const isCurrent = useAnonenCloudStore((s) => s.pendingPolicyIsCurrent);
  const dismissPolicy = useAnonenCloudStore((s) => s.dismissPolicy);
  const declineCurrentPolicy = useAnonenCloudStore(
    (s) => s.declineCurrentPolicy,
  );
  const acceptPolicy = useAnonenCloudStore((s) => s.acceptPolicy);
  const selectModel = useModelStore((s) => s.selectModel);
  const card = useModelStore((s) =>
    cardId ? (s.models.find((m) => m.id === cardId) ?? null) : null,
  );

  React.useEffect(() => {
    const unlisten = listen<string>(
      "anonen-model-disclosure-required",
      (event) => {
        const cloud = useAnonenCloudStore.getState();
        if (cloud.policyNeedingDisclosure(event.payload)) {
          cloud.showPolicy(event.payload, true);
        }
      },
    );
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  if (!cardId) return null;

  const upstream = useAnonenCloudStore.getState().policySubjectOf(cardId);
  const training = upstream ? trainingLine(upstream) : null;
  const retention = upstream ? retentionLine(upstream) : null;

  const usageMultiplier = card?.usage_multiplier ?? 1;
  const usageDeltaPercent = Math.round(Math.abs(1 - usageMultiplier) * 100);

  const onAccept = async () => {
    await acceptPolicy();
    await selectModel(cardId);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-background border border-mid-gray/20 rounded-lg p-6 max-w-md w-full mx-4 space-y-4">
        <h2 className="text-base font-semibold">{card?.name ?? cardId}</h2>
        {requiresAck && (
          <p className="text-sm text-text/70">
            {t(
              isCurrent
                ? "modelDataPolicy.introCurrent"
                : "modelDataPolicy.intro",
            )}
          </p>
        )}
        <div className="space-y-3 text-sm">
          <Row
            label={t("modelDataPolicy.destination")}
            value={card?.provider_label ?? ""}
          />

          {training && retention ? (
            <>
              <Row
                label={t("modelDataPolicy.trainingLabel")}
                value={t(training.key, training.values)}
              />
              <Row
                label={t("modelDataPolicy.retentionLabel")}
                value={t(retention.key, retention.values)}
              />
            </>
          ) : (
            <Row
              label={t("modelDataPolicy.trainingLabel")}
              value={t("modelDataPolicy.localOnly")}
            />
          )}
          {card && (
            <>
              <Row
                label={t("modelDataPolicy.languages")}
                value={
                  card.supported_languages.length > 0
                    ? getLanguageDisplayText(card.supported_languages, t)
                    : t("modelSelector.languageAutoOnly")
                }
              />
              <Row
                label={t("modelDataPolicy.translation")}
                value={
                  card.supports_translation
                    ? t("modelSelector.translationSupported")
                    : t("modelSelector.translationUnsupported")
                }
              />
              {usageMultiplier !== 1 && (
                <Row
                  label={t("modelDataPolicy.usage")}
                  value={t(
                    usageMultiplier < 1
                      ? "modelSelector.capabilities.usageDiscount"
                      : "modelSelector.capabilities.usageSurcharge",
                    { percent: usageDeltaPercent },
                  )}
                />
              )}
            </>
          )}
        </div>

        {upstream && isPolicyUnknown(upstream) && (
          <p className="text-xs text-text/60">
            {t("modelDataPolicy.unknownNote")}
          </p>
        )}
        {upstream && (
          <p className="text-xs text-text/60">
            {t("modelDataPolicy.seePolicy")}
          </p>
        )}
        <div className="flex gap-2 justify-end">
          <button
            className="px-3 py-1.5 text-sm rounded border border-mid-gray/20 hover:bg-mid-gray/10 transition-colors"
            onClick={isCurrent ? declineCurrentPolicy : dismissPolicy}
          >
            {requiresAck ? t("modelDataPolicy.cancel") : t("common.close")}
          </button>
          {requiresAck && (
            <button
              className="px-3 py-1.5 text-sm rounded bg-logo-primary text-white hover:opacity-90 transition-opacity"
              onClick={onAccept}
            >
              {t("modelDataPolicy.accept")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div className="text-xs text-text/60">{label}</div>
      <div>{value}</div>
    </div>
  );
}
