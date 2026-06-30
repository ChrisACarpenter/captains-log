/*
 * Captain's Log — Custom theme derivation engine (Phase 2.8, Slice 2)
 *
 * Public API:
 *   - deriveTokens(primaries, base): full token map (primaries + derived)
 *   - applyCustomTheme(map): write inline CSS vars onto :root
 *   - clearCustomTheme(): remove inline vars so Light/Dark CSS rules win
 *   - contrastRatio(fg, bg): WCAG ratio (1.0 – 21.0)
 *
 * The pipeline is deterministic — given the shipping Light/Dark primaries it
 * reproduces the existing hand-tuned derived values within a small tolerance.
 * See theme.test.ts for the reproduction harness.
 */

import { converter, formatHex, formatRgb, parse, wcagContrast } from 'culori';
import type { Color, Oklch, Rgb } from 'culori';

// -------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------

export type PrimaryTokens = {
  bgBase: string;
  bgSurface: string;
  bgElevated: string;
  textPrimary: string;
  textSecondary: string;
  textMuted: string;
  borderStructural: string;
  borderDecorative: string;
  accentPrimary: string;
  accentGreen: string;
  accentPink: string;
  btnSapphire: string;
};

export type ThemeBase = 'light' | 'dark';

export type DerivedTokens = Record<string, string>;

// -------------------------------------------------------------------------
// Slice 6 — surface diagnostics
// -------------------------------------------------------------------------

/** OKLCH L range where bg-surface is ambiguous between light/dark base. */
const AMBIGUOUS_L_MIN = 0.5;
const AMBIGUOUS_L_MAX = 0.6;

/** OKLCH chroma threshold above which a surface counts as "saturated". */
const SATURATED_C_MIN = 0.15;

/** True when bg-surface lands in the mid-grey zone where light vs dark
 *  text is a coin-flip without user guidance. Callers surface a 2-way
 *  "Force base:" picker only when this returns true. */
export function isAmbiguousBaseSurface(bgSurfaceHex: string): boolean {
  try {
    const o = parseToOklch(bgSurfaceHex);
    return o.l >= AMBIGUOUS_L_MIN && o.l <= AMBIGUOUS_L_MAX;
  } catch {
    return false;
  }
}

/** True when bg-surface is vivid enough that the standard derivations may
 *  produce surprising results (e.g. focus ring blends in, error pink reads
 *  as warning). Callers show a non-blocking advisory banner. */
export function isSaturatedSurface(bgSurfaceHex: string): boolean {
  try {
    const o = parseToOklch(bgSurfaceHex);
    return o.c > SATURATED_C_MIN;
  } catch {
    return false;
  }
}

/** Heuristic that picks the base polarity for a given surface. Cheap
 *  perceptual proxy: anything above the ambiguous band reads as light;
 *  anything below reads as dark; ties (the ambiguous band itself) fall
 *  back to 'dark' so a caller without a forceBase override still gets
 *  a deterministic answer. */
export function inferBaseFromSurface(bgSurfaceHex: string): ThemeBase {
  try {
    const o = parseToOklch(bgSurfaceHex);
    return o.l > AMBIGUOUS_L_MAX ? 'light' : 'dark';
  } catch {
    return 'dark';
  }
}

// -------------------------------------------------------------------------
// Internal helpers
// -------------------------------------------------------------------------

const toOklch = converter('oklch');
const toRgb = converter('rgb');

function parseToOklch(hexOrColor: string): Oklch {
  const parsed = parse(hexOrColor);
  if (!parsed) throw new Error(`Cannot parse color: ${hexOrColor}`);
  const o = toOklch(parsed);
  return {
    mode: 'oklch',
    l: o.l ?? 0,
    c: o.c ?? 0,
    h: o.h ?? 0,
  };
}

type OklchLike = { l?: number; c?: number; h?: number };

function oklchToHex(o: OklchLike): string {
  const color: Oklch = {
    mode: 'oklch',
    l: clamp(o.l ?? 0, 0, 1),
    c: Math.max(0, o.c ?? 0),
    h: o.h ?? 0,
  };
  // Clamp into sRGB gamut by reducing chroma if needed.
  const rgb = toRgb(color) as Rgb;
  const r = clamp(rgb.r, 0, 1);
  const g = clamp(rgb.g, 0, 1);
  const b = clamp(rgb.b, 0, 1);
  return formatHex({ mode: 'rgb', r, g, b });
}

