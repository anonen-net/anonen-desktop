import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import i18n from "@/i18n";
import {
  commands,
  type AnonenCloudAuthStatus,
  type AnonenCloudUsageSnapshot,
  type AsrGatewayModel,
  type AsrGatewayUsage,
} from "@/bindings";
import {
  formatDuration,
  formatResetDateTimeOrNull,
  isLowRemaining,
  remainingS,
} from "@/lib/cloudUsage";
import {
  audioTooLongGuideKey,
  capRecordingKey,
  capToastKeys,
} from "@/lib/failureNotice";
import {
  knowsPolicyOf,
  policyFingerprint,
  policySubject,
  selectedCardNeedingDisclosure,
  type PolicySubject,
} from "@/lib/modelDataPolicy";
import type { AudioTooLongEvent } from "@/lib/types/events";
import { useSettingsStore } from "./settingsStore";
import { tutorialSeen } from "@/lib/tutorialSeen";

interface CapExceededPayload {
  which: string | null;
  resets_at: string | null;

  fallback: string | null;

  audio_saved: boolean;

  suspended: boolean;
}

interface CloudModelsUpdatedPayload {
  selection_missing: boolean;
  recommended_model_id: string | null;
}

interface AnonenCloudStore {
  authStatus: AnonenCloudAuthStatus;
  usage: AnonenCloudUsageSnapshot | null;
  lastCapExceeded: CapExceededPayload | null;
  initialized: boolean;

  initialize: () => Promise<void>;
  refreshAuthStatus: () => Promise<void>;
  refreshUsage: () => Promise<void>;
  fetchCloudModels: () => Promise<void>;

  cloudModels: AsrGatewayModel[];

  pendingPolicyCardId: string | null;

  pendingPolicyRequiresAck: boolean;

  pendingPolicyIsCurrent: boolean;

  policyNeedingDisclosure: (cardId: string) => PolicySubject | null;

  policySubjectOf: (cardId: string) => PolicySubject | null;

  knowsPolicyOf: (cardId: string) => boolean;
  showPolicy: (
    cardId: string,
    requiresAck: boolean,
    isCurrent?: boolean,
  ) => void;
  dismissPolicy: () => void;

  declineCurrentPolicy: () => Promise<void>;

  acceptPolicy: () => Promise<void>;
  requestOtp: (email: string) => Promise<void>;
  verifyOtp: (email: string, token: string) => Promise<void>;
  logout: () => Promise<void>;
  clearCapExceeded: () => void;
}

const ANONEN_CLOUD_PREFIX = "anonen-cloud:";

export const VALID_SUBSCRIPTION_STATUSES: ReadonlySet<string> = new Set([
  "trialing",
  "active",
  "past_due",
]);

export function isActiveSubscriptionStatus(
  status: string | null | undefined,
): boolean {
  return status != null && VALID_SUBSCRIPTION_STATUSES.has(status);
}

async function switchToRecommendedCloudModel(preferredId: string | null) {
  const { useModelStore } = await import("./modelStore");
  const store = useModelStore.getState();
  const target =
    (preferredId && store.models.some((m) => m.id === preferredId)
      ? preferredId
      : store.models.find(
          (m) => m.id.startsWith(ANONEN_CLOUD_PREFIX) && m.is_recommended,
        )?.id) ?? null;
  if (!target) {
    toast.error(i18n.t("anonenCloud.toast.noRecommendedModel"));
    return;
  }
  const name = store.getModelInfo(target)?.name ?? target;

  const ok = await store.selectModel(target);
  if (ok) {
    toast.message(i18n.t("anonenCloud.toast.switched", { target: name }));
  } else {
    toast.error(i18n.t("anonenCloud.toast.switchFailed", { target: name }));
  }
}

export function notifyCloudModelRetired(recommendedModelId: string | null) {
  toast.error(i18n.t("anonenCloud.toast.modelRetired"), {
    duration: 10000,
    action: {
      label: i18n.t("anonenCloud.toast.modelRetiredSwitch"),
      onClick: () => {
        switchToRecommendedCloudModel(recommendedModelId).catch((e) => {
          console.warn("Failed to switch to recommended model:", e);
          toast.error(
            i18n.t("anonenCloud.toast.switchFailed", {
              target: recommendedModelId ?? "?",
            }),
          );
        });
      },
    },
  });
}

export async function proposeDefaultCloudModel() {
  try {
    const { useModelStore } = await import("./modelStore");
    const store = useModelStore.getState();
    if (store.currentModel.startsWith(ANONEN_CLOUD_PREFIX)) {
      return;
    }
    const first = store.models.find((m) =>
      m.id.startsWith(ANONEN_CLOUD_PREFIX),
    );
    if (!first) {
      return;
    }
    const cloud = useAnonenCloudStore.getState();
    if (cloud.policyNeedingDisclosure(first.id)) {
      cloud.showPolicy(first.id, true);
    } else {
      await store.selectModel(first.id);
    }
  } catch {}
}

