export const PRIOR_WEIGHT = 5;

export const MIN_AUDIO_MS = 2000;

export function perAudioSecond(
  samples: { ms: number; audioMs: number }[],
): number[] {
  return samples
    .filter((s) => s.audioMs >= MIN_AUDIO_MS && s.ms > 0)
    .map((s) => s.ms / (s.audioMs / 1000));
}

export function medianMs(samples: number[]): number | null {
  if (samples.length === 0) return null;
  const sorted = [...samples].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)];
}

export function blendedSpeedScores(
  stats: Record<string, number[]>,
  serverScores: Record<string, number>,
): Record<string, number> {
  const rows: { id: string; s: number; n: number; l: number }[] = [];
  for (const [id, s] of Object.entries(serverScores)) {
    if (!(s > 0)) continue;
    const samples = stats[id] ?? [];
    const l = medianMs(samples);
    rows.push({ id, s, n: l === null ? 0 : samples.length, l: l ?? 0 });
  }
  const measured = rows.filter((r) => r.n > 0);
  const total = measured.reduce((sum, r) => sum + r.n, 0);
  if (total === 0) return {};

  const c = measured.reduce((sum, r) => sum + r.n * r.l * r.s, 0) / total;
  const blended = rows.map((r) => ({
    id: r.id,
    ms: (PRIOR_WEIGHT * (c / r.s) + r.n * r.l) / (PRIOR_WEIGHT + r.n),
  }));
  const fastest = Math.min(...blended.map((r) => r.ms));
  const out: Record<string, number> = {};
  for (const r of blended) {
    out[r.id] = Math.max(0.01, Math.min(1, fastest / r.ms));
  }
  return out;
}
