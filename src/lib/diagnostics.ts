import { getVersion } from "@tauri-apps/api/app";
import { arch, platform, version as osVersion } from "@tauri-apps/plugin-os";
import i18n from "i18next";
import { commands } from "@/bindings";
import { useAnonenCloudStore } from "@/stores/anonenCloudStore";
import { useSettingsStore } from "@/stores/settingsStore";

const LOG_LINES = 500;

const NATIVE_LOG_LINES = 80;
const USER_ID_PREFIX = 8;
const ANONEN_CLOUD_PREFIX = "anonen-cloud:";
const LABEL_WIDTH = 10;

const row = (label: string, value: string): string =>
  `${label.padEnd(LABEL_WIDTH)}: ${value}`;

const hours = (seconds: number): string => (seconds / 3600).toFixed(1);

const timestamp = (now: Date): string => {
  const pad = (n: number) => String(n).padStart(2, "0");
  const offsetMinutes = -now.getTimezoneOffset();
  const sign = offsetMinutes < 0 ? "-" : "+";
  const abs = Math.abs(offsetMinutes);
  const zone = Intl.DateTimeFormat().resolvedOptions().timeZone ?? "unknown";
  return (
    `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ` +
    `${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())} ` +
    `${sign}${pad(Math.floor(abs / 60))}:${pad(abs % 60)} (${zone})`
  );
};

const appRow = async (): Promise<string> => {
  const version = await getVersion().catch(() => "unknown");
  const install = await commands
    .isPortable()
    .then((portable) => (portable ? "portable" : "installed"))
    .catch(() => "install type unknown");
  return row("app", `anonen ${version} (${install})`);
};

const environmentRows = (): string[] => {
  const settings = useSettingsStore.getState().settings;
  const rows = [
    row("os", `${platform()} ${osVersion()} (${arch()})`),
    row("language", `ui ${i18n.language}`),
  ];
  if (!settings) return [...rows, row("settings", "not loaded")];

  const model = settings.selected_model ?? "(none selected)";
  const kind = model.startsWith(ANONEN_CLOUD_PREFIX) ? "cloud" : "local";
  return [
    ...rows,
    row("model", `${model} (${kind})`),
    row("audio in", settings.selected_microphone ?? "(default device)"),
    row("log level", String(settings.log_level ?? "(default)")),
  ];
};

const accountRows = (): string[] => {
  const { authStatus, usage } = useAnonenCloudStore.getState();
  if (!authStatus.signed_in) return [row("account", "signed out")];

  const id = authStatus.user_id?.slice(0, USER_ID_PREFIX);
  const rows = [row("account", `signed in (id ${id ?? "unavailable"})`)];
  if (!usage) return [...rows, row("plan", "no usage snapshot cached")];

  const subscription = usage.subscription
    ? `${usage.subscription.status}, period end ${usage.subscription.current_period_end}`
    : "none";
  rows.push(row("plan", `${usage.plan?.name ?? "unknown"} (${subscription})`));
  rows.push(
    row(
      "usage",
      `week ${hours(usage.usage.week_used_s)}/${hours(usage.usage.week_cap_s)}h, ` +
        `month ${hours(usage.usage.month_used_s)}/${hours(usage.usage.month_cap_s)}h`,
    ),
  );
  if (usage.usage.max_request_s != null) {
    rows.push(row("per rec.", `${usage.usage.max_request_s}s max`));
  }
  return rows;
};

const readLogTail = async (): Promise<string[] | null> => {
  try {
    const result = await commands.readRecentLogs(LOG_LINES);
    return result.status === "ok" ? result.data : null;
  } catch {
    return null;
  }
};

const readNativeLogTail = async (): Promise<string[] | null> => {
  try {
    const result = await commands.readNativeStderrLog(NATIVE_LOG_LINES);
    return result.status === "ok" ? result.data : null;
  } catch {
    return null;
  }
};

export const buildDiagnosticsReport = async (): Promise<string> => {
  const [log, nativeLog] = await Promise.all([
    readLogTail(),
    readNativeLogTail(),
  ]);
  const logRow =
    log === null
      ? row("log", "unavailable")
      : row("log", `last ${log.length} lines, oldest first`);

  const nativeRow = row(
    "native log",
    nativeLog === null
      ? "unavailable"
      : nativeLog.length === 0
        ? "none"
        : `last ${nativeLog.length} lines`,
  );

  const header = [
    "==== ANONEN diagnostics ====",
    row("generated", timestamp(new Date())),
    await appRow(),
    ...environmentRows(),
    ...accountRows(),
    logRow,
    nativeRow,
    "============================",
  ].join("\n");

  const native =
    nativeLog === null || nativeLog.length === 0
      ? ""
      : `\n==== native stderr (last ${nativeLog.length}) ====\n` +
        `${nativeLog.join("\n")}\n============================`;
  const body = log === null || log.length === 0 ? "" : `\n${log.join("\n")}`;
  return `${header}${native}${body}`;
};
