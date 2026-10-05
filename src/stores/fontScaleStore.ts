import { create } from "zustand";
import { emit } from "@tauri-apps/api/event";

export type FontScale = 0.9 | 1 | 1.15 | 1.3;

export const FONT_SCALES: FontScale[] = [0.9, 1, 1.15, 1.3];

const BASE_FONT_PX = 15;

export const FONT_SCALE_CHANGED_EVENT = "font-scale-changed";

const STORAGE_KEY = "anonen-font-scale";
const DEFAULT_SCALE: FontScale = 1;

function isFontScale(value: unknown): value is FontScale {
  return FONT_SCALES.includes(value as FontScale);
}

function load(): FontScale {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw !== null) {
      const parsed = Number(raw);
      if (isFontScale(parsed)) return parsed;
    }
  } catch {}
  return DEFAULT_SCALE;
}

export function applyFontScale(scale: FontScale): void {
  document.documentElement.style.fontSize = `${BASE_FONT_PX * scale}px`;
}

interface FontScaleStore {
  scale: FontScale;
  setScale: (scale: FontScale) => void;
}

export const useFontScaleStore = create<FontScaleStore>()((set) => {
  const initial = load();
  applyFontScale(initial);

  return {
    scale: initial,
    setScale: (scale: FontScale) => {
      set({ scale });
      applyFontScale(scale);
      try {
        localStorage.setItem(STORAGE_KEY, String(scale));
      } catch {}

      void emit(FONT_SCALE_CHANGED_EVENT, scale);
    },
  };
});
