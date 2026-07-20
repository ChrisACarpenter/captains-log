# Captain's Log — Style Guide

## How to use this guide

Captain's Log primarily uses the **Prodigy RPG game's visual language** — chunky drop-shadow buttons, Paytone One + ABeeZee fonts, gemstone-named button variants. This is what we build with for all main app UI.

The **Prodigy Corporate Brand** (the marketing-site aesthetic) is documented at the end as a fallback for occasional use — partner-facing popups, marketing-style splash screens, anywhere the RPG style might feel too playful for the context. **Don't use it for the main app UI**, but it's here in case we need it.

Both styles share the same color palette and the same brand voice. They differ in typography, button treatment, iconography, and overall density.

---

# Shared foundations

These apply across the entire app, regardless of whether a screen is in the primary RPG style or the corporate reference style.

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

## Theming

The app supports **dark** and **light** themes. **Dark is the default.** Users can toggle in Settings (Phase 2 feature).

Implementation: CSS custom properties for all color tokens, with `:root` (dark theme) and `[data-theme="light"]` overrides. Theme infrastructure ships in Phase 1 so we get both themes for free; the in-app toggle UI ships in Phase 2.

**Theme tokens — v2 (2026-06-23).** Refined after a six-lens adversarial critique of the original light/dark pair. Highlights of the v2 changes: warm-tinted neutrals (Embered direction), split border tokens (decorative orange-tinted vs structural warm-neutral), theme-aware Marble button, tokenized previously-hardcoded values (focus glow, sapphire bg, marble colors), WCAG 2.2 contrast fixes on light-mode focus ring and muted text, new `--bg-code` surface for the Phase 2 markdown editor, new `--stripe-track` / `--stripe-fill` for the week-progress stripe.

### Dark theme (default)

| Token | Hex / Value | Notes |
|---|---|---|
| `--bg-base` | `#2b2420` | Warm near-black with orange undertone |
| `--bg-surface` | `#36302c` | Warm dark surface — cards, inputs |
| `--bg-elevated` | `#3d3936` | Slightly cooler than surface — dropdowns, popovers |
| `--bg-code` | `#1f1a17` | Markdown code blocks (Phase 2) |
| `--text-primary` | `#f6e7d7` | Warm cream |
| `--text-secondary` | `#d2b094` | Warm tan |
| `--text-muted` | `#a89784` | Solid warm tan — passes AA on all dark surfaces |
| `--accent-primary` | `#ff5c08` | Prodigy Orange — fill/stroke; primary CTA bg |
| `--accent-primary-text` | `#ff7a3a` | Brightened orange for orange text on elevated dark surfaces |
| `--accent-maroon` | `#6c1e38` | Structural-accent only (active sidebar week, ruby button, button shadow tint). NOT a text color in dark mode |
| `--border-decorative` | `rgba(255, 92, 8, 0.14)` | Orange-tinted — decorative edges, chip borders, stripe track |
| `--border-structural` | `rgba(246, 231, 215, 0.10)` | Warm-neutral — cards, inputs, sidebar splits, panel rules |
| `--focus-ring` | `#ff5c08` | Prodigy Orange (passes 1.4.11 on dark) |
| `--focus-glow` | `rgba(255, 92, 8, 0.22)` | Tokenized — was hardcoded in 5 places before v2 |
| `--stripe-track` | `rgba(255, 92, 8, 0.14)` | Week-stripe unfilled track |
| `--stripe-fill` | `#ff5c08` | Week-stripe filled portion |
| `--btn-shadow` | `0 4px 0 0 rgba(0, 0, 0, 0.6)` | Signature RPG drop shadow |
| `--btn-marble-bg` | `#3a322c` | Theme-aware Marble (fixes the "invisible browse button" bug) |
| `--btn-marble-text` | `#f6e7d7` | — |
| `--btn-marble-border` | `rgba(255, 92, 8, 0.22)` | — |
| `--btn-sapphire` | `#3a82c8` | Tokenized — was hardcoded before v2 |

### Light theme

