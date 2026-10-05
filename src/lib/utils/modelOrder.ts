import type { ModelInfo } from "@/bindings";

const ANONEN_CLOUD_PREFIX = "anonen-cloud:";

const isCloud = (id: string) => id.startsWith(ANONEN_CLOUD_PREFIX);

export function compareModelsForDisplay(a: ModelInfo, b: ModelInfo): number {
  const aCloud = isCloud(a.id);
  const bCloud = isCloud(b.id);
  if (aCloud !== bCloud) return aCloud ? -1 : 1;

  if (a.is_custom !== b.is_custom) return a.is_custom ? 1 : -1;
  return 0;
}