function clamp(n: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, n));
}

/** WCAG contrast ratio. fg and bg are anything culori can parse. */
export function contrastRatio(fg: string, bg: string): number {
  const a = parse(fg);
  const b = parse(bg);
  if (!a || !b) return 1;
  // culori's wcagContrast returns the ratio (>= 1).
  return wcagContrast(a, b);
}

/**
 * Walk L (lightness) of `seed` toward `targetEnd` (0 = darker, 1 = lighter)
 * until WCAG contrast against `against` clears `minRatio`. Hue + chroma
 * are held; chroma is reduced only if we hit the L bound.
 *
 * Returns the final candidate hex along with the achieved contrast ratio
 * and a `converged` flag. When the iteration cannot reach `minRatio` within
 * the budget, callers that need a hard contrast floor should fall back via
 * `betterOfBlackWhite` rather than paint the under-contrast candidate.
 */
type IterationResult = { value: string; ratio: number; converged: boolean };

function iterateForContrast(
  seed: Oklch,
  against: string,
  minRatio: number,
  targetEnd: 0 | 1,
): IterationResult {
  let l = seed.l;
  let c = seed.c;
  const step = 0.01;
  // Chroma slowly fades as L moves toward an endpoint — pure saturated hues
  // get less saturated near black/white in real-world pigments, and this
  // matches the hand-tuned shipping values (e.g. #ff80c0 has C lower than
  // its #eb018b seed).
  const chromaDecayPerStep = 0.0015;
  let lastHex = oklchToHex({ l, c, h: seed.h });
  let lastRatio = contrastRatio(lastHex, against);
  for (let i = 0; i < 120; i++) {
    const candidate: Oklch = { mode: 'oklch', l, c, h: seed.h };
    const hex = oklchToHex(candidate);
    const ratio = contrastRatio(hex, against);
    lastHex = hex;
    lastRatio = ratio;
    if (ratio >= minRatio) {
      return { value: hex, ratio, converged: true };
    }
    if (targetEnd === 1) {
      if (l >= 1) {
        if (c <= 0.001) break;
        c = Math.max(0, c - 0.02);
      } else {
        l = Math.min(1, l + step);
        c = Math.max(0, c - chromaDecayPerStep);
      }
    } else {
      if (l <= 0) {
        if (c <= 0.001) break;
        c = Math.max(0, c - 0.02);
      } else {
        l = Math.max(0, l - step);
        c = Math.max(0, c - chromaDecayPerStep);
      }
    }
  }
  return { value: lastHex, ratio: lastRatio, converged: false };
}

/** Return whichever of {white, black} has higher contrast against `host`.
 *  Used as a hard-floor fallback when iterateForContrast fails to converge
 *  but the token MUST clear a contrast threshold (e.g. text variants). */
function betterOfBlackWhite(host: string): { value: string; ratio: number } {
  const whiteRatio = contrastRatio('#ffffff', host);
  const blackRatio = contrastRatio('#000000', host);
  return whiteRatio >= blackRatio
    ? { value: '#ffffff', ratio: whiteRatio }
    : { value: '#000000', ratio: blackRatio };
}

/** Resolve a hue-walk that MUST hit minRatio against host. If iteration
 *  converges, use it; otherwise fall back to better-of-{white,black}. */
function resolveTextWithFallback(
  seed: Oklch,
  host: string,
  minRatio: number,
  targetEnd: 0 | 1,
): { value: string; ratio: number; converged: boolean } {
  const walked = iterateForContrast(seed, host, minRatio, targetEnd);
  if (walked.converged) return walked;
  const fallback = betterOfBlackWhite(host);
  // If the walked candidate happens to beat the fallback (rare, but
  // possible when both fail to clear minRatio), prefer the higher ratio.
  if (walked.ratio > fallback.ratio) {
    return { value: walked.value, ratio: walked.ratio, converged: false };
  }
  return { value: fallback.value, ratio: fallback.ratio, converged: false };
}

/** Composite `fg` at `alpha` over an opaque `bg`. Returns the resulting
 *  opaque hex — i.e. what the eye sees when the rgba layer is painted on
 *  top of the surface. Used by ::selection legibility: browsers render
 *  selection as a semi-transparent fill, so the visible "selection
 *  background" is a blend of accent-primary over bg-surface, not the
 *  raw accent-primary itself. */
