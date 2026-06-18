# Captain's Log — Style Guide

Brand colors, theming, and visual standards for the app.

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

## Label chips

Each label renders as a small pill with the `#` prefix. The chip's color is derived from the label name (stable hash → accent palette index), so the same label always gets the same color across the app.

Cycle order: Pink → Yellow → Sky → Lavender → Green → Dark Teal → (repeat).

Chip text color picked for contrast against the chosen accent.

## Typography

Not yet specified — placeholder for v1. Using OS system fonts:

- **macOS:** -apple-system, SF Pro
- **Windows:** Segoe UI
- **Linux:** Cantarell or system default

A specific Prodigy font will be slotted in when one is identified.

## Iconography

The Prodigy Deck Template (slides 80+) has an icon library worth pulling from for visual consistency. As we design specific UI elements, prefer icons from this library.

To be expanded as we design specific components.

## Brand voice (for UI copy)

From the Prodigy deck:

- Progressive, imaginative, zealous, galvanizing
- Sage-Outlaw persona — driven by knowledge, willing to challenge defaults
- Conversational yet professional
- Pronouns (we, our, you), contractions, concise / friendly / benefit-driven
- Direct, not blunt; educational, not verbose

Apply this to: button labels, error messages, empty states, onboarding copy, settings descriptions, notification copy.

## Open items

- Typography pick (font family for body and headings)
- Component spec library (buttons, inputs, modals — sizes, states, motion)
- Spacing scale (4px / 8px grid?)
- Iconography set selection
