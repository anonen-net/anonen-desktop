import React, { useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { RefreshCw } from "lucide-react";
import type { ModelCardStatus } from "@/components/onboarding";
import { ModelCard } from "@/components/onboarding";
import { useModelStore } from "@/stores/modelStore";
import { useAnonenCloudStore } from "@/stores/anonenCloudStore";
import { useModelStatsStore } from "@/stores/modelStatsStore";
import type { ModelInfo } from "@/bindings";
import { compareModelsForDisplay } from "@/lib/utils/modelOrder";
import { playClickSound } from "@/lib/utils/clickSound";
import { isOfferedModel } from "@/lib/modelCatalog";

export const ModelsSettings: React.FC = () => {
  const { t } = useTranslation();
  const {
    models,
    currentModel,
    downloadingModels,
    downloadProgress,
    downloadStats,
    verifyingModels,
    extractingModels,
    loading,
    downloadModel,
    cancelDownload,
    selectModel,
    deleteModel,
    loadModels,
  } = useModelStore();
  const { fetchCloudModels } = useAnonenCloudStore();
  const [refreshingModels, setRefreshingModels] = useState(false);

  const modelStats = useModelStatsStore((s) => s.stats);
  const clearModelStats = useModelStatsStore((s) => s.clear);
  const getModelStatus = (modelId: string): ModelCardStatus => {
    if (modelId in extractingModels) {
      return "extracting";
    }
    if (modelId in verifyingModels) {
      return "verifying";
    }
    if (modelId in downloadingModels) {
      return "downloading";
    }
    if (modelId === currentModel) {
      return "active";
    }
    const model = models.find((m: ModelInfo) => m.id === modelId);
    if (model?.is_downloaded) {
      return "available";
    }
    return "downloadable";
  };

  const getDownloadProgress = (modelId: string): number | undefined => {
    const progress = downloadProgress[modelId];
    return progress?.percentage;
  };

  const getDownloadSpeed = (modelId: string): number | undefined => {
    const stats = downloadStats[modelId];
    return stats?.speed;
  };

  const handleModelSelect = async (modelId: string) => {
    playClickSound();
    await selectModel(modelId);
  };

  const handleModelDownload = async (modelId: string) => {
    await downloadModel(modelId);
  };

  const handleModelDelete = async (modelId: string) => {
    await deleteModel(modelId);
  };

  const handleModelCancel = async (modelId: string) => {
    try {
      await cancelDownload(modelId);
    } catch (err) {
      console.error(`Failed to cancel download for ${modelId}:`, err);
    }
  };

  const filteredModels = useMemo(() => models.filter(isOfferedModel), [models]);

  const { downloadedModels, availableModels } = useMemo(() => {
    const downloaded: ModelInfo[] = [];
    const available: ModelInfo[] = [];

    for (const model of filteredModels) {
      if (
        model.is_custom ||
        model.is_downloaded ||
        model.id in downloadingModels ||
        model.id in extractingModels
      ) {
        downloaded.push(model);
      } else {
        available.push(model);
      }
    }

    downloaded.sort(compareModelsForDisplay);
    available.sort(compareModelsForDisplay);

    return {
      downloadedModels: downloaded,
      availableModels: available,
    };
  }, [filteredModels, downloadingModels, extractingModels]);

  if (loading) {
    return (
      <div className="max-w-3xl w-full mx-auto">
        <div className="flex items-center justify-center py-16">
          <div className="w-8 h-8 border-2 border-logo-primary border-t-transparent rounded-full animate-spin" />
        </div>
      </div>
    );
  }

  const handleResetStats = () => {
    clearModelStats();
    toast.success(t("settings.models.statsReset"));
  };

  const handleRefreshModels = async () => {
    setRefreshingModels(true);
    try {
      await fetchCloudModels();
      await loadModels();
    } catch (e) {
      console.error("Model refresh failed:", e);
    } finally {
      setRefreshingModels(false);
    }
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-4">
      <div className="mb-4">
        <div className="flex items-start justify-between gap-3">
          <h1 className="text-xl font-semibold mb-2">
            {t("settings.models.title")}
          </h1>

          <button
            type="button"
            onClick={handleRefreshModels}
            disabled={refreshingModels}
            className="text-xs text-muted hover:text-text inline-flex items-center gap-1 disabled:opacity-50 shrink-0 mt-1"
          >
            <RefreshCw
              className={`w-3.5 h-3.5 ${refreshingModels ? "animate-spin" : ""}`}
            />
            {t("settings.models.refresh")}
          </button>

          {Object.keys(modelStats).length > 0 && (
            <button
              type="button"
              onClick={handleResetStats}
              className="text-xs text-muted hover:text-text shrink-0 mt-1"
            >
              {t("settings.models.resetStats")}
            </button>
          )}
        </div>
        <p className="text-sm text-muted">{t("settings.models.description")}</p>
      </div>
      {filteredModels.length > 0 ? (
        <div className="space-y-6">
          <div className="space-y-3">
            <h2 className="text-sm font-medium text-muted">
              {t("settings.models.yourModels")}
            </h2>
            {downloadedModels.map((model: ModelInfo) => (
              <ModelCard
                key={model.id}
                model={model}
                status={getModelStatus(model.id)}
                onSelect={handleModelSelect}
                onDownload={handleModelDownload}
                onCancel={handleModelCancel}
                onDelete={handleModelDelete}
                downloadProgress={getDownloadProgress(model.id)}
                downloadSpeed={getDownloadSpeed(model.id)}
                showRecommended={false}
                samples={modelStats[model.id]}
              />
            ))}
          </div>

          {availableModels.length > 0 && (
            <div className="space-y-3">
              <h2 className="text-sm font-medium text-muted">
                {t("settings.models.availableModels")}
              </h2>
              {availableModels.map((model: ModelInfo) => (
                <ModelCard
                  key={model.id}
                  model={model}
                  status={getModelStatus(model.id)}
                  onSelect={handleModelSelect}
                  onDownload={handleModelDownload}
                  onCancel={handleModelCancel}
                  downloadProgress={getDownloadProgress(model.id)}
                  downloadSpeed={getDownloadSpeed(model.id)}
                  showRecommended={false}
                  samples={modelStats[model.id]}
                />
              ))}
            </div>
          )}
        </div>
      ) : (
        <div className="text-center py-8 text-muted">
          {t("settings.models.noModelsMatch")}
        </div>
      )}
    </div>
  );
};
