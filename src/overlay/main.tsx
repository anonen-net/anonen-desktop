import React from "react";
import ReactDOM from "react-dom/client";
import { listen } from "@tauri-apps/api/event";
import RecordingOverlay from "./RecordingOverlay";
import "@/i18n";
import {
  applyTheme,
  useThemeStore,
  THEME_CHANGED_EVENT,
  type ThemePayload,
} from "@/stores/themeStore";
import { disableBrowserContextMenu } from "@/lib/disableBrowserContextMenu";
import {
  applyFontScale,
  useFontScaleStore,
  FONT_SCALE_CHANGED_EVENT,
  type FontScale,
} from "@/stores/fontScaleStore";

disableBrowserContextMenu();

useThemeStore.getState();

useFontScaleStore.getState();
void listen<FontScale>(FONT_SCALE_CHANGED_EVENT, (event) => {
  applyFontScale(event.payload);
});

void listen<ThemePayload>(THEME_CHANGED_EVENT, (event) => {
  applyTheme(event.payload);
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <RecordingOverlay />
  </React.StrictMode>,
);