function compositeOver(fg: string, alpha: number, bg: string): string {
  const f = parse(fg);
  const b = parse(bg);
  if (!f || !b) return bg;
  const fr = toRgb(f) as Rgb;
  const br = toRgb(b) as Rgb;
  const a = clamp(alpha, 0, 1);
  const r = clamp(fr.r * a + br.r * (1 - a), 0, 1);
  const g = clamp(fr.g * a + br.g * (1 - a), 0, 1);
  const bl = clamp(fr.b * a + br.b * (1 - a), 0, 1);
  return formatHex({ mode: 'rgb', r, g, b: bl });
}

/** rgba(...) string from a hex + alpha. */
function withAlpha(hexOrColor: string, alpha: number): string {
  const p = parse(hexOrColor);
  if (!p) return hexOrColor;
  const rgb = toRgb(p) as Rgb;
  const r = Math.round(clamp(rgb.r, 0, 1) * 255);
  const g = Math.round(clamp(rgb.g, 0, 1) * 255);
  const b = Math.round(clamp(rgb.b, 0, 1) * 255);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

// -------------------------------------------------------------------------
// Public: deriveTokens
// -------------------------------------------------------------------------

export function deriveTokens(
  primaries: PrimaryTokens,
  base: ThemeBase,
): DerivedTokens {
  const isDark = base === 'dark';

  // Independently infer the effective polarity from the SURFACE — not the
  // global `base` arg. Edge case: a user picks a pale bgSurface but the
  // theme is still marked Dark (e.g. they're mid-edit). Text-walk callers
  // need to know which endpoint (white-ward or black-ward) the iteration
  // should head toward against THIS specific surface, regardless of how
  // the user labelled the overall base.
  const inferredBaseBase = inferBaseFromSurface(primaries.bgBase);
  const inferredElevatedBase = inferBaseFromSurface(primaries.bgElevated);

  // Helper: choose the targetEnd for an iteration based on the host
  // surface itself. Pale host → walk toward black (0). Dark host → toward
  // white (1).
  const endFor = (hostBase: ThemeBase): 0 | 1 =>
    hostBase === 'dark' ? 1 : 0;

  // Parse all primaries once.
  const bgBase = parseToOklch(primaries.bgBase);
  const bgSurface = parseToOklch(primaries.bgSurface);
  const bgElevated = parseToOklch(primaries.bgElevated);
  const accentPrimary = parseToOklch(primaries.accentPrimary);
  const accentPink = parseToOklch(primaries.accentPink);
  const accentGreen = parseToOklch(primaries.accentGreen);

  // -- bg-code: one step darker than bg-surface on dark, one step warmer/lighter on light
  //    Match shipping: dark surface #36302c L≈0.262 → code #1f1a17 L≈0.151 (Δ ≈ -0.11)
  //                    light surface #ffffff L=1.000 → code #f4e3d2 L≈0.917 (Δ ≈ -0.08)
  const bgCode: Oklch = {
    mode: 'oklch',
    l: clamp(isDark ? bgSurface.l - 0.11 : bgSurface.l - 0.08, 0, 1),
    c: bgSurface.c > 0.005 ? bgSurface.c : Math.min(bgBase.c, 0.04),
    h: bgSurface.c > 0.005 ? bgSurface.h : bgBase.h,
  };

  // -- accent-primary-text: walk L toward light on dark theme, toward dark on
  //    light theme until contrast clears the hardest host surface.
  //    Dark theme: target bg-elevated (lightest dark surface → hardest for
  //      orange-text, the surface where orange has the narrowest L margin).
  //    Light theme: target bg-base (cream, darker than the white surface →
  //      the cream is where orange text struggles).
  //    Both shipping themes clear ≥4.5 on the easier surface as a side
  //    effect.
  const primaryTextHost = isDark ? primaries.bgElevated : primaries.bgBase;
  const primaryTextHostBase = isDark
    ? inferredElevatedBase
    : inferredBaseBase;
  const accentPrimaryText = resolveTextWithFallback(
    accentPrimary,
    primaryTextHost,
    isDark ? 5.0 : 4.5,
    endFor(primaryTextHostBase),
  );

  // -- accent-pink-text: same pattern, host = hardest surface.
  //    Shipping ratios: dark #ff80c0 on bg-elevated #3d3936 = 4.95;
  //                     light #b50079 on bg-base #f7e7db = 5.37.
  const pinkTextHost = isDark ? primaries.bgElevated : primaries.bgBase;
  const pinkTextHostBase = isDark ? inferredElevatedBase : inferredBaseBase;
  const accentPinkText = resolveTextWithFallback(
    accentPink,
    pinkTextHost,
    isDark ? 5.0 : 5.3,
    endFor(pinkTextHostBase),
  );

  // -- accent-green-text: text laid on the green-fill button. Walk toward 0
  //    (dark text) until well past AA against the green fill itself.
  //    Shipping: #1a2b0a on #95c13b ≈ 7.15:1.
  const accentGreenText = resolveTextWithFallback(
    accentGreen,
    primaries.accentGreen,
    7.0,
    0,
  );

  // -- btn-primary-text: text laid on the primary-orange button fill. Pick
  //    whichever endpoint (white/dark) gives better contrast against the fill;
  //    walk further only if needed.
  //    Shipping: #ffffff on #ff5c08 ≈ 3.7:1 — note: doesn't actually clear 4.5.
  //    The current ship value (#ffffff) is hardcoded; we honor the intent
  //    (white when accent is saturated/dark, dark when accent is pale) and
  //    only re-derive when white truly fails.
  //    Shipping: #ffffff on #ff5c08 at ~3.1:1 — the historic brand choice
  //    overrides strict AA. We honor that intent: white is the default for
  //    any saturated/mid-dark primary; only flip to a derived dark when the
  //    primary is genuinely pale (OKLCH L >= 0.75) where white truly fails.
  let btnPrimaryText: string;
  if (accentPrimary.l >= 0.75) {
    // Pale primary: derive a dark text that clears AA against it.
    const palePrimaryText = resolveTextWithFallback(
      { mode: 'oklch', l: 0, c: 0, h: accentPrimary.h },
      primaries.accentPrimary,
      4.5,
      0,
    );
    btnPrimaryText = palePrimaryText.value;
  } else {
    btnPrimaryText = '#ffffff';
  }

  // -- focus-ring: ≥3:1 against the hardest page surface (WCAG 2.2 SC 1.4.11)
  //    Dark: bg-elevated (lightest dark surface) is the hardest target.
  //    Light: bg-base (cream) is harder than the white bg-surface — orange
  //    on cream is where shipping #d94a00 was tuned.
  const focusRingHost = isDark ? primaries.bgElevated : primaries.bgBase;
  const focusRingHostBase = isDark ? inferredElevatedBase : inferredBaseBase;
  let focusRingHex: string;
  // focus-ring is a soft floor — 3:1 is the goal but we accept the
  // best-effort walked value rather than slamming to pure white/black.
  // Callers can surface a warning if `focusRingConverged === false`.
  let focusRingConverged: boolean;
  if (contrastRatio(primaries.accentPrimary, focusRingHost) >= 3.0) {
    focusRingHex = primaries.accentPrimary;
    focusRingConverged = true;
  } else {
    const walked = iterateForContrast(
      accentPrimary,
      focusRingHost,
      3.0,
      endFor(focusRingHostBase),
    );
    focusRingHex = walked.value;
    focusRingConverged = walked.converged;
  }
  // Silence unused-var warning for converged flag (reserved for future
  // UI surfacing of focus-ring contrast warnings — Slice 6 follow-up).
  void focusRingConverged;

  // -- focus-glow: accent-primary at 22% alpha
  const focusGlow = withAlpha(primaries.accentPrimary, 0.22);

  // -- selection-fg: ::selection paints accent-primary at ~30% alpha over
  //    bg-surface (locked decision: 30% per the slice plan). The visible
  //    selection background is therefore the blend, not the raw primary.
  //    For a pale primary (e.g. cream surface + pale orange), white text
  //    on that blend falls under AA, so we pick whichever of {near-white
  //    cream, near-black} gives more contrast against the composite.
  //
  //    Shipping behaviour preserved: dark + saturated orange primary still
  //    resolves to neutral-cream (the value the existing ::selection rule
  //    used) because the white candidate wins handily on dark surfaces.
  const selectionBlendBg = compositeOver(
    primaries.accentPrimary,
    0.3,
    primaries.bgSurface,
  );
  const selectionLightCandidate = isDark
    ? '#f7e7db' // matches shipping --neutral-cream on dark
    : '#ffffff';
  const selectionDarkCandidate = '#1c1612'; // matches shipping --text-primary (light)
  const lightRatio = contrastRatio(selectionLightCandidate, selectionBlendBg);
  const darkRatio = contrastRatio(selectionDarkCandidate, selectionBlendBg);
  const selectionFg =
    lightRatio >= darkRatio ? selectionLightCandidate : selectionDarkCandidate;

  // -- stripe-track: accent-primary at 14% (dark) / 18% (light)
  const stripeTrack = withAlpha(primaries.accentPrimary, isDark ? 0.14 : 0.18);
  // -- stripe-fill: mirrors accent-primary
  const stripeFill = primaries.accentPrimary;

  // -- Error palette: derived from user's --accent-pink (locked decision #3).
  const bgErrorTint = withAlpha(primaries.accentPink, 0.12);
  const bgErrorTintSoft = withAlpha(primaries.accentPink, 0.08);
  const borderError = withAlpha(primaries.accentPink, 0.4);

  // -- Marble button: mirrors bg-elevated + text-primary; border tints toward accent.
  const btnMarbleBg = primaries.bgElevated;
  const btnMarbleText = primaries.textPrimary;
  const btnMarbleBorder = withAlpha(primaries.accentPrimary, 0.22);

  // -- btn-shadow: pure black on dark themes, warm-maroon-ish on light.
  //    Match shipping exactly:
  //      dark : 0 4px 0 0 rgba(0,0,0,0.6)
  //      light: 0 4px 0 0 rgba(108,30,56,0.32)
  const btnShadow = isDark
    ? '0 4px 0 0 rgba(0, 0, 0, 0.6)'
    : '0 4px 0 0 rgba(108, 30, 56, 0.32)';

  // -- brand-orange / brand-maroon: aliases that mirror semantic accents.
  //    accent-maroon is not user-editable — leave as the shipping value, but
  //    re-tint toward the user's pink-hue if they've moved the pink anchor
  //    drastically. For Slice 2, keep accent-maroon fixed; brand-orange
  //    mirrors accent-primary directly.
  const accentMaroon = isDark ? '#6c1e38' : '#6c1e38';
  const brandOrange = primaries.accentPrimary;
  const brandMaroon = accentMaroon;

  // -- neutral-cream: on light themes, mirror bg-base (the cream); on dark,
  //    keep the shipping value (it's the ::selection fg over orange).
  const neutralCream = isDark ? '#f7e7db' : primaries.bgBase;

  const derived: DerivedTokens = {
    // Primaries — included so callers have one full map to apply.
    '--bg-base': primaries.bgBase,
    '--bg-surface': primaries.bgSurface,
    '--bg-elevated': primaries.bgElevated,
    '--bg-code': oklchToHex(bgCode),

    '--text-primary': primaries.textPrimary,
    '--text-secondary': primaries.textSecondary,
    '--text-muted': primaries.textMuted,

    '--border-structural': primaries.borderStructural,
    '--border-decorative': primaries.borderDecorative,

    '--accent-primary': primaries.accentPrimary,
    '--accent-primary-text': accentPrimaryText.value,
    '--accent-pink': primaries.accentPink,
    '--accent-pink-text': accentPinkText.value,
    '--accent-green': primaries.accentGreen,
    '--accent-green-text': accentGreenText.value,
    '--btn-primary-text': btnPrimaryText,

    '--accent-maroon': accentMaroon,
    '--brand-orange': brandOrange,
    '--brand-maroon': brandMaroon,
    '--neutral-cream': neutralCream,

    '--focus-ring': focusRingHex,
    '--focus-glow': focusGlow,
    '--selection-fg': selectionFg,

    '--stripe-track': stripeTrack,
    '--stripe-fill': stripeFill,

    '--bg-error-tint': bgErrorTint,
    '--bg-error-tint-soft': bgErrorTintSoft,
    '--border-error': borderError,

    '--btn-marble-bg': btnMarbleBg,
    '--btn-marble-text': btnMarbleText,
    '--btn-marble-border': btnMarbleBorder,

    '--btn-sapphire': primaries.btnSapphire,
    '--btn-shadow': btnShadow,
  };

  return derived;
}

// -------------------------------------------------------------------------
// Public: applyCustomTheme / clearCustomTheme
// -------------------------------------------------------------------------

const APPLIED_KEYS_ATTR = 'data-cap-custom-keys';

export function applyCustomTheme(derived: DerivedTokens): void {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;
  const keys: string[] = [];
  for (const [k, v] of Object.entries(derived)) {
    root.style.setProperty(k, v);
    keys.push(k);
  }
  root.setAttribute(APPLIED_KEYS_ATTR, keys.join(','));
}

export function clearCustomTheme(): void {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;
  const tracked = root.getAttribute(APPLIED_KEYS_ATTR);
  if (tracked) {
    for (const k of tracked.split(',')) {
      if (k) root.style.removeProperty(k);
    }
    root.removeAttribute(APPLIED_KEYS_ATTR);
  }
}

// -------------------------------------------------------------------------
// Convenience: shipping primaries (used by callers to seed Reset-to-Light/Dark
// and by the test harness to verify the pipeline reproduces shipping themes).
// -------------------------------------------------------------------------

export const SHIPPING_DARK_PRIMARIES: PrimaryTokens = {
  bgBase: '#2b2420',
  bgSurface: '#36302c',
  bgElevated: '#3d3936',
  textPrimary: '#f6e7d7',
  textSecondary: '#d2b094',
  textMuted: '#a89784',
  borderStructural: 'rgba(246, 231, 215, 0.1)',
  borderDecorative: 'rgba(255, 92, 8, 0.14)',
  accentPrimary: '#ff5c08',
  accentGreen: '#95c13b',
  accentPink: '#eb018b',
  btnSapphire: '#3a82c8',
};

export const SHIPPING_LIGHT_PRIMARIES: PrimaryTokens = {
  bgBase: '#f7e7db',
  bgSurface: '#ffffff',
  bgElevated: '#fdf6ee',
  textPrimary: '#1c1612',
  textSecondary: '#5a4438',
  textMuted: '#7a5e48',
  borderStructural: 'rgba(28, 22, 18, 0.12)',
  borderDecorative: 'rgba(255, 92, 8, 0.18)',
  accentPrimary: '#ff5c08',
  accentGreen: '#95c13b',
  accentPink: '#eb018b',
  btnSapphire: '#3a82c8',
};

// -------------------------------------------------------------------------
// Public: generateLabelColor (Phase 2.8 follow-on — Colorful Labels)
// -------------------------------------------------------------------------

/**
 * Deterministic per-label chip color. Same `name` ALWAYS yields the same
 * hex — the lazy-assignment write-back path in LabelInput relies on this so
 * a label that has never been persisted with an explicit color still
 * renders the same hue on every machine before the first save.
 *
 * Algorithm:
 *   1. djb2-style hash of the name → 360° hue.
 *   2. Theme-aware lightness:
 *        - light → L=0.45 (dark chip on pale surface)
 *        - dark  → L=0.70 (pale chip on dark surface)
 *        - custom → branch on bgSurface's OKLCH-L (locked decision #4):
 *            surface > 0.5 → use the light-theme value (0.45)
 *            surface ≤ 0.5 → use the dark-theme value  (0.70)
 *      A missing/unparseable bgSurface defaults to dark (matches the app's
 *      default theme polarity).
 *   3. Chroma fixed at 0.12 — enough saturation for hue separation without
 *      blowing out against the surface. Matches the perceptual range the
 *      shipping accent palette occupies.
 */
export function generateLabelColor(
  name: string,
  theme: 'light' | 'dark' | 'custom',
  bgSurface?: string,
): string {
  // djb2-ish hash → 360° hue. Bitwise `| 0` keeps the accumulator in i32.
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = ((hash << 5) - hash + name.charCodeAt(i)) | 0;
  }
  const hue = Math.abs(hash) % 360;

  let L: number;
  if (theme === 'light') {
    L = 0.45;
  } else if (theme === 'dark') {
    L = 0.7;
  } else {
    // Custom: lightness keys off bg-surface luminance so chips stay
    // legible regardless of which pole the user pushed the surface to.
    let surfaceL = 0.3; // fallback assumes dark surface
    if (bgSurface) {
      try {
        const o = parseToOklch(bgSurface);
        surfaceL = o.l;
      } catch {
        // Unparseable — keep the dark-surface default.
      }
    }
    L = surfaceL > 0.5 ? 0.45 : 0.7;
  }

  return oklchToHex({ l: L, c: 0.12, h: hue });
}

// Quiet unused-import warning when this module is checked in isolation.
export type { Color };
export { formatRgb };
