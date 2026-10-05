import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { resolve } from "path";
import { execSync } from "child_process";

const host = process.env.TAURI_DEV_HOST;

function resolveGitInfo() {
  const read = (cmd: string) => {
    try {
      return execSync(cmd, { stdio: ["ignore", "pipe", "ignore"] })
        .toString()
        .trim();
    } catch {
      return "";
    }
  };
  return {
    hash: read("git rev-parse --short HEAD") || "unknown",
    commitDate: read("git log -1 --format=%cd --date=short"),
  };
}

const gitInfo = resolveGitInfo();

const updaterEnabled = true;

if (process.env.HANDY_EDITION && !process.env.ANONEN_EDITION) {
  throw new Error(
    "HANDY_EDITION is no longer used; set ANONEN_EDITION instead " +
      "(otherwise this produces a dev build).",
  );
}
const appEdition = process.env.ANONEN_EDITION === "dev" ? "dev" : "product";

export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],

  envDir: resolve(__dirname, "src-tauri"),

  define: {
    __GIT_HASH__: JSON.stringify(gitInfo.hash),
    __GIT_COMMIT_DATE__: JSON.stringify(gitInfo.commitDate),
    __UPDATER_ENABLED__: JSON.stringify(updaterEnabled),
    __APP_EDITION__: JSON.stringify(appEdition),
  },

  resolve: {
    alias: {
      "@": resolve(__dirname, "./src"),
      "@/bindings": resolve(__dirname, "./src/bindings.ts"),
    },
  },

  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        overlay: resolve(__dirname, "src/overlay/index.html"),
      },
    },
  },

  clearScreen: false,

  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
