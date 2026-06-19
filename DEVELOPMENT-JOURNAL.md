# Captain's Log — Development Journal

Append-only log of decisions, progress, and open questions. Most recent entries at the bottom.

---

## 2026-06-18 — Project kickoff

Started the planning conversation. Captured the problem statement, feature wishlist, and architectural direction.

### Decisions made

- **Project name:** Captain's Log
- **Location:** `/Users/chris.carpenter/PROJECTS/Prodigy/CaptainsLog/`
- **Tech stack direction:** Tauri 2.0, cross-platform from day one (Mac first), web frontend
- **Storage:** Plain markdown on disk
- **File organization:** `journals/YYYY/YYYY-Www.md` — year folders, one file per ISO week
- **Metadata:** `journals/.metadata/labels.json`, `journals/.metadata/settings.json`
- **Sync:** Local-only for v1. Abstract the storage layer so Google Drive sync can plug in later. Encryption can layer on top of either backend.
- **Vocabulary:** "Note" = single timestamped entry, "Weekly Notes" = section that holds them, "Weekly Summary" = optional 4-field structured summary using the Lattice template
- **Quick capture:** Menu bar icon, 2-click flow (open → submit)
- **Icons:** Menu bar (quick capture) AND dock (opens full journal window)
- **Label model:** Two input paths feeding one index
  - Dedicated Labels field (JIRA-style autocomplete, never asks user to type `#`)
  - Inline `#hashtags` in body text (also autocomplete-aware)
  - Both contribute to the file's frontmatter aggregate and to `labels.json`
  - Self-cleans when usage drops to zero
- **First-run setup:** Name, journal storage location, reminder preference (and time if on)
- **Weekly reminder notifications:** Optional, user-configurable day and time
- **Performance Review module:** Phase 5 — bundles a date range of Notes + Summaries into a single markdown file plus instructions for an LLM. Assumes MCP connectors are available for link enrichment (Jira, GitHub, Slack, Confluence).

### Why these decisions

- **Tauri over Electron:** ~5MB vs ~150MB bundle, uses native WebKit on macOS, smaller memory footprint, modern choice in 2026.
- **Year-based file organization:** Universal mental model (review periods are Prodigy-specific). Date-range export works regardless of folder structure.
- **Two label input paths:** Users will hashtag naturally in prose (social media convention); they also want a dedicated field for deliberate tagging. Both flow into the same index, so UX is consistent regardless of where the user enters labels.
- **Labels on a dedicated line below the entry heading (not in frontmatter only):** Keeps the human-readable markdown readable. Labels added via the field go in `**Labels:**`; labels added via inline hashtags stay in the body. Both contribute to the file frontmatter aggregate.
- **Markdown as source of truth:** No vendor lock-in; users can edit in any external tool; the journal travels with the user; the LLM bundle export is trivial.

### Open questions / deferred

- **Brand style guide:** Prodigy has a deck (`docs.google.com/presentation/d/1Cmau...`) with named colors but no extracted hex codes. Slide 4 has the actual swatches. Chris will read off the hex codes when convenient. Not blocking.
- **Font family:** Not specified in the deck. Using system defaults until we have an answer.
- **Confluence brand guide search:** Run on 2026-06-18. Only hits were 2019-2020 Design System meeting notes, all by deactivated contributors. No active brand guide exists; the deck is the most current source.
- **Web frontend framework:** TBD (React vs Svelte vs SolidJS). Decide at the start of Phase 1.
- **Markdown editor library:** TBD (CodeMirror 6 vs TipTap vs Milkdown). Decide at the start of Phase 2 work.

### Next session

- Pick a web frontend framework
- Sketch the UI for quick capture and the journal window
- Set up the Tauri project structure inside `CaptainsLog/app/`

---

## 2026-06-18 (later) — Scaffolding complete; framework picked; colors locked

### Done

