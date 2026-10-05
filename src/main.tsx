import React from "react";
import ReactDOM from "react-dom/client";
import { platform } from "@tauri-apps/plugin-os";
import App from "./App";
import { disableBrowserContextMenu } from "./lib/disableBrowserContextMenu";

document.documentElement.dataset.platform = platform();

disableBrowserContextMenu();

import "./stores/themeStore";

import "./stores/fontScaleStore";

import "./i18n";

import { useModelStore } from "./stores/modelStore";
useModelStore.getState().initialize();

import { useModelStatsStore } from "./stores/modelStatsStore";
useModelStatsStore.getState().initialize();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
