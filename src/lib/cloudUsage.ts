import type { AsrGatewayUsage } from "@/bindings";

export const LOW_REMAINING_THRESHOLD = 0.1;

export function weekCapFullS(usage: AsrGatewayUsage): number {
  const full = usage.week_cap_full_s ?? usage.week_cap_s;
  return Math.max(full, usage.week_cap_s);
}

export function isTrialCapped(usage: AsrGatewayUsage): boolean {
  return weekCapFullS(usage) > usage.week_cap_s && usage.week_cap_s > 0;
}

export function weekRemainingS(usage: AsrGatewayUsage): number {
  return Math.max(0, usage.week_cap_s - usage.week_used_s);
}

export function monthRemainingS(usage: AsrGatewayUsage): number {
  return Math.max(0, usage.month_cap_s - usage.month_used_s);
}

export function remainingS(usage: AsrGatewayUsage): number {
  return Math.min(weekRemainingS(usage), monthRemainingS(usage));
}

export function isLowRemaining(
  usage: AsrGatewayUsage | null | undefined,
): boolean {
  if (!usage) return false;
  if (usage.week_cap_s <= 0 && usage.month_cap_s <= 0) return false;
  const weekRatio =
    usage.week_cap_s > 0 ? weekRemainingS(usage) / usage.week_cap_s : 1;
  const monthRatio =
    usage.month_cap_s > 0 ? monthRemainingS(usage) / usage.month_cap_s : 1;
  return Math.min(weekRatio, monthRatio) < LOW_REMAINING_THRESHOLD;
}

export function formatDuration(secs: number): string {
  const totalMin = Math.max(0, Math.floor(secs / 60));
  const h = Math.floor(totalMin / 60);
  const m = totalMin % 60;
  return h > 0 ? `${h}h ${String(m).padStart(2, "0")}m` : `${m}m`;
}

export function formatResetDateTime(iso: string, language: string): string {
  return formatResetDateTimeOrNull(iso, language) ?? iso;
}

export function formatResetDateTimeOrNull(
  iso: string,
  language: string,
): string | null {
  const d = new Date(iso);
  if (isNaN(d.valueOf())) return null;
  return new Intl.DateTimeFormat(language, {
    timeZone: "Asia/Tokyo",
    year: "numeric",
    month: "numeric",
    day: "numeric",
    weekday: "short",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(d);
}

export function formatResetDate(iso: string, language: string): string {
  const d = new Date(iso);
  if (isNaN(d.valueOf())) return iso;
  return new Intl.DateTimeFormat(language, {
    timeZone: "Asia/Tokyo",
    year: "numeric",
    month: "numeric",
    day: "numeric",
  }).format(d);
}

export const RETRANSCRIBE_CONFIRM_QUIET_MS = 3 * 60 * 1000;

const ANONEN_CLOUD_PREFIX = "anonen-cloud:";

export function needsRetranscribeConfirm(
  modelId: string | null,
  entitled: boolean,
  confirmedAt: number | null,
  now: number,
): boolean {
  if (!entitled) return false;
  if (modelId !== null && !modelId.startsWith(ANONEN_CLOUD_PREFIX)) {
    return false;
  }
  if (confirmedAt === null) return true;
  const elapsed = now - confirmedAt;
  return elapsed < 0 || elapsed >= RETRANSCRIBE_CONFIRM_QUIET_MS;
}

let retranscribeConfirmedAt: number | null = null;

export function lastRetranscribeConfirm(): number | null {
  return retranscribeConfirmedAt;
}

export function noteRetranscribeConfirmed(at: number): void {
  retranscribeConfirmedAt = at;
}