| Token | Hex / Value | Notes |
|---|---|---|
| `--bg-base` | `#f7e7db` | Documented Prodigy brand cream |
| `--bg-surface` | `#ffffff` | White — cards, inputs |
| `--bg-elevated` | `#fdf6ee` | Cream-tinted off-white — third surface tier for popovers/dropdowns |
| `--bg-code` | `#f4e3d2` | Markdown code blocks (Phase 2) |
| `--text-primary` | `#1c1612` | Warm near-black |
| `--text-secondary` | `#5a4438` | Warm neutral — maroon demoted out of body text after critique |
| `--text-muted` | `#7a5e48` | Darkened — passes AA on cream (5.0:1) and white (5.8:1) |
| `--accent-primary` | `#ff5c08` | Prodigy Orange — fill/stroke; primary CTA bg |
| `--accent-primary-text` | `#c44400` | Darkened orange for orange text on cream (4.6:1) — brand orange fails as text on light bg |
| `--accent-maroon` | `#6c1e38` | Structural-accent only — active sidebar week, ruby button, link hover |
| `--border-decorative` | `rgba(255, 92, 8, 0.18)` | Orange-tinted decorative edges, stripe track |
| `--border-structural` | `rgba(28, 22, 18, 0.12)` | Warm near-black at low opacity — cards, inputs, sidebar splits |
| `--focus-ring` | `#d94a00` | Darkened to pass WCAG 2.2 SC 1.4.11 (3.4:1 on cream) — brand orange itself fails 1.4.11 on cream |
| `--focus-glow` | `rgba(255, 92, 8, 0.22)` | — |
| `--stripe-track` | `rgba(255, 92, 8, 0.18)` | Week-stripe unfilled track |
| `--stripe-fill` | `#ff5c08` | Week-stripe filled portion |
| `--btn-shadow` | `0 4px 0 0 rgba(108, 30, 56, 0.32)` | Maroon-tinted shadow on light bg |
| `--btn-marble-bg` | `#ffffff` | Theme-aware Marble |
| `--btn-marble-text` | `#1c1612` | — |
| `--btn-marble-border` | `rgba(255, 92, 8, 0.22)` | Dropped from 0.35 — utility button shouldn't brand-shout |

**Token usage rules:**
- `--accent-primary` is fill/stroke only in light mode. For orange text use `--accent-primary-text`.
- `--accent-maroon` is structural-accent only — never used as body-adjacent text in dark mode.
- `--border-decorative` is for chips, decorative card edges, and the week-stripe track. Anything that defines a container boundary or affordance (cards, inputs, sidebar splits, panel rules) uses `--border-structural`.
- `--text-muted` is solid in both themes (the dark theme used to use an alpha-channel translucent value which baked in a contrast ceiling — v2 fix).

### Label chips

Each label renders as a small pill with the `#` prefix. The chip's color is derived from the label name (stable hash → accent palette index), so the same label always gets the same color across the app.

Cycle order: Pink → Yellow → Sky → Lavender → Green → Dark Teal → (repeat).

Chip text color picked for contrast against the chosen accent.

## Brand voice

From the Prodigy brand. Applies app-wide regardless of which style is in play:

- Progressive, imaginative, zealous, galvanizing
- Sage-Outlaw persona — driven by knowledge, willing to challenge defaults
- Conversational yet professional
- Pronouns (we, our, you), contractions, concise / friendly / benefit-driven
- Direct, not blunt; educational, not verbose

Apply to: button labels, error messages, empty states, onboarding copy, settings descriptions, notification copy.

---

# Primary style — Prodigy RPG game language

**This is what you build with.** All main Captain's Log UI uses these patterns.

The RPG style is sourced from the Prodigy game's design system — confirmed via the `ui-library` atlas (the canonical UI source, per Cale). Visual characteristics: chunky physical-feeling buttons, heavy display typography, gemstone-named color variants, 4px grid, sentence-case microcopy.

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

