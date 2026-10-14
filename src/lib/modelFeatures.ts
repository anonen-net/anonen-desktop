export type ModelFeature = "translation" | "fillerReduction";

const ANONEN_CLOUD_PREFIX = "anonen-cloud:";

export interface FeatureCatalogRow {
  id: string;
  supports_filler_removal?: boolean | null;
}

export const modelFeatures = (
  model: { id: string; supports_translation: boolean },
  cloudModels: readonly FeatureCatalogRow[],
): ModelFeature[] => {
  const features: ModelFeature[] = [];

  if (model.supports_translation) features.push("translation");

  if (model.id.startsWith(ANONEN_CLOUD_PREFIX)) {
    const upstreamId = model.id.slice(ANONEN_CLOUD_PREFIX.length);
    if (cloudModels.find((m) => m.id === upstreamId)?.supports_filler_removal) {
      features.push("fillerReduction");
    }
  }
  return features;
};
