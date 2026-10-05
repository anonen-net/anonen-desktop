import type { ModelInfo } from "@/bindings";

export const isOfferedModel = (model: ModelInfo): boolean => {
  if (model.engine_type === "Whisper") {
    return model.id === "large" || model.is_custom;
  }
  return model.engine_type === "Remote";
};
