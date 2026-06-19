# Captain's Log — Style Guide

Brand colors, typography, iconography, and component patterns for the app. Sourced primarily from the Prodigy RPG game's design system, with adjustments for an adult productivity context.

---

## Colors

Pulled from the [Prodigy Deck Template](https://docs.google.com/presentation/d/1CmauVX1YwCpDBQKqU5DVlRaSPnO9CdCP04UlMv64CpQ) (slide 4).

### Primary
- **Prodigy Orange** `#ff5c08` — main brand color, used for primary interactive elements (buttons, focus rings, key accents)

### Secondary
- **Maroon** `#6c1e38` — secondary brand color

### Accents

Used sparingly to add visual variety to non-primary UI moments (label chips, info callouts, status indicators).

- **Pink** `#eb018b`
- **Yellow** `#fec702`
- **Sky** `#aae6f5`
- **Lavender** `#beaeed`
- **Green** `#95c13b`
- **Dark Teal** `#235151`

### Neutrals

- **Cream** `#f7e7db` — light neutral
- **Tan** `#c8ac92` — mid neutral
- **Near-Black** `#2b2a28` — dark neutral

---

## Theming

The app supports **dark** and **light** themes. **Dark is the default.** Users can toggle in Settings (Phase 2 feature).

Implementation: CSS custom properties for all color tokens, with `:root` (dark theme) and `[data-theme="light"]` overrides. Theme infrastructure ships in Phase 1 so we get both themes for free; the in-app toggle UI ships in Phase 2.

### Dark theme (default)

| Token | Color | Hex |
|---|---|---|
| `--bg-base` | Near-Black | `#2b2a28` |
| `--bg-surface` | Slightly lighter (derived) | `#3a3835` |
| `--bg-elevated` | Lighter still | `#4a4844` |
| `--text-primary` | Cream | `#f7e7db` |
| `--text-secondary` | Tan | `#c8ac92` |
| `--text-muted` | Tan @ 60% opacity | — |
| `--accent-primary` | Prodigy Orange | `#ff5c08` |
| `--accent-secondary` | Maroon | `#6c1e38` |
| `--border-subtle` | Near-Black @ lighter | `#4a4844` |
| `--focus-ring` | Prodigy Orange | `#ff5c08` |

### Light theme

| Token | Color | Hex |
|---|---|---|
| `--bg-base` | Cream | `#f7e7db` |
| `--bg-surface` | White | `#ffffff` |
| `--bg-elevated` | Pure white | `#ffffff` |
| `--text-primary` | Near-Black | `#2b2a28` |
| `--text-secondary` | Maroon | `#6c1e38` |
| `--text-muted` | Tan | `#c8ac92` |
| `--accent-primary` | Prodigy Orange | `#ff5c08` |
| `--accent-secondary` | Maroon | `#6c1e38` |
| `--border-subtle` | Tan @ 30% | — |
| `--focus-ring` | Prodigy Orange | `#ff5c08` |

### Label chips

Each label renders as a small pill with the `#` prefix. The chip's color is derived from the label name (stable hash → accent palette index), so the same label always gets the same color across the app.

Cycle order: Pink → Yellow → Sky → Lavender → Green → Dark Teal → (repeat).

Chip text color picked for contrast against the chosen accent.

---

## Typography

Pulled from the Prodigy RPG game's [Typography spec](https://prodigygame.atlassian.net/wiki/spaces/GD/pages/548831244/Typography). Two typefaces, both free Google Fonts.

### Typefaces

- **Display:** [Paytone One](https://fonts.google.com/specimen/Paytone+One) — heavy rounded sans for headers, button labels, titles. Distinctive RPG feel.
- **Body:** [ABeeZee](https://fonts.google.com/specimen/ABeeZee) — friendly geometric sans designed for reading. Used for body text, captions, helper text.

### Type scale

| Use | Font | Size / line height | Color token |
|---|---|---|---|
| Display Large (window/section headings) | Paytone | 32 / 40 | `--text-primary` |
| Display Small (subheads) | Paytone | 24 / 32 | `--text-primary` |
| Button | Paytone | 18 / 26 | depends on button variant |
| Body | ABeeZee | 16 / 24 | `--text-primary` |
| Caption / helper / timestamps | ABeeZee | 13 / 18 | `--text-secondary` |

Slightly tighter than the original RPG spec (which targets 1280×720 game canvas at scale). Adjusted for desktop reading distance.

### Rules

- **Sentence case is the default.** Caps are allowed when they genuinely help hierarchy — small caps for section headers, acronyms, brand-mark text. (We don't follow the RPG's strict "no all-caps" rule, which was for early readers.)
- **Minimum body size: 14px.** Below that, helper/caption only.
- **Color tokens, not literals.** Don't hardcode `#363636` — use `var(--text-primary)` so themes work.
- **Left-align longer text.** Centered only for short titles.
- **No animated text.** Movement makes reading harder.
- **WCAG AA contrast required.** All text on backgrounds must pass.

### Loading the fonts

Via Google Fonts CSS import in `index.html` or a global stylesheet:

```css
@import url('https://fonts.googleapis.com/css2?family=ABeeZee&family=Paytone+One&display=swap');
```

For offline-capable Tauri builds, we may self-host these later — both fonts have OFL licenses that permit redistribution.

---

## Iconography

Two icon systems work together: a comprehensive functional library and selected decorative pieces from the RPG game assets.

### Functional icons — Lucide

For all UI controls (settings, search, save, close, calendar, etc.), use **[Lucide](https://lucide.dev/)** via [`@lucide/svelte`](https://lucide.dev/guide/packages/lucide-svelte).

- ISC license (effectively free)
- 1,300+ icons covering every functional need
- Clean line-icon style that pairs with the RPG aesthetic without being a pixel-art clone
- SVG-based — scales and themes via CSS

#### Sizes

Standard web sizes (not the game's pixel grid):

| Size | Use |
|---|---|
| 16px | Inline within text |
| 20px | Compact UI (toolbar icons, label chips) |
| 24px | Default for most UI controls |
| 32px | Prominent buttons, headers |
| 48px | Hero moments, empty states |

#### Color rules

- Default: `var(--text-primary)` (matches body text)
- Hover/active: `var(--accent-primary)` (Prodigy Orange `#ff5c08`)
- Disabled: `var(--text-muted)`
- Decorative-only icons in section illustrations: any palette color

### Brand & decorative icons — RPG game assets

For branded moments (app icon, empty states, splash, achievements), pull from the RPG game source at `/Users/chris.carpenter/PROJECTS/Prodigy/Games/RPG/prodigy-game`. Confirmed-safe assets:

| Asset | Path | Use |
|---|---|---|
| Anchor | `assets/atlases/zone-pirate/anchor.png` | App icon / brand mark candidate |
| Book | `assets/atlases/general-icons/book.png` | Secondary mark, entry icon |
| Compass | `assets/atlases/ui-raster-epicsv2/taming-meter-Compass.png` | Empty state, navigation |
| Stamp (red) | `assets/atlases/ui-petslots/icon-stamp-red.png` | Achievement / timestamp |
| Wizard hat | `assets/single-images/icon/icon-hat/icon-hat-1.png` (530+ variants) | Subtle Prodigy-iconic accent |

**IP guidance:** generic UI elements (anchor, book, compass, stamps, generic hats, scrolls) are safe for an internal Prodigy tool. Character art (Mythics, pets, full NPC sprites) stays in the game.

When we use these, copy them into `app/assets/branded/` rather than hot-linking — keeps the app portable.

---

## Component patterns

Adapted from the [Prodigy RPG Components spec](https://prodigygame.atlassian.net/wiki/spaces/GD/pages/548569098/Components). The mechanical patterns we adopt:

### Buttons

- **Bottom-only drop shadow:** `0 4px 0 0 rgba(36, 20, 44, 0.5)` — the RPG signature move. Makes buttons feel physical, like a game piece. In light theme, use a maroon-tinted variant.
- **Press collapses the shadow:** on `:active`, button translates down by the shadow offset. Visual effect of the cap being pressed flat.
- **Three sizes:**
  - Small (height 36px) — toolbar / inline actions
  - Medium (height 48px, default) — most UI buttons
  - Large (height 56px) — primary CTAs, hero buttons
- **Sentence case labels**, strong verbs (`Save`, `Choose`, `Add note` — not `Save Changes`).
- **Primary action goes on the right** in modal footers; dismiss on the left. RPG convention. Opposite of macOS native, but we're aligning with Prodigy.
- **16px gap** between adjacent buttons.

### Spacing

- **4px grid.** All paddings, margins, and sizes are multiples of 4.
- Spacing scale tokens: `--space-1` (4px), `--space-2` (8px), `--space-3` (12px), `--space-4` (16px), `--space-6` (24px), `--space-8` (32px), `--space-12` (48px).

### Dialogs

- **Primary action right, dismiss left** — pick one dismissal pattern per dialog (Cancel button OR an X, never both).
- **Max widths:** 480px (confirmation), 600px (information), 720px (feature).
- **Title:** Display Small (Paytone 24px).
- **Body:** ABeeZee 16px, left-aligned.

### Toasts / notifications

- Top-center placement.
- Auto-dismiss after 3-5 seconds based on severity.
- Max 2 lines body copy.
- 24-32px icon at the start.
- No close button — auto-dismiss only.

### Inputs

- **Visible label always.** No label-as-placeholder (except a search field, which is unambiguous).
- **States to design:** Default, Hover, Focus, Error, Success, Disabled.
- **Error text below the input:** ABeeZee 13px in the error color, with a small icon prefix.

### Tabs

- Two sizes: compact (32px height) and standard (40px height).
- Active tab uses `--accent-primary`; inactive uses `--text-secondary`.
- Underline indicator below the active tab, 2px, in `--accent-primary`.

### Progress indicators

Two patterns for showing progress, depending on whether the work is discrete or continuous.

**Steppers — for 3 to 5 discrete named steps:**

- Use only when you have a fixed set of 3–5 named steps. For more steps or continuous progress, use a meter instead.
- States per step: not-started, current, completed (visually distinct).
- Arrangements: horizontal (left-to-right) or vertical (top-to-bottom with step labels beside).
- Sizes: small (20px icon nodes), large (40px icon nodes).
- Never use a stepper for a page load — that's the meter's job.

**Meters — for continuous progress:**

Color carries meaning:

- **Green** (`#95c13b`) — default; general task completion
- **Yellow** (`#fec702`) — levels, XP, accumulation toward a tier
- **Red / Maroon** (`#6c1e38`) — depletion, warnings, time running out

Sizes:

- Small: `height: 16px`, `min-width: 100px`, `max-width: 300px`
- Medium / Large: TBD when needed

Always pair the meter with a numeric value or label (beside, below, or on top of the bar).

### Open patterns to spec later

- Tooltips (delay, max width, dismissal)
- Empty states
- Loading / skeleton states
- Motion / animation timings
- Modals (beyond basic confirmation)

---

## Brand voice (for UI copy)

From the Prodigy brand:

- Progressive, imaginative, zealous, galvanizing
- Sage-Outlaw persona — driven by knowledge, willing to challenge defaults
- Conversational yet professional
- Pronouns (we, our, you), contractions, concise / friendly / benefit-driven
- Direct, not blunt; educational, not verbose

Apply this to: button labels, error messages, empty states, onboarding copy, settings descriptions, notification copy.

### Microcopy rules

From the Prodigy Writing for Kids guidelines (Confluence GD space), adapted for an adult professional context:

- **Consistent action verbs.** One verb per action across the app — don't mix "Save" and "Submit" for the same operation.
- **No internal jargon.** Don't put "Tauri," "IPC," "MCP" in user-facing copy. Plain language always.
- **Short headings + weight for hierarchy.** Concise heading text with weight differentiation beats long descriptive titles.
- **Break up long copy.** If a description runs more than ~100 words, add subheadings, lists, or visual breaks.
- **Sentence case** by default (per Typography rules).

---

## Open items

- Spacing scale finalization (do we need `--space-5` for 20px, or stick to the powers-of-2 cadence?)
- Motion / animation spec (timing functions, durations, easing curves)
- Full component spec library (Phase 2 work, as we build screens)
- Self-hosted font files for offline builds (currently Google Fonts CDN)
