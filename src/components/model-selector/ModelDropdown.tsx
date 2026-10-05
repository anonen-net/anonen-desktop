import React from "react";
import { useTranslation } from "react-i18next";
import type { ModelInfo } from "@/bindings";
import { getTranslatedModelName } from "../../lib/utils/modelTranslation";
import { compareModelsForDisplay } from "../../lib/utils/modelOrder";

interface ModelDropdownProps {
  models: ModelInfo[];
  currentModelId: string;
  onModelSelect: (modelId: string) => void;
}

const ModelDropdown: React.FC<ModelDropdownProps> = ({
  models,
  currentModelId,
  onModelSelect,
}) => {
  const { t } = useTranslation();

  const downloadedModels = models
    .filter((m) => m.is_downloaded)
    .sort(compareModelsForDisplay);

  const handleModelClick = (modelId: string) => {
    onModelSelect(modelId);
  };

  return (
    <div className="absolute bottom-full start-0 mb-2 w-64 max-h-[60vh] overflow-y-auto bg-background border border-mid-gray/20 rounded-lg shadow-lg py-2 z-50">
      {downloadedModels.length > 0 ? (
        <div>
          {downloadedModels.map((model) => (
            <div
              key={model.id}
              onClick={() => handleModelClick(model.id)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  handleModelClick(model.id);
                }
              }}
              tabIndex={0}
              role="button"
              className={`w-full px-3 py-2 text-start hover:bg-mid-gray/10 transition-colors cursor-pointer focus:outline-none ${
                currentModelId === model.id
                  ? "bg-logo-primary/10 text-logo-primary"
                  : ""
              }`}
            >
              <div className="flex items-center justify-between gap-2">
                <div className="min-w-0">
                  <div className="text-sm text-text/80">
                    {getTranslatedModelName(model, t)}
                    {model.engine_type !== "Remote" && (
                      <span className="ms-1.5 text-[10px] font-medium text-muted/80 uppercase">
                        {t("modelSelector.localModel")}
                      </span>
                    )}
                    {model.is_custom && (
                      <span className="ms-1.5 text-[10px] font-medium text-muted/80 uppercase">
                        {t("modelSelector.custom")}
                      </span>
                    )}
                  </div>
                </div>
                {currentModelId === model.id && (
                  <div className="text-xs text-logo-primary flex-shrink-0 whitespace-nowrap">
                    {t("modelSelector.active")}
                  </div>
                )}
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div className="px-3 py-2 text-sm text-muted">
          {t("modelSelector.noModelsAvailable")}
        </div>
      )}
    </div>
  );
};

export default ModelDropdown;
