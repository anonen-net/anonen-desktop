import { create } from "zustand";
import { persist } from "zustand/middleware";
import { listen } from "@tauri-apps/api/event";

const KEEP = 20;

const ANONEN_CLOUD_PREFIX = "anonen-cloud:";

interface TranscriptionStatsPayload {
  model_id: string;
  asr_ms: number;
}

interface ModelStatsStore {
  stats: Record<string, number[]>;

  initialized: boolean;

  record: (modelId: string, asrMs: number) => void;
  getSamples: (modelId: string) => number[];

  clear: () => void;
  initialize: () => Promise<void>;
}

export const useModelStatsStore = create<ModelStatsStore>()(
  persist(
    (set, get) => ({
      stats: {},
      initialized: false,

      record: (modelId, asrMs) => {
        if (!modelId || asrMs <= 0) return;
        set((state) => {
          const current = state.stats[modelId] ?? [];
          const updated = [asrMs, ...current].slice(0, KEEP);
          return { stats: { ...state.stats, [modelId]: updated } };
        });
      },

      getSamples: (modelId) => get().stats[modelId] ?? [],

      clear: () => set({ stats: {} }),

      initialize: async () => {
        if (get().initialized) return;
        set({ initialized: true });
        await listen<TranscriptionStatsPayload>(
          "transcription-stats-recorded",
          (event) => {
            get().record(event.payload.model_id, event.payload.asr_ms);
          },
        );
      },
    }),
    {
      name: "anonen-model-stats",
      version: 1,

      partialize: (state) => ({ stats: state.stats }),
    },
  ),
);

export function medianMs(samples: number[]): number | null {
  if (samples.length === 0) return null;
  const sorted = [...samples].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)];
}

export const MIN_FOR_RELATIVE = 3;

export function relativeSpeedScores(
  stats: Record<string, number[]>,
): Record<string, number> {
  const medians = new Map<string, number>();
  for (const [modelId, samples] of Object.entries(stats)) {
    if (!modelId.startsWith(ANONEN_CLOUD_PREFIX)) continue;
    if (samples.length < MIN_FOR_RELATIVE) continue;
    const m = medianMs(samples);
    if (m !== null && m > 0) medians.set(modelId, m);
  }
  if (medians.size < 2) return {};

  const fastest = Math.min(...medians.values());
  const out: Record<string, number> = {};
  for (const [modelId, ms] of medians) {
    out[modelId] = Math.max(0.01, Math.min(1, fastest / ms));
  }
  return out;
}