- Folder structure created at `/Users/chris.carpenter/PROJECTS/Prodigy/CaptainsLog/`
- All planning docs written: README, ROADMAP, DESIGN, DEVELOPMENT-JOURNAL, STYLE-GUIDE, and the `docs/` subfolder (data-format, label-system, file-structure, first-run-setup, ux-flows)
- `.gitignore` in place
- Git repo initialized

### Decisions made

- **Frontend framework: Svelte 5**
  - **Why:** Smaller bundle than React (good Tauri citizen), less boilerplate, reactivity model simpler than React hooks, forms-heavy nature of this app plays to Svelte's strengths. CodeMirror 6 works fine in Svelte.
  - **Trade-off:** Smaller ecosystem than React. Acceptable for a focused single-purpose app.
  - **Can be revisited** before any code is written if there's a strong reason.
- **Brand colors locked** — pulled from the Prodigy deck (slide 4) into [STYLE-GUIDE.md](STYLE-GUIDE.md). Twelve colors total: primary (orange), secondary (maroon), six accents, three neutrals.
- **Theming:** App supports dark and light themes; **dark is the default**. Theme infrastructure (CSS variables) is a Phase 1 task so both themes are available early; the in-app toggle UI lands in Phase 2.

### Still open

- **Typography:** No font family identified. Using system defaults (-apple-system, SF Pro on Mac) for now.
- **Component and spacing standards:** Will codify as we design specific UI elements.
- **Editor library:** Leaning CodeMirror 6, final pick at start of Phase 2.

### Next session

- Pick a Tauri project structure (single window vs multi-window setup)
- Spin up the actual Tauri app shell inside `CaptainsLog/app/`
- Get a "Hello, Captain's Log" window appearing in dark mode using the new color tokens

---

## 2026-06-19 — Brand styling deep dive

Major brand/style work today. Pivoted from "match Prodigy's corporate marketing brand" to "match the Prodigy RPG game's visual language." More distinctive, more fun, better fit for a homebrew internal tool.

### Sources mined

- **Components spec** (Confluence GD/548569098) — buttons, dialogs, inputs, toasts, tabs, tooltips, pagination, progress meters
- **Typography spec** (Confluence GD/548831244) — Paytone One + ABeeZee, type scale, color rules
- **Iconography spec** (Confluence GD/659065735) — UI vs sprite icons, sizes, two-color rule
- **RPG game source** at `Prodigy/Games/RPG/prodigy-game` — anchor, book, compass, stamps, wizard hats for branded moments
- **UI Library page** (Confluence GD/3930357789) — deeper dive in progress via subagent at time of writing

All Confluence specs are from 2019 with deactivated authors, but the systems shipped in game and the patterns are sound. We treat them as aesthetic direction, not pixel-perfect specs.

### Decisions made

- **Fonts: Paytone One (display) + ABeeZee (body).** Both free Google Fonts. Single biggest brand signal — instantly Prodigy.
- **Functional icons: Lucide.** 1,300+ clean line icons via `@lucide/svelte`. Pairs with the RPG aesthetic without being pixel-art.
- **Brand/decorative icons: selected RPG assets.** Anchor, book, compass, stamps, wizard hats. Copy into `app/assets/branded/` when we use them.
- **All-caps prohibition dropped.** The RPG rule (no caps anywhere) was for early readers. Adults are fine with capitals where hierarchy benefits.
- **Bottom-only drop shadow (`0 4px 0 0`) is the signature button move.** Most distinctive RPG visual language piece — port directly. On press, the shadow collapses (button translates down by the offset).
- **Primary action on right** in dialogs (RPG convention, opposite of macOS native, but on-brand).
- **4px spacing grid** with token scale.

### STYLE-GUIDE.md updated

Major rewrite. Typography, Iconography, and Component Patterns sections all filled in. Open items list shrunk to spacing finalization, motion spec, and full component library.

### Still open

- Spacing scale finalization
- Motion / animation spec (timings, easing)
- Full component spec library (Phase 2 work as we build screens)
- Self-hosted font files for offline builds

### UI Library deep dive — findings