let initializeInFlight = false;
let fetchModelsInFlight: Promise<void> | null = null;

let lastLowRemaining: boolean | null = null;

function warnIfCrossedLowRemaining(usage: AsrGatewayUsage) {
  const isLow = isLowRemaining(usage);
  const wasLow = lastLowRemaining;
  lastLowRemaining = isLow;
  if (wasLow === null || wasLow || !isLow) return;
  toast.warning(i18n.t("anonenCloud.toast.lowRemaining"), {
    description: i18n.t("anonenCloud.toast.lowRemainingDetail", {
      remaining: formatDuration(remainingS(usage)),
    }),
    duration: 8000,
  });
}

export const useAnonenCloudStore = create<AnonenCloudStore>((set, get) => ({
  authStatus: { signed_in: false, email: null, user_id: null },
  usage: null,
  lastCapExceeded: null,
  initialized: false,
  cloudModels: [],
  pendingPolicyCardId: null,
  pendingPolicyRequiresAck: false,
  pendingPolicyIsCurrent: false,

  initialize: async () => {
    if (get().initialized || initializeInFlight) return;
    initializeInFlight = true;
    try {
      listen<AnonenCloudUsageSnapshot>("anonen-cloud-usage-update", (event) => {
        set({ usage: event.payload, lastCapExceeded: null });
        warnIfCrossedLowRemaining(event.payload.usage);
      });

      registerFailureListeners();

      listen<CloudModelsUpdatedPayload>(
        "anonen-cloud-models-updated",
        async (event) => {
          try {
            await get().fetchCloudModels();
            const { useModelStore } = await import("./modelStore");
            await useModelStore.getState().loadCurrentModel();
          } catch {}
          if (event.payload.selection_missing) {
            notifyCloudModelRetired(event.payload.recommended_model_id);
          }
        },
      );

      const [authResult, usageResult] = await Promise.all([
        commands.anonenCloudAuthStatus(),
        commands.anonenCloudCurrentUsage(),
      ]);
      if (authResult.status === "ok") {
        set({ authStatus: authResult.data, initialized: true });
      } else {
        set({ initialized: true });
      }
      if (usageResult.status === "ok" && usageResult.data) {
        set({ usage: usageResult.data });
      }

      const signedIn = authResult.status === "ok" && authResult.data.signed_in;

      if (signedIn && !get().usage) {
        try {
          const fresh = await commands.anonenCloudFetchUsage();
          if (fresh.status === "ok") {
            set({ usage: fresh.data, lastCapExceeded: null });
          }
        } catch {}
      }

      if (signedIn) {
        await get().fetchCloudModels();
      }

      if (signedIn) {
        try {
          const { useModelStore } = await import("./modelStore");

          await useModelStore.getState().loadCurrentModel();
          const store = useModelStore.getState();
          const currentIsValid =
            store.currentModel !== "" &&
            store.models.some((m) => m.id === store.currentModel);
          if (!currentIsValid) {
            if (store.currentModel.startsWith(ANONEN_CLOUD_PREFIX)) {
              const catalogKnown = store.models.some((m) =>
                m.id.startsWith(ANONEN_CLOUD_PREFIX),
              );
              if (catalogKnown) {
                notifyCloudModelRetired(null);
              }
            } else {
              await proposeDefaultCloudModel();
            }
          }
        } catch {}
      }
    } finally {
      initializeInFlight = false;
    }
  },

  refreshAuthStatus: async () => {
    const result = await commands.anonenCloudAuthStatus();
    if (result.status === "ok") {
      set({ authStatus: result.data });
    }
  },

  refreshUsage: async () => {
    const [usageResult] = await Promise.all([
      commands.anonenCloudFetchUsage(),
      get().fetchCloudModels(),
    ]);
    if (usageResult.status === "ok") {
      set({ usage: usageResult.data, lastCapExceeded: null });
    } else {
      throw new Error(usageResult.error);
    }
  },

  fetchCloudModels: async () => {
    if (fetchModelsInFlight) {
      await fetchModelsInFlight;
      return;
    }
    const task = (async () => {
      const result = await commands.anonenCloudFetchModels();
      if (result.status === "ok") {
        set({ cloudModels: result.data });
        try {
          const { useModelStore } = await import("./modelStore");
          await useModelStore.getState().loadModels();

          const cloud = get();
          if (cloud.pendingPolicyCardId === null) {
            const current = selectedCardNeedingDisclosure(
              useModelStore.getState().currentModel,
              cloud.policyNeedingDisclosure,
            );
            if (current !== null) cloud.showPolicy(current, true, true);
          }
        } catch {}
      }
    })();
    fetchModelsInFlight = task;
    try {
      await task;
    } finally {
      fetchModelsInFlight = null;
    }
  },

  policySubjectOf: (cardId) => policySubject(cardId, get().cloudModels),
  knowsPolicyOf: (cardId) => knowsPolicyOf(cardId, get().cloudModels),

  policyNeedingDisclosure: (cardId) => {
    const subject = get().policySubjectOf(cardId);
    if (!subject) return null;
    const acknowledged =
      useSettingsStore.getState().settings?.acknowledged_cloud_policies ?? [];
    return acknowledged.includes(policyFingerprint(subject)) ? null : subject;
  },

  showPolicy: (cardId, requiresAck, isCurrent = false) =>
    set({
      pendingPolicyCardId: cardId,
      pendingPolicyRequiresAck: requiresAck,
      pendingPolicyIsCurrent: isCurrent,
    }),
  dismissPolicy: () =>
    set({
      pendingPolicyCardId: null,
      pendingPolicyRequiresAck: false,
      pendingPolicyIsCurrent: false,
    }),
  declineCurrentPolicy: async () => {
    const cardId = get().pendingPolicyCardId;
    get().dismissPolicy();
    const { useModelStore } = await import("./modelStore");

    if (cardId === null || useModelStore.getState().currentModel !== cardId)
      return;
    const result = await commands.clearActiveModelCommand();
    if (result.status === "ok") {
      useModelStore.setState({ currentModel: "" });
    } else {
      console.error("Failed to clear the model selection:", result.error);
    }
  },

  acceptPolicy: async () => {
    const cardId = get().pendingPolicyCardId;
    const subject = cardId ? get().policySubjectOf(cardId) : null;
    set({
      pendingPolicyCardId: null,
      pendingPolicyRequiresAck: false,
      pendingPolicyIsCurrent: false,
    });
    if (!subject) return;
    const result = await commands.acknowledgeModelDataPolicy(
      policyFingerprint(subject),
    );
    if (result.status === "error") {
      console.error("Failed to record data policy disclosure:", result.error);
      return;
    }
    await useSettingsStore.getState().refreshSettings();
  },

  requestOtp: async (email) => {
    const result = await commands.anonenCloudRequestOtp(email, null);
    if (result.status !== "ok") {
      throw new Error(result.error);
    }
  },

  verifyOtp: async (email, token) => {
    const result = await commands.anonenCloudVerifyOtp(email, token);
    if (result.status !== "ok") {
      throw new Error(result.error);
    }
    set({ authStatus: result.data });
    await Promise.all([
      get().fetchCloudModels(),
      commands.anonenCloudFetchUsage().then((r) => {
        if (r.status === "ok") set({ usage: r.data, lastCapExceeded: null });
      }),
    ]).catch(() => {});

    if (tutorialSeen()) {
      await proposeDefaultCloudModel();
    }
  },

  logout: async () => {
    const result = await commands.anonenCloudLogout();
    if (result.status === "ok") {
      lastLowRemaining = null;
      set({ authStatus: result.data, usage: null, lastCapExceeded: null });
    } else {
      console.error("Logout failed:", result.error);
      toast.error(i18n.t("anonenCloud.toast.logoutFailed"));
    }
  },

  clearCapExceeded: () => set({ lastCapExceeded: null }),
}));

