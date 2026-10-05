import { create } from "zustand";
import { emit } from "@tauri-apps/api/event";

export interface ThemeColors {
  accentColor: string;
  accentColorLight: string;
}

export type ThemeMode = "system" | "light" | "dark";

export interface ThemePayload {
  colors: ThemeColors;
  mode: ThemeMode;
}

export const THEME_CHANGED_EVENT = "theme-changed";

interface ThemeStore {
  colors: ThemeColors;
  mode: ThemeMode;
  setColors: (colors: Partial<ThemeColors>) => void;
  setMode: (mode: ThemeMode) => void;
  reset: () => void;
}

const STORAGE_KEY = "handy-theme-colors";

const DEFAULT_COLORS: ThemeColors = {
  accentColor: "#16a34a",
  accentColorLight: "#16a34a",
};
const DEFAULT_MODE: ThemeMode = "system";

interface StoredTheme extends ThemeColors {
  mode?: ThemeMode;
}

function load(): { colors: ThemeColors; mode: ThemeMode } {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed: StoredTheme = JSON.parse(raw);
      return {
        colors: { ...DEFAULT_COLORS, ...parsed },
        mode: parsed.mode ?? DEFAULT_MODE,
      };
    }
  } catch {}
  return { colors: { ...DEFAULT_COLORS }, mode: DEFAULT_MODE };
}

function save(colors: ThemeColors, mode: ThemeMode) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...colors, mode }));
}

function applyColors(colors: ThemeColors) {
  const root = document.documentElement;
  root.style.setProperty("--color-background-ui", colors.accentColor);
  root.style.setProperty("--color-logo-primary", colors.accentColorLight);
}

const darkMedia = window.matchMedia("(prefers-color-scheme: dark)");

function applyMode(mode: ThemeMode) {
  const resolved =
    mode === "system" ? (darkMedia.matches ? "dark" : "light") : mode;
  document.documentElement.dataset.theme = resolved;
}

export function applyTheme({ colors, mode }: ThemePayload) {
  applyColors(colors);
  applyMode(mode);
}

function broadcast(colors: ThemeColors, mode: ThemeMode) {
  void emit(THEME_CHANGED_EVENT, { colors, mode } satisfies ThemePayload);
}

export const useThemeStore = create<ThemeStore>()((set, get) => {
  const initial = load();
  applyColors(initial.colors);
  applyMode(initial.mode);

  darkMedia.addEventListener("change", () => {
    if (get().mode === "system") {
      applyMode("system");
    }
  });

  return {
    colors: initial.colors,
    mode: initial.mode,

    setColors: (partial) =>
      set((state) => {
        const next = { ...state.colors, ...partial };
        save(next, state.mode);
        applyColors(next);
        broadcast(next, state.mode);
        return { colors: next };
      }),

    setMode: (mode) =>
      set((state) => {
        save(state.colors, mode);
        applyMode(mode);
        broadcast(state.colors, mode);
        return { mode };
      }),

    reset: () => {
      localStorage.removeItem(STORAGE_KEY);
      applyColors(DEFAULT_COLORS);
      applyMode(DEFAULT_MODE);
      broadcast(DEFAULT_COLORS, DEFAULT_MODE);
      set({ colors: { ...DEFAULT_COLORS }, mode: DEFAULT_MODE });
    },
  };
});

export const THEME_DEFAULTS = DEFAULT_COLORS;
