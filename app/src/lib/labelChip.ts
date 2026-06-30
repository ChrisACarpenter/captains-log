/*
 * Captain's Log — Label chip color resolution (Phase 3a, Slice 7)
 *
 * Single source of truth for the inline `style="--chip-color: …"` value used
 * by every chip-like surface that renders a label name. Originally lived
 * inline in LabelInput.svelte; extracted so the Labels-tab list (Settings →
 * Labels), the LabelDetailsModal preview chip, and any future chip surface
 * all paint consistently with the rules below.
 *
 * Resolution order:
 *   1. When `colorfulLabels === false` → cycle through the 5-color accent-var
 *      palette by hashing the name. This is the pre-2.8-follow-on behavior
 *      and remains the default for users who haven't opted into the colorful
 *      labels toggle in /settings.
 *   2. When `colorfulLabels === true`:
 *        a. If `entry.color` is set, that persisted hex wins (the user
 *           explicitly committed to a color via the Label Manager →
 *           set_label_color command — they're choosing it across themes).
 *        b. Otherwise, deterministically generate a hue from the name via
 *           generateLabelColor, regenerated against the live theme on every
 *           render. No persistence: a theme switch always re-yields a
 *           color tuned for the new surface.
 *
 * Returns a complete inline-style string, e.g. `--chip-color: #ff5c08;` or
 * `--chip-color: var(--accent-pink-text);`. Callers paste it onto an element's
 * `style` attribute and the chip CSS (border/color: var(--chip-color)) does
 * the rest.
 */
import { generateLabelColor } from './theme';

export type ChipEntry = {
  name: string;
  color?: string | null;
};

// -------------------------------------------------------------------------
// Accent-var palette (colorful-labels OFF path)
// -------------------------------------------------------------------------
//
// Phase 2.7 contrast-audit cleanup:
//   - --accent-pink swapped for --accent-pink-text (raw pink at body size
//     fails WCAG AA on every dark surface — bg-base 3.57:1, bg-surface
//     2.73:1, bg-elevated 2.35:1; the *-text variant lifts above 4.5:1).
//   - --accent-teal removed — it's #235151 (very dark) and measures only
//     1.61:1 on bg-surface, so it's effectively invisible in dark theme.
//     No safe lifted variant exists yet; pruning is the lower-risk fix.
const ACCENT_VARS = [
  '--accent-pink-text',
  '--accent-yellow',
  '--accent-sky',
  '--accent-lavender',
  '--accent-green',
];

function chipAccent(name: string): string {
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = ((hash << 5) - hash + name.charCodeAt(i)) | 0;
  }
  return ACCENT_VARS[Math.abs(hash) % ACCENT_VARS.length];
}

/**
 * Resolve the inline `style="--chip-color: …"` value for one label entry.
 *
 * @param entry         The label being rendered. `entry.color` is the
 *                      persisted hex override from labels.json (if any).
 * @param colorfulLabels Whether the colorful-labels toggle is on. Drives
 *                      which branch is taken — accent-var cycle vs. per-label
 *                      hex.
 * @param theme         The current resolved theme polarity: 'light', 'dark',
 *                      or 'custom'. For 'custom', `bgSurface` is required to
 *                      pick the readable lightness.
 * @param bgSurface     Computed value of `--bg-surface` on :root. Only used
 *                      when `theme === 'custom'`; passing it for light/dark
 *                      is harmless. Callers typically read it via
 *                      `getComputedStyle(document.documentElement).getPropertyValue('--bg-surface')`.
 *
 * @returns A complete inline-style string. Empty input or unresolvable
 *          color still yields a valid `--chip-color: …;` value (the
 *          generator never throws).
 */
export function chipStyleFor(
  entry: ChipEntry,
  colorfulLabels: boolean,
  theme: 'light' | 'dark' | 'custom',
  bgSurface?: string,
): string {
  if (!colorfulLabels) {
    return `--chip-color: var(${chipAccent(entry.name)});`;
  }

  const persisted = entry.color ?? null;
  if (persisted) {
    return `--chip-color: ${persisted};`;
  }

  const hex = generateLabelColor(entry.name, theme, bgSurface);
  return `--chip-color: ${hex};`;
}
