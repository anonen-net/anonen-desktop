import type { AsrGatewayModel } from "@/bindings";

export function policyFingerprint(model: {
  id: string;
  training_use?: string | null;
  retention_kind?: string | null;
  retention_days?: number | null;
}): string {
  const training = model.training_use || "unknown";
  const retention = model.retention_kind || "unknown";
  const days =
    model.retention_days === null || model.retention_days === undefined
      ? "-"
      : String(model.retention_days);
  return `${model.id}@${training}/${retention}/${days}`;
}

export type PolicySubject = Pick<
  AsrGatewayModel,
  "id" | "training_use" | "retention_kind" | "retention_days"
>;

const ANONEN_CLOUD_PREFIX = "anonen-cloud:";

export function policySubject(
  cardId: string,
  catalog: readonly PolicySubject[],
): PolicySubject | null {
  if (!cardId.startsWith(ANONEN_CLOUD_PREFIX)) return null;
  const rawId = cardId.slice(ANONEN_CLOUD_PREFIX.length);
  return catalog.find((model) => model.id === rawId) ?? { id: rawId };
}

export function knowsPolicyOf(
  cardId: string,
  catalog: readonly PolicySubject[],
): boolean {
  const subject = policySubject(cardId, catalog);
  return subject === null || catalog.includes(subject);
}

export interface PolicyLine {
  key: string;
  values?: Record<string, unknown>;
}

export function trainingLine(
  model: Pick<AsrGatewayModel, "training_use">,
): PolicyLine {
  switch (model.training_use) {
    case "no":
      return { key: "modelDataPolicy.training.no" };

    case "opt_out_applied":
      return { key: "modelDataPolicy.training.optOut" };
    case "yes":
      return { key: "modelDataPolicy.training.yes" };
    default:
      return { key: "modelDataPolicy.unknown" };
  }
}

export function retentionLine(
  model: Pick<AsrGatewayModel, "retention_kind" | "retention_days">,
): PolicyLine {
  switch (model.retention_kind) {
    case "none":
      return { key: "modelDataPolicy.retention.none" };
    case "days":
      return model.retention_days
        ? {
            key: "modelDataPolicy.retention.days",
            values: { days: model.retention_days },
          }
        : { key: "modelDataPolicy.unknown" };

    case "unspecified":
      return { key: "modelDataPolicy.retention.unspecified" };
    default:
      return { key: "modelDataPolicy.unknown" };
  }
}

export function isPolicyUnknown(model: PolicySubject): boolean {
  return (
    trainingLine(model).key === "modelDataPolicy.unknown" &&
    retentionLine(model).key === "modelDataPolicy.unknown"
  );
}

export function selectedCardNeedingDisclosure(
  currentModel: string,
  needingDisclosure: (cardId: string) => unknown,
): string | null {
  if (!currentModel.startsWith("anonen-cloud:")) return null;
  return needingDisclosure(currentModel) ? currentModel : null;
}
