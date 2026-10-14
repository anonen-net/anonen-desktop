import React from "react";
import { useTranslation } from "react-i18next";
import {
  Check,
  Cloud,
  Download,
  FlaskConical,
  Globe,
  HardDrive,
  Laptop,
  Loader2,
  Trash2,
} from "lucide-react";
import type { ModelInfo } from "@/bindings";
import {
  useModelStatsStore,
  type StatSample,
} from "../../stores/modelStatsStore";
import { useModelStore } from "../../stores/modelStore";
import {
  blendedSpeedScores,
  medianMs,
  perAudioSecond,
} from "../../lib/speedBlend";
import { formatModelSize } from "../../lib/utils/format";
import {
  getTranslatedModelDescription,
  getTranslatedModelName,
} from "../../lib/utils/modelTranslation";
import { getLanguageDisplayText } from "../../lib/utils/languageDisplay";
import Badge from "../ui/Badge";
import { Button } from "../ui/Button";
import { useAnonenCloudStore } from "../../stores/anonenCloudStore";
import { isPreview, policySubject } from "../../lib/modelDataPolicy";
import { modelFeatures } from "../../lib/modelFeatures";
import { ModelFeatureBadge } from "../ui/ModelFeatureBadge";

export type ModelCardStatus =
  | "downloadable"
  | "downloading"
  | "verifying"
  | "extracting"
  | "switching"
  | "active"
  | "available";

interface ModelCardProps {
  model: ModelInfo;
  variant?: "default" | "featured";
  status?: ModelCardStatus;
  disabled?: boolean;
  className?: string;
  onSelect: (modelId: string) => void;
  onDownload?: (modelId: string) => void;
  onDelete?: (modelId: string) => void;
  onCancel?: (modelId: string) => void;
  downloadProgress?: number;
  downloadSpeed?: number;
  showRecommended?: boolean;

  samples?: StatSample[];
}

