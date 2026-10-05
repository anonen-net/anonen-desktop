import React, { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import type { ModelInfo } from "@/bindings";
import type { ModelCardStatus } from "./ModelCard";
import ModelCard from "./ModelCard";
import { useModelStore } from "../../stores/modelStore";
import { useAnonenCloudStore } from "../../stores/anonenCloudStore";
import { isOfferedModel } from "@/lib/modelCatalog";
import { ask } from "@tauri-apps/plugin-dialog";

interface OnboardingProps {
  onModelSelected: () => void;

  onSkip: () => void;
}

const Onboarding: React.FC<OnboardingProps> = ({ onModelSelected, onSkip }) => {
  const { t } = useTranslation();
  const {
    models,
    currentModel,
    downloadModel,
    selectModel,
    downloadingModels,
    verifyingModels,
    extractingModels,
    downloadProgress,
    downloadStats,
  } = useModelStore();
  const [selectedModelId, setSelectedModelId] = useState<string | null>(null);

  const localSelectRef = useRef<string | null>(null);
  const [selectingLocal, setSelectingLocal] = useState<string | null>(null);

  const activeRef = useRef(true);
  useEffect(() => {
    activeRef.current = true;
    return () => {
      activeRef.current = false;
    };
  }, []);

  const isDownloading = selectedModelId !== null;

  const isSwitching = selectingLocal !== null;

  const cloudModels = models.filter((m) => m.engine_type === "Remote");
  const localModels = models.filter(
    (m) => m.engine_type !== "Remote" && isOfferedModel(m),
  );
  const hasCloudModels = cloudModels.length > 0;

  const onModelSelectedRef = useRef(onModelSelected);
  onModelSelectedRef.current = onModelSelected;
  useEffect(() => {
    if (currentModel === "" || localSelectRef.current !== null) return;
    onModelSelectedRef.current();
  }, [currentModel]);

  useEffect(() => {
    if (hasCloudModels) return;
    useAnonenCloudStore
      .getState()
      .fetchCloudModels()
      .catch((e) => console.warn("Failed to fetch cloud models:", e));
  }, []);

  const handleSelectCloudModel = async (modelId: string) => {
    const ok = await selectModel(modelId);
    if (!ok && useAnonenCloudStore.getState().pendingPolicyCardId === null) {
      toast.error(t("onboarding.errors.selectModel"));
    }
  };

  const selectLocalModel = useCallback(
    async (modelId: string) => {
      if (localSelectRef.current !== null) return;
      localSelectRef.current = modelId;
      setSelectingLocal(modelId);
      const success = await selectModel(modelId);

      if (!activeRef.current) return;
      if (success) {
        onModelSelectedRef.current();
      } else {
        localSelectRef.current = null;
        setSelectingLocal(null);
        toast.error(t("onboarding.errors.selectModel"));
        setSelectedModelId(null);
      }
    },
    [selectModel, t],
  );

  useEffect(() => {
    if (!selectedModelId) return;

    const model = models.find((m) => m.id === selectedModelId);
    const stillDownloading = selectedModelId in downloadingModels;
    const stillVerifying = selectedModelId in verifyingModels;
    const stillExtracting = selectedModelId in extractingModels;

    if (
      model?.is_downloaded &&
      !stillDownloading &&
      !stillVerifying &&
      !stillExtracting
    ) {
      selectLocalModel(selectedModelId);
    }
  }, [
    selectedModelId,
    models,
    downloadingModels,
    verifyingModels,
    extractingModels,
    selectLocalModel,
  ]);

  const handleDownloadModel = async (modelId: string) => {
    const model = models.find((m) => m.id === modelId);
    if (model && model.engine_type !== "Remote") {
      const size =
        model.size_mb >= 1024
          ? `${(model.size_mb / 1024).toFixed(1)} GB`
          : `${Math.round(model.size_mb)} MB`;
      const ok = await ask(
        t("onboarding.downloadConfirm.body", { name: model.name, size }),
        {
          title: t("onboarding.downloadConfirm.title"),
          kind: "info",
          okLabel: t("onboarding.downloadConfirm.ok"),
          cancelLabel: t("onboarding.downloadConfirm.cancel"),
        },
      );
      if (!ok) return;
    }
    setSelectedModelId(modelId);

    const success = await downloadModel(modelId);
    if (!success) {
      setSelectedModelId(null);
    }
  };

  const getModelStatus = (model: ModelInfo): ModelCardStatus => {
    if (model.id === selectingLocal) return "switching";
    if (model.id in extractingModels) return "extracting";
    if (model.id in verifyingModels) return "verifying";
    if (model.id in downloadingModels) return "downloading";
    return model.is_downloaded ? "available" : "downloadable";
  };

  const getModelDownloadProgress = (modelId: string): number | undefined => {
    return downloadProgress[modelId]?.percentage;
  };

  const getModelDownloadSpeed = (modelId: string): number | undefined => {
    return downloadStats[modelId]?.speed;
  };

  return (
    <div className="h-screen w-screen flex flex-col p-6 gap-4 inset-0">
      <div className="flex flex-col items-center gap-2 shrink-0">
        <h1 className="text-4xl font-bold">{t("app.name")}</h1>
        <p className="text-text/70 max-w-md font-medium mx-auto">
          {t("onboarding.subtitle")}
        </p>
        <button
          type="button"
          onClick={onSkip}
          className="px-4 py-1.5 rounded-lg border-2 border-border text-sm font-medium text-muted hover:text-text transition-colors"
        >
          {t("onboarding.chooseLater")}
        </button>
      </div>

      <div className="max-w-[600px] w-full mx-auto text-center flex-1 flex flex-col min-h-0">
        <div className="flex flex-col gap-4 pb-6">
          {cloudModels.map((model: ModelInfo, index: number) => (
            <ModelCard
              key={model.id}
              model={model}
              variant={index === 0 ? "featured" : "default"}
              status="available"
              disabled={isSwitching}
              onSelect={handleSelectCloudModel}
              showRecommended={false}
            />
          ))}

          {localModels
            .filter((model: ModelInfo) => model.is_recommended)
            .map((model: ModelInfo) => (
              <ModelCard
                key={model.id}
                model={model}
                variant={hasCloudModels ? "default" : "featured"}
                status={getModelStatus(model)}
                disabled={isDownloading || isSwitching}
                onSelect={selectLocalModel}
                onDownload={handleDownloadModel}
                showRecommended={false}
                downloadProgress={getModelDownloadProgress(model.id)}
                downloadSpeed={getModelDownloadSpeed(model.id)}
              />
            ))}

          {localModels
            .filter((model: ModelInfo) => !model.is_recommended)
            .sort(
              (a: ModelInfo, b: ModelInfo) =>
                Number(a.size_mb) - Number(b.size_mb),
            )
            .map((model: ModelInfo) => (
              <ModelCard
                key={model.id}
                model={model}
                status={getModelStatus(model)}
                disabled={isDownloading || isSwitching}
                onSelect={selectLocalModel}
                onDownload={handleDownloadModel}
                showRecommended={false}
                downloadProgress={getModelDownloadProgress(model.id)}
                downloadSpeed={getModelDownloadSpeed(model.id)}
              />
            ))}
        </div>
      </div>
    </div>
  );
};

export default Onboarding;