let failureListenersRegistered = false;

let subscriptionToastOn = false;

export function setSubscriptionToastOn(on: boolean) {
  subscriptionToastOn = on;
}

export function registerFailureListeners() {
  if (failureListenersRegistered) return;
  failureListenersRegistered = true;

  listen("anonen-cloud-checkout-required", () => {
    if (!subscriptionToastOn) return;
    toast.error(i18n.t("anonenCloud.toast.subscriptionRequired"), {
      description: i18n.t("anonenCloud.toast.subscriptionGuide"),
      duration: 8000,
    });
  });

  listen<CapExceededPayload>("anonen-cloud-cap-exceeded", (event) => {
    const payload = event.payload;
    useAnonenCloudStore.setState({ lastCapExceeded: payload });
    const which = i18n.t(
      `anonenCloud.toast.capWindow.${payload.which ?? "unknown"}`,
      { defaultValue: i18n.t("anonenCloud.toast.capWindow.unknown") },
    );

    const when = payload.resets_at
      ? formatResetDateTimeOrNull(payload.resets_at, i18n.language)
      : null;
    const keys = capToastKeys(payload.suspended, when !== null);
    const lines = [
      when !== null ? i18n.t(keys.timing, { when }) : i18n.t(keys.timing),
    ];
    if (payload.fallback === "local") {
      lines.push(i18n.t("anonenCloud.toast.capLocalGuide"));
    }
    lines.push(i18n.t(capRecordingKey(payload.audio_saved)));
    toast.error(i18n.t(keys.title, { which }), {
      description: lines.join("\n"),
      duration: 10000,
    });
  });

  listen<AudioTooLongEvent>("anonen-cloud-audio-too-long", (event) => {
    toast.error(i18n.t("anonenCloud.toast.audioTooLong"), {
      description: i18n.t(audioTooLongGuideKey(event.payload.audio_saved)),
      duration: 10000,
    });
  });

  listen("anonen-cloud-model-invalid", () => {
    notifyCloudModelRetired(null);
    useAnonenCloudStore.getState().fetchCloudModels();
  });
}