Self-hosted directly out of `app/static/fonts/` with plain `@font-face` rules in `app.css`. WOFF2 files were copied from the [Fontsource](https://fontsource.org/) `@fontsource/paytone-one` and `@fontsource/abeezee` npm packages, then the packages were uninstalled once the assets landed. Licenses ship next to the files (`LICENSE-paytone-one.txt`, `LICENSE-abeezee.txt`) — both fonts are OFL.

**Both Latin subsets, per face.** Two `@font-face` blocks per family:

- The **`latin`** subset carries A–Z, a–z, 0–9, common punctuation, and the Latin-1 Supplement block (most Western European accented characters). WOFF2s: `paytone-one-latin-400-normal.woff2`, `abeezee-latin-400-{normal,italic}.woff2`.
- The **`latin-ext`** subset carries harder-to-reach diacritics (Ā, ē, ġ, etc.) and a few extra typographic marks. WOFF2s: `paytone-one-latin-ext-400-normal.woff2`, `abeezee-latin-ext-400-{normal,italic}.woff2`.

Each `@font-face` block declares an explicit `unicode-range` so the browser knows which file supplies which glyph — the values match Google Fonts' canonical subset definitions. **Load-bearing:** an earlier iteration used the `latin-ext` subset alone; every basic-Latin character fell back to `system-ui` because the WOFF2 didn't contain those glyphs.

The static-folder route is served verbatim by SvelteKit's adapter-static, so at runtime the browser fetches `tauri://localhost/fonts/*.woff2`. No code-split chunk, no async CSS registration — the `@font-face` rules resolve on the same synchronous CSS load as the rest of the design tokens.

Why not `@fontsource/*` imports directly? Vite + adapter-static + Tauri put the resulting `@font-face` block into an async CSS chunk, and the WOFF2 URLs didn't resolve at the runtime asset layer inside Tauri's built .app — display type fell back to `system-ui`. The static-folder approach dodges every step of that chain.

## Iconography

Two icon systems work together: a comprehensive functional library and selected decorative pieces from the RPG game assets.

### Functional icons — custom Icon.svelte (Lucide-derived)

For all UI controls (formatting toolbar, info callouts, calendar, etc.), use the custom **`$lib/Icon.svelte`** component. It embeds hand-picked SVG paths cribbed from [Lucide](https://lucide.dev/)'s MIT-licensed set — 24×24 viewBox, 2px stroke, `currentColor` so consumers theme via CSS.

- No `@lucide/svelte` runtime dependency — bundle stays slim, icons inline as SVG
- Each icon is a named entry in the `IconName` union (`bold`, `italic`, `link`, `info`, `calendar`, etc.)
- Adding a new icon: extend the `IconName` union + add a `case 'name'` arm in Icon.svelte's template

The game's `EStandardIcons` set (Left, Right, Up, Down, Check, Close, Info, Spin, Shop, Build, Gift, Lock, Music, Play, Plus, Sparkle, Delete) maps cleanly to Lucide's catalog — port new ones into Icon.svelte as needed.

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

For branded moments (app icon, empty states, splash, achievements), pull from the RPG game source at `/Users/chris.carpenter/PROJECTS/Prodigy/Games/RPG/prodigy-game`. Canonical asset source: the `ui-library` atlas (per Cale). Confirmed-safe assets to draw from:

| Asset | Path | Use |
|---|---|---|
| Anchor | `assets/atlases/zone-pirate/anchor.png` | App icon / brand mark candidate |
| Book | `assets/atlases/general-icons/book.png` | Secondary mark, entry icon |
| Compass | `assets/atlases/ui-raster-epicsv2/taming-meter-Compass.png` | Empty state, navigation |
| Stamp (red) | `assets/atlases/ui-petslots/icon-stamp-red.png` | Achievement / timestamp |
| Wizard hat | `assets/single-images/icon/icon-hat/icon-hat-1.png` (530+ variants) | Subtle Prodigy-iconic accent |
| Noot (npc-noot-small) | `assets/atlases/_generated/ui-login-credentials/ui-login-credentials.png` @ `(3, 178, 235×226)` | Week-stripe reminder marker — see `/branded/noot-reminder.png` |
| Pointer hand | `assets/atlases/_generated/ui-guide-hands/ui-guide-hands.png` @ `(3, 3, 65×86)`, rotated 90° CW | Wizard input pointer — see `/branded/guide-hand.png` |

**IP guidance:** generic UI elements (anchor, book, compass, stamps, generic hats, scrolls) are safe for an internal Prodigy tool. Character art (Mythics, pets, full NPC sprites) stays in the game.

When we use these, copy them into `app/static/branded/` (SvelteKit's static-asset directory, served at `/branded/<file>`) rather than hot-linking — keeps the app portable.

## Component patterns

Adapted from the [Prodigy RPG Components spec](https://prodigygame.atlassian.net/wiki/spaces/GD/pages/548569098/Components), and refined via direct inspection of the game source (`src/ui/MathStandardButtonEnums.ts`, `src/ui/StandardButton.ts`).

> **Looking for what's actually shipping in the app?** This section codifies the *design language* — colors, sizing, gemstone button variants, layout grammar. The **implemented Svelte component library** (Modal, TipBubble, InputField, MarkdownEditor, etc.) lives at [`docs/components.md`](docs/components.md). That doc is the map; the canonical API contract for each component lives in the `<!-- ... -->` header comment at the top of its `.svelte` file.

### Buttons

#### Sizes

- **Small** — height 36px — toolbar / inline actions
- **Medium** — height 48px (default) — most UI buttons
- **Large** — height 56px — primary CTAs, hero buttons

(The RPG game itself ships 48 / 60 / 68px buttons. We scale ours down by 12px for desktop reading distance and information density — a deliberate departure documented for future reference.)

#### Drop shadow

- `0 4px 0 0 rgba(36, 20, 44, 0.5)` — the RPG signature move. Makes buttons feel physical, like a game piece. In light theme, use a maroon-tinted variant.
- On `:active`, the button translates down by the shadow offset. The element's visual height stays constant.
- The RPG game's actual press-collapse is 2px; we use 4px for stronger tactile feedback at desktop scale.
- **Disabled buttons have no shadow.** They render in the "stone" disabled treatment (see Variants) with the shadow removed entirely — visually pre-pressed.

#### Variants

Gemstone naming, ported directly from the game's `EStandardButtonType` enum:

| Variant | Use | Text color |
|---|---|---|
| `--btn-emerald` | Confirm, save, primary positive action | White (`#FFFFFF`) |
| `--btn-sapphire` | Primary navigation, general primary | White (`#FFFFFF`) |
| `--btn-ruby` | **Cancel, destructive, close.** Bind Esc / Backspace to Ruby buttons (the game does this via `AccessibleClose`). | White (`#FFFFFF`) |
| `--btn-marble` | Secondary / default neutral | Near-black (`#363636`) |
| `--btn-stone` | **Disabled treatment for ALL variants.** Single shared disabled state, not per-color. No shadow. | Greyed |

#### Labels and layout

- **Sentence case labels**, strong verbs (`Save`, `Choose`, `Add note` — not `Save Changes`).
- **Primary action goes on the right** in modal footers; dismiss on the left. RPG convention. Opposite of macOS native, but on-brand.
- **16px gap** between adjacent buttons.

### Spacing

- **4px grid.** All paddings, margins, and sizes snap to multiples of 4 — except for `--space-half`, the one deliberate below-grid primitive (see below).
- Spacing scale tokens:
  - `--space-half` (2px) — the only below-grid value. Reserved for tight-cluster gaps: toolbar icon rows, chip vertical padding, dense metadata rows. Don't use for section-level rhythm.
  - `--space-1` (4px), `--space-2` (8px), `--space-3` (12px), `--space-4` (16px), `--space-5` (20px), `--space-6` (24px), `--space-8` (32px), `--space-12` (48px)
- `--space-5` (20px) exists because LabelDetailsModal needed a value between 16 and 24 for its dense stat rows — a real gap in the scale, not a one-off.

### Radii

Tokenized corner radii, used across chrome, chips, buttons, and modals:

- `--radius-xs` (3px) — checkbox squares, cm-toolbar buttons, inline `code` markers
- `--radius-sm` (6px) — standard input + small chrome
- `--radius-md` (10px) — modal cards, pill buttons, panels
- `--radius-lg` (16px) — feature-height dialogs
- `--radius-full` (50%) — circular affordances (spinner, radio dot, floor-cat focus ring)
- `--radius-pill` (999px) — full-round pill buttons (day-pill, force-base-pill, tooltip)

### Motion

Motion is split into orthogonal **duration** and **easing** tokens so any duration can pair with any easing. Every transition callsite writes them in pairs: `transition: prop var(--duration-fast) var(--ease-standard);`.

**Durations:**

- `--duration-instant` (0ms) — focus ring, immediate state flips
- `--duration-fast` (120ms) — chrome hover/focus (buttons, chips, inputs)
- `--duration-base` (180ms) — elevated hover (modal chrome, cards)
- `--duration-reveal` (240ms) — modal / tooltip / toast enter
- `--duration-slow` (600ms) — data-driven state reveal (WeekStripe growth)

**Easings:**

- `--ease-standard` (ease-out) — every finite transition
- `--ease-loop` (linear) — infinite rotational animations (spinner)
- `--ease-oscillate` (ease-in-out) — attention pulses (PointerFinger bob)

**Reduced-motion:** modal fade-in respects `prefers-reduced-motion: reduce` (all mount animations drop to instant). Any new animation must do the same.

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

### Section banners

Horizontal 3-slice banners across the top of panels and dialogs — a recurring RPG motif (`ui-panels/title-bar-{left,middle,right}.png`, `ui-shared/banners/banner-red-*.png`). Worth a reusable pattern in Captain's Log for week dividers, section headers in the journal window, and dialog title strips.

- 3-slice composition: left cap + stretchable middle + right cap
- Sits flush with the panel/card edge above
- Title text in Paytone, centered or left-aligned
- In CSS, simplest implementation is a rounded-top container with a colored top band; for higher fidelity, an SVG with explicit slice insets

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

### Week Stripe

A 4px Prodigy-orange progress meter pinned to the top of the main window's content area (immediately under the system title bar). Implementation lives at `app/src/lib/WeekStripe.svelte`; rendered by `+layout.svelte` for every route except `/capture`.

- **Track** uses `--stripe-track` (low-opacity orange — `rgba(255,92,8,0.14)` dark / `0.18` light)
- **Fill** uses `--stripe-fill` (solid `#ff5c08`), width = `(now − Monday 00:00) / 7 days × 100%`
- Updates every 60 seconds — smooth growth across the week
- **Reminder marker (Noot):** when the user has a weekly reminder enabled, a small Noot mascot (`/branded/noot-reminder.png`, ~28px tall) hangs just below the stripe at the position corresponding to the reminder day/time. The image is `pointer-events: none` so it never blocks clicks.

Design intent (per the v2 critique): the stripe is *load-bearing* on day 1 — a real week-progress meter, not decorative chrome. It earns its position in the most HIG-sensitive 4 pixels of the window by carrying information. The Noot marker doubles as visual proof that your reminder is wired up.

Phase 2 considerations: when the journal browser ships with a sidebar, the stripe should span only the content pane (the sidebar gets its own week-list selection treatment using `--accent-maroon`). Window-state behavior (unfocused dim, fullscreen hide) is undefined for v2 and will be specced when Tauri window-state handling lands.

### Wizard guide hand

First-run setup uses a rotated `pointer-hand-straight` sprite (from the `ui-guide-hands` atlas) to point at the input the wizard is asking the user to fill in. Lives at `/branded/guide-hand.png` (extracted from upper-left of atlas, rotated 90° clockwise so the finger points right).

- Placed inside a `.guide-row` flex container, left of the active input
- ~36px tall, animates with a subtle 4px-amplitude horizontal "bob" every 1.6s (`--ease-oscillate`, infinite) to draw the eye toward the field
- Used on wizard steps 1 (Name), 2 (Location), 3 (Reminder) — not on the welcome step

### Open patterns to spec later

- Tooltips (delay, max width, dismissal)
- Empty states
- Loading / skeleton states
- Modals (beyond basic confirmation)
- Week-stripe window-state behavior (unfocused, fullscreen, Phase 2 sidebar geometry)

## Microcopy rules

From the Prodigy RPG game's Writing for Kids guidelines (Confluence GD space), adapted for an adult professional context:

- **Consistent action verbs.** One verb per action across the app — don't mix "Save" and "Submit" for the same operation.
- **No internal jargon.** Don't put "Tauri," "IPC," "MCP" in user-facing copy. Plain language always.
- **Short headings + weight for hierarchy.** Concise heading text with weight differentiation beats long descriptive titles.
- **Break up long copy.** If a description runs more than ~100 words, add subheadings, lists, or visual breaks.
- **Sentence case** by default (per Typography rules).

---

# Reference — Prodigy corporate brand

The marketing-site aesthetic. **Not for main app UI.** Documented here for the rare moments we might need it.

## When to use

- Partner-facing popups (e.g., a parent or teacher landing on a Captain's Log export)
- Marketing-style modals or splash screens (a polished launch screen, perhaps)
- Anywhere the RPG style might feel too playful for the audience

If none of these apply, use the primary RPG style.

## Visual characteristics

Pulled from analysis of [prodigygame.com/main-en/teachers](https://www.prodigygame.com/main-en/teachers) and the Prodigy Deck Template:

- **Generous whitespace**, clear typographic hierarchy
- **Solid filled buttons** (no chunky drop shadow — flat modern style)
- **Modern illustrated SVG icons** (filled, simple, modern line-weight)
- **Large headlines** followed by descriptive subtext
- **Font family:** not extractable from the marketing site (Webflow CDN hides the CSS). Use system fonts as fallback until identified.

## What stays consistent with the primary style

- Colors (same Prodigy Orange, Maroon, accents, neutrals)
- Brand voice (Sage-Outlaw — applies to both styles)
- Sentence case
- WCAG AA contrast requirements
- 4px grid (for spacing)

## What's different from the primary style

| Aspect | Primary (RPG) | Reference (Corporate) |
|---|---|---|
| Button style | Chunky `0 4px 0 0` drop shadow, gemstone variants | Flat solid filled |
| Display font | Paytone One | TBD (system fallback) |
| Body font | ABeeZee | TBD (system fallback) |
| Iconography | Lucide line icons + RPG assets | Filled illustrated SVGs |
| Density | Compact, game-piece feel | Generous whitespace, marketing feel |

---

## Open items

- Corporate font identification (if we ever need the reference style)