Subagent scanned the UI Library Confluence page (GD/3930357789) and all 30 descendants. Verdict: 2019 ghost-town with a 2025 sticky note on the door. Most pages are empty drafts; every substantive author is deactivated.

Three patterns worth adopting (added to STYLE-GUIDE.md):

- **Stepper vs Meter distinction** — 3–5 discrete named steps → stepper; continuous or longer sequences → meter
- **Meter color semantics** — green = done, yellow = accumulation/levels, red = depletion/warnings
- **Microcopy rules** from the Writing for Kids spec — consistent verbs, no internal jargon, short headings, sentence case

Everything else (checkboxes, radios, scroll, notifications, colour, spacing) was either empty, trivially obvious, or game-asset-specific.

**The RPG Confluence component library is not a source of truth going forward.** Treat what we've harvested over the last two days as the final pull. New patterns go directly into STYLE-GUIDE.md.

### Next session

- Spin up the Tauri app shell
- Implement theme infrastructure (CSS variables for both themes + the Paytone/ABeeZee imports)
- Build a Hello-World button with the signature bottom-drop-shadow pattern as proof of concept

---

## 2026-06-19 (later) — Game code investigation per Cale's suggestion

Cale (Prodigy lead UX) suggested digging into the actual game code to see how UI atlases are referenced and used. Subagent investigated the RPG game source. Tech stack confirmed: **TypeScript + PixiJS + Webpack**.

### Validated existing decisions

- Paytone + ABeeZee fonts (confirmed in `src/ui/legacy/LegacyTextStyles.ts` and `MathStandardButtonEnums.ts`)
- Gemstone variant naming (`emerald`, `sapphire`, `ruby`, `marble`)
- Three button sizes
- 4px grid (button padding, icon sizes 20/32)
- Lucide-friendly functional icon set (game's `EStandardIcons` enum maps 1:1)

### Two atlas systems coexist in the game

- **Legacy** `ui-buttons` atlas — 3-slice horizontal strips (`*-left`, `*-middle`, `*-right`)
- **New** `ui-library` atlas — single PNG per button with Pixi `sliceData` metadata (true 9-slice)

The new `ui-library` system is what we target visually. Worth asking Cale next week which the UX team considers canonical for new work.

### New findings, all integrated into STYLE-GUIDE.md

- **Stone is the canonical disabled treatment.** Single shared state across all gem variants — not a tinted variant. No shadow. Visually pre-pressed.
- **Ruby is for cancel/destructive only.** Esc/Backspace bind to Ruby buttons via `AccessibleClose`.
- **Marble uses dark text (`#363636`); all other gem buttons use white.** Documented per-variant.
- **Section banners are a recurring 3-slice motif.** Added as a reusable pattern (`title-bar-*`, `banner-red-*`).
- **Button sizes:** game ships 48/60/68; we keep 36/48/56 as a deliberate desktop adjustment. Documented in the Buttons section.
- **Shadow offset:** game uses 2px (active frame is 2px shorter than default); we keep 4px for stronger tactile feedback at desktop scale. Documented.

### Resolved: ui-library is the canonical source

Confirmed with Cale: **`ui-library`** (the new 9-slice approach with Pixi `sliceData` metadata) is the canonical UI system going forward. The legacy `ui-buttons` 3-slice strips are being phased out. Captain's Log targets the `ui-library` visual language for any future asset references.

---

## 2026-06-19 (later still) — STYLE-GUIDE restructure

Reorganized `STYLE-GUIDE.md` so the RPG game style is clearly the primary visual language and the Prodigy corporate brand is clearly demarcated as reference-only.

New structure:

1. **How to use this guide** — preamble explaining the split
2. **Shared foundations** — colors, theming, label chips, brand voice (apply across both styles)
3. **Primary — Prodigy RPG game language** — typography, iconography, component patterns, microcopy (what we build with)
4. **Reference — Prodigy corporate brand** — marketing-site aesthetic, with explicit "when to use" criteria and a delta table

The split makes it obvious which patterns to reach for when building the app, and gives us a documented fallback for partner-facing popups or marketing-style moments without polluting the primary spec.
