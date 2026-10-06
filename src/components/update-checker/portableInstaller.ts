const RELEASES_BASE = "https://github.com/anonen-net/anonen-desktop/releases";

export const PORTABLE_RELEASES_URL = `${RELEASES_BASE}/latest`;

export function releaseTagUrl(version: string): string {
  const v = version.replace(/^v/, "");
  return /^\d+\.\d+\.\d+$/.test(v)
    ? `${RELEASES_BASE}/tag/v${v}`
    : RELEASES_BASE;
}

export function resolvePortableInstallerUrl(
  rawJson: Record<string, unknown> | undefined,
  platformName: string,
  archName: string,
): string {
  if (platformName !== "windows") return PORTABLE_RELEASES_URL;

  const platforms = rawJson?.platforms;
  if (!platforms || typeof platforms !== "object") return PORTABLE_RELEASES_URL;

  const entry = (platforms as Record<string, unknown>)[
    `windows-${archName}-nsis`
  ];
  if (!entry || typeof entry !== "object") return PORTABLE_RELEASES_URL;

  const url = (entry as Record<string, unknown>).url;
  return typeof url === "string" ? url : PORTABLE_RELEASES_URL;
}
