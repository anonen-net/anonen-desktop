import { create } from "zustand";
import { persist } from "zustand/middleware";
import { listen } from "@tauri-apps/api/event";

const KEEP = 20;

interface TranscriptionStatsPayload {
  model_id: string;
  asr_ms: number;

  audio_ms: number;
}

export interface StatSample {
  ms: number;
  audioMs: number;
}

interface ModelStatsStore {
  stats: Record<string, StatSample[]>;

  initialized: boolean;

  record: (modelId: string, asrMs: number, audioMs: number) => void;
  getSamples: (modelId: string) => StatSample[];

  clear: () => void;
  initialize: () => Promise<void>;
}

export const useModelStatsStore = create<ModelStatsStore>()(
  persist(
    (set, get) => ({
      stats: {},
      initialized: false,

      record: (modelId, asrMs, audioMs) => {
        if (!modelId || !(asrMs > 0) || !(audioMs > 0)) return;
        set((state) => {
          const current = state.stats[modelId] ?? [];
          const updated = [{ ms: asrMs, audioMs }, ...current].slice(0, KEEP);
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
            get().record(
              event.payload.model_id,
              event.payload.asr_ms,
              event.payload.audio_ms,
            );
          },
        );
      },
    }),
    {
      name: "anonen-model-stats",

      version: 2,
      migrate: () => ({ stats: {} }),

      partialize: (state) => ({ stats: state.stats }),
    },
  ),
);

export { medianMs } from "../lib/speedBlend";
