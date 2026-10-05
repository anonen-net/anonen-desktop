export const PORTABLE_RELEASES_URL =
  "https://github.com/anonen-net/anonen-desktop/releases/latest";

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