const ModelCard: React.FC<ModelCardProps> = ({
  model,
  variant = "default",
  status = "downloadable",
  disabled = false,
  className = "",
  onSelect,
  onDownload,
  onDelete,
  onCancel,
  downloadProgress,
  downloadSpeed,
  showRecommended = true,
  samples,
}) => {
  const { t } = useTranslation();
  const isFeatured = variant === "featured";

  const isClickable = status === "available" || status === "downloadable";

  const measuredMedianMs = samples ? medianMs(samples.map((s) => s.ms)) : null;

  const allStats = useModelStatsStore((s) => s.stats);
  const allModels = useModelStore((s) => s.models);
  const blendedSpeed = React.useMemo(() => {
    const server: Record<string, number> = {};
    for (const m of allModels) {
      if (m.engine_type === "Remote" && m.id.startsWith("anonen-cloud:")) {
        server[m.id] = m.speed_score;
      }
    }

    const perSecond: Record<string, number[]> = {};
    for (const [id, samples] of Object.entries(allStats)) {
      perSecond[id] = perAudioSecond(samples);
    }
    return { scores: blendedSpeedScores(perSecond, server), perSecond };
  }, [allStats, allModels]);
  const speedScore = blendedSpeed.scores[model.id] ?? model.speed_score;

  const speedIsMeasured =
    blendedSpeed.scores[model.id] !== undefined &&
    (blendedSpeed.perSecond[model.id]?.length ?? 0) > 0;

  const cloudModels = useAnonenCloudStore((s) => s.cloudModels);
  const preview = isPreview(policySubject(model.id, cloudModels));
  const features = modelFeatures(model, cloudModels);

  const displayName = getTranslatedModelName(model, t);
  const displayDescription = getTranslatedModelDescription(model, t);
  const showModelSize =
    status === "downloadable" || status === "available" || status === "active";
  const formattedModelSize = formatModelSize(Number(model.size_mb));

  const usageMultiplier = model.usage_multiplier ?? 1;
  const usageDeltaPercent = Math.round(Math.abs(1 - usageMultiplier) * 100);

  const baseClasses =
    "flex flex-col rounded-md px-4 py-3 gap-2 text-left transition-all duration-200";

  const getVariantClasses = () => {
    if (status === "active") {
      return "border-2 border-logo-primary bg-logo-primary/10 shadow-hard-xs";
    }
    if (isFeatured) {
      return "border-2 border-logo-primary/60 bg-logo-primary/5";
    }
    return "border-2 border-border bg-surface";
  };

  const getInteractiveClasses = () => {
    if (!isClickable) return "";
    if (disabled) return "opacity-50 cursor-not-allowed";
    return "cursor-pointer hover:border-logo-primary hover:bg-logo-primary/5 hover:shadow-hard-xs group";
  };

  const handleClick = () => {
    if (!isClickable || disabled) return;
    if (status === "downloadable" && onDownload) {
      onDownload(model.id);
    } else {
      onSelect(model.id);
    }
  };

  const handleDelete = (e: React.MouseEvent) => {
    e.stopPropagation();
    onDelete?.(model.id);
  };

  return (
    <div
      onClick={handleClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" && isClickable) handleClick();
      }}
      role={isClickable ? "button" : undefined}
      tabIndex={isClickable ? 0 : undefined}
      className={[
        baseClasses,
        getVariantClasses(),
        getInteractiveClasses(),
        className,
      ]
        .filter(Boolean)
        .join(" ")}
    >
      <div className="flex justify-between items-center w-full">
        <div className="flex flex-col items-start flex-1 min-w-0">
          <div className="flex items-center gap-1.5 min-w-0 max-w-full">
            <h3
              title={displayName}
              className={`min-w-0 truncate text-base font-semibold text-text ${isClickable ? "group-hover:text-logo-primary" : ""} transition-colors`}
            >
              {displayName}
            </h3>

            {preview && (
              <Badge variant="preview" size="sm">
                <FlaskConical
                  className="w-2.5 h-2.5 me-0.5"
                  aria-hidden="true"
                />
                {t("modelSelector.preview")}
              </Badge>
            )}
            {showRecommended && model.is_recommended && (
              <Badge variant="primary" size="sm">
                {t("onboarding.recommended")}
              </Badge>
            )}

            {model.engine_type !== "Remote" && (
              <Badge variant="primary" size="sm">
                {t("modelSelector.localModel")}
              </Badge>
            )}
            {model.is_custom && (
              <Badge variant="secondary" size="sm">
                {t("modelSelector.custom")}
              </Badge>
            )}

            {features.map((feature) => (
              <ModelFeatureBadge key={feature} feature={feature} size="sm" />
            ))}
            {status === "switching" && (
              <Badge variant="secondary" size="sm">
                <Loader2 className="w-2.5 h-2.5 me-0.5 animate-spin" />
                {t("modelSelector.switching")}
              </Badge>
            )}
          </div>
          {displayDescription && (
            <p className="text-muted text-sm leading-relaxed">
              {displayDescription}
            </p>
          )}
        </div>
        {(model.accuracy_score > 0 ||
          model.speed_score > 0 ||
          measuredMedianMs !== null) && (
          <div className="hidden sm:flex items-center shrink-0 ms-3">
            <div className="space-y-1">
              {(model.accuracy_score > 0 || model.speed_score > 0) && (
                <>
                  <div className="flex items-center justify-end gap-2">
                    <p className="text-xs text-muted text-end whitespace-nowrap">
                      {t("onboarding.modelCard.accuracy")}
                    </p>
                    <div className="w-14 h-1.5 bg-mid-gray/20 rounded-full overflow-hidden">
                      <div
                        className="h-full bg-logo-primary rounded-full"
                        style={{ width: `${model.accuracy_score * 100}%` }}
                      />
                    </div>
                  </div>

                  {model.engine_type === "Remote" && (
                    <div className="flex items-center justify-end gap-2">
                      <p className="text-xs text-muted text-end whitespace-nowrap">
                        {t("onboarding.modelCard.speed")}
                        {speedIsMeasured && (
                          <span
                            className="ms-1"
                            title={t("onboarding.modelCard.speedFromMeasured")}
                          >
                            *
                          </span>
                        )}
                      </p>
                      <div className="w-14 h-1.5 bg-mid-gray/20 rounded-full overflow-hidden">
                        <div
                          className="h-full bg-logo-primary rounded-full"
                          style={{ width: `${speedScore * 100}%` }}
                        />
                      </div>
                    </div>
                  )}
                </>
              )}

              {measuredMedianMs !== null && samples && (
                <p className="text-xs text-logo-primary text-end tabular-nums">
                  {t("onboarding.modelCard.measured", {
                    seconds: (measuredMedianMs / 1000).toFixed(1),
                    count: samples.length,
                  })}
                </p>
              )}
            </div>
          </div>
        )}
      </div>

      <hr className="w-full border-border/20" />

      <div className="flex items-center gap-3 w-full -mb-0.5 mt-0.5 h-5">
        {model.provider_label && (
          <div className="flex items-center gap-1 text-xs text-muted min-w-0">
            {model.engine_type === "Remote" ? (
              <Cloud className="w-3.5 h-3.5 shrink-0" />
            ) : (
              <Laptop className="w-3.5 h-3.5 shrink-0" />
            )}
            <span className="truncate">{model.provider_label}</span>
          </div>
        )}
        {model.supported_languages.length > 0 && (
          <div
            className="flex items-center gap-1 text-xs text-muted shrink-0"
            title={
              model.supported_languages.length === 1
                ? t("modelSelector.capabilities.singleLanguage")
                : t("modelSelector.capabilities.languageSelection")
            }
          >
            <Globe className="w-3.5 h-3.5" />
            <span>{getLanguageDisplayText(model.supported_languages, t)}</span>
          </div>
        )}

        <div className="ms-auto flex items-center gap-3 shrink-0">
          {usageMultiplier !== 1 && (
            <Badge
              variant={usageMultiplier < 1 ? "discount" : "surcharge"}
              className="shrink-0 px-2 py-0 tabular-nums"
              title={t(
                usageMultiplier < 1
                  ? "modelSelector.capabilities.usageDiscount"
                  : "modelSelector.capabilities.usageSurcharge",
                { percent: usageDeltaPercent },
              )}
            >
              {t(
                usageMultiplier < 1
                  ? "modelSelector.usageDiscountShort"
                  : "modelSelector.usageSurchargeShort",
                { percent: usageDeltaPercent },
              )}
            </Badge>
          )}
          {showModelSize && model.size_mb > 0 && (
            <span className="flex items-center gap-1.5 text-xs text-muted">
              {status === "downloadable" ? (
                <Download className="w-3.5 h-3.5" />
              ) : (
                <HardDrive className="w-3.5 h-3.5" />
              )}
              <span>{formattedModelSize}</span>
            </span>
          )}

          {onDelete &&
            model.engine_type !== "Remote" &&
            (status === "available" || status === "active") && (
              <Button
                variant="ghost"
                size="sm"
                onClick={handleDelete}
                title={t("modelSelector.deleteModel", {
                  modelName: displayName,
                })}
                className="flex items-center gap-1.5 text-logo-primary/85 hover:text-logo-primary hover:bg-logo-primary/10"
              >
                <Trash2 className="w-3.5 h-3.5" />
                <span>{t("common.delete")}</span>
              </Button>
            )}

          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              useAnonenCloudStore.getState().showPolicy(model.id, false);
            }}
            className="text-xs text-muted hover:text-text transition-colors"
          >
            {t("modelSelector.details")}
          </button>
        </div>
      </div>

      {status === "downloading" && downloadProgress !== undefined && (
        <div className="w-full mt-3">
          <div className="w-full h-1.5 bg-mid-gray/20 rounded-full overflow-hidden">
            <div
              className="h-full bg-logo-primary rounded-full transition-all duration-300"
              style={{ width: `${downloadProgress}%` }}
            />
          </div>
          <div className="flex items-center justify-between text-xs mt-1">
            <span className="text-muted">
              {t("modelSelector.downloading", {
                percentage: Math.round(downloadProgress),
              })}
            </span>
            <div className="flex items-center gap-2">
              {downloadSpeed !== undefined && downloadSpeed > 0 && (
                <span className="tabular-nums text-muted">
                  {t("modelSelector.downloadSpeed", {
                    speed: downloadSpeed.toFixed(1),
                  })}
                </span>
              )}
              {onCancel && (
                <Button
                  variant="danger-ghost"
                  size="sm"
                  onClick={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    onCancel(model.id);
                  }}
                  aria-label={t("modelSelector.cancelDownload")}
                >
                  {t("modelSelector.cancel")}
                </Button>
              )}
            </div>
          </div>
        </div>
      )}
      {status === "verifying" && (
        <div className="w-full mt-3">
          <div className="w-full h-1.5 bg-mid-gray/20 rounded-full overflow-hidden">
            <div className="h-full bg-logo-primary rounded-full animate-pulse w-full" />
          </div>
          <p className="text-xs text-muted mt-1">
            {t("modelSelector.verifyingGeneric")}
          </p>
        </div>
      )}
      {status === "extracting" && (
        <div className="w-full mt-3">
          <div className="w-full h-1.5 bg-mid-gray/20 rounded-full overflow-hidden">
            <div className="h-full bg-logo-primary rounded-full animate-pulse w-full" />
          </div>
          <p className="text-xs text-muted mt-1">
            {t("modelSelector.extractingGeneric")}
          </p>
        </div>
      )}
    </div>
  );
};

export default ModelCard;
