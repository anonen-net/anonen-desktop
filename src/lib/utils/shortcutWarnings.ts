const MODIFIERS = new Set([
  "ctrl",
  "shift",
  "alt",
  "option",
  "super",
  "command",
  "meta",
]);

const SAFE_ALONE = new Set([
  "escape",
  "pause",
  "scrolllock",
  "insert",
  "printscreen",
]);

const isFunctionKey = (part: string): boolean =>
  /^f([1-9]|1\d|2[0-4])$/.test(part);

const WELL_KNOWN_CONFLICTS = new Set([
  "ctrl+a",
  "ctrl+c",
  "ctrl+f",
  "ctrl+n",
  "ctrl+p",
  "ctrl+s",
  "ctrl+t",
  "ctrl+v",
  "ctrl+w",
  "ctrl+x",
  "ctrl+y",
  "ctrl+z",

  "alt+tab",
  "alt+f4",

  "alt+space",
  "ctrl+shift+escape",
  "command+q",
  "command+w",
  "command+tab",
  "command+space",
]);

export type ShortcutWarningCode = "noModifier" | "duplicate" | "wellKnown";

export interface ShortcutWarning {
  code: ShortcutWarningCode;

  conflictsWith?: string;
}

export const shortcutParts = (binding: string): string[] =>
  binding
    .split("+")
    .map((p) =>
      p
        .trim()
        .toLowerCase()
        .replace(/_(left|right)$/, ""),
    )
    .filter((p) => p.length > 0);

export const canonicalShortcut = (binding: string): string =>
  [...shortcutParts(binding)].sort().join("+");

const hasModifier = (parts: string[]): boolean =>
  parts.some((p) => MODIFIERS.has(p));

export const shortcutWarnings = (
  binding: string,
  others: Record<string, string> = {},
): ShortcutWarning[] => {
  const parts = shortcutParts(binding);
  if (parts.length === 0) return [];

  const warnings: ShortcutWarning[] = [];

  if (!hasModifier(parts)) {
    const alone = parts[0];

    if (parts.length === 1 && (SAFE_ALONE.has(alone) || isFunctionKey(alone))) {
    } else {
      warnings.push({ code: "noModifier" });
    }
  }

  const mine = canonicalShortcut(binding);
  for (const [id, other] of Object.entries(others)) {
    if (other && canonicalShortcut(other) === mine) {
      warnings.push({ code: "duplicate", conflictsWith: id });
      break;
    }
  }

  if (WELL_KNOWN_CONFLICTS.has(parts.join("+"))) {
    warnings.push({ code: "wellKnown" });
  }

  return warnings;
};
