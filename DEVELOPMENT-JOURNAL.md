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

---

## 2026-06-22 — Phase 1 kickoff: Tauri scaffold + Svelte frontend

Started Phase 1. Environment was almost ready (Node 25, Homebrew, Xcode CLT all present) but **Rust wasn't installed** — installed via rustup, stable toolchain (rustc 1.96.0).

### Scaffold

Used `create-tauri-app@latest` to bootstrap inside `CaptainsLog/app/`:

```
npx -y create-tauri-app@latest app --manager npm --template svelte-ts --identifier com.prodigygame.captainslog
```

Template choice: **svelte-ts**, which gave us SvelteKit 2 + Svelte 5 + TypeScript + Vite + `adapter-static` for SPA output. Kept SvelteKit (vs raw Svelte) — its file-based routing is a clean fit for multi-window scenarios (main journal at `/`, quick capture at `/capture`, first-run at `/setup`).

### npm registry override

Chris's `~/.npmrc` defaults to Prodigy's AWS CodeArtifact registry (with an expired auth token). Public packages — Tauri, Svelte, Lucide — all live on the public registry. Added `app/.npmrc` to pin this project to `https://registry.npmjs.org/`. Project-level config wins over user-level. Committed.

### Renames and customization

Scaffold defaulted everything to `app`. Renamed to match the project:

- `src-tauri/Cargo.toml` → `name = "captainslog"`, lib name `captainslog_lib`, real description and author
- `src-tauri/src/main.rs` → calls `captainslog_lib::run()`
- `src-tauri/src/lib.rs` → removed the placeholder `greet` command, added a module-layout comment for future code
- `src-tauri/tauri.conf.json` → `productName: "Captain's Log"`, window title, dimensions (1200×800, min 800×500), main window labeled `"main"`
- `package.json` → `name: "captainslog"`, description
- `app/README.md` → replaced the scaffold's generic template with a dev guide (stack, layout, setup, troubleshooting)

### Verified

- `cargo check` — passes (29.5s first-time download/compile)
- `npm run check` — passes (134 files, 0 errors, 0 warnings)

### Not yet running

Haven't done `npm run tauri dev` yet — first launch would open an empty Hello-World window. Saving that until after theme infrastructure lands so the first thing we see has actual Captain's Log styling.

### Next

- Theme infrastructure (CSS variables for dark + light, Google Fonts for Paytone + ABeeZee)
- Storage layer in Rust (StorageBackend trait + LocalFilesystem)
- First Tauri command (create_note)
- Quick capture popup window

---

## 2026-06-22 (later) — Phase 1 first vertical slice end-to-end

Theme → storage → notes API → create_note command → capture form → tray icon. The pipeline is wired top to bottom.

```
Svelte form → invoke('create_note') → Tauri command →
notes::append_note → LocalFilesystem → ~/Documents/CaptainsLog/YYYY/YYYY-Www.md
```

### What got built today

- **Theme infrastructure** — `src/app.css` with CSS variables for dark (default) and light themes, full palette tokens, 4px spacing scale, type scale, motion tokens, Paytone One + ABeeZee imports from Google Fonts. Signature `.btn` class with bottom drop shadow and press-collapse behavior + gemstone variants (`.btn-emerald`, `.btn-sapphire`, `.btn-ruby`, `.btn-marble`).
- **storage.rs** — `StorageBackend` trait + `LocalFilesystem` impl. Full CRUD over weekly files (`YYYY/YYYY-Www.md`) and metadata files (`.metadata/labels.json`, `.metadata/settings.json`). 15 unit tests covering roundtrips, missing files, invalid weeks, malformed filenames.
- **notes.rs** — `Note` struct, markdown serializer matching `docs/data-format.md`, ISO 8601 week math (including cross-year boundary), weekly-file scaffold generator, `append_note`. 13 unit tests.
- **commands.rs** — `create_note` and `read_week` Tauri commands using `tauri::State<LocalFilesystem>`.
- **Capture form** (`src/routes/+page.svelte`) — title, body, labels fields. Cmd+Enter submit. Inline saved/error status. Styled with theme tokens.
- **Tray icon** — left-click toggles main window visibility. Built with `tauri::tray::TrayIconBuilder` (required enabling the `tray-icon` feature on the tauri crate). Uses default app icon as placeholder.

### Verified

- Backend: 28/28 unit tests passing (`cargo test --lib`)
- Frontend: `svelte-check` clean (135 files, 0 errors)
- `cargo check` passes for the lib + tray-icon feature
- `npm run build` produces a clean static SPA

### Needs manual verification (when Chris is back at his machine)

Can't run `npm run tauri dev` from here (opens an interactive window). When you're back:

```bash
cd /Users/chris.carpenter/PROJECTS/Prodigy/CaptainsLog/app
npm run tauri dev
```

Checklist:
1. Window opens at 1200×800, dark theme, Paytone heading
2. Capture form: enter title + body + labels, click Submit
3. Status shows "Captured. Note written to this week's file."
4. Verify `~/Documents/CaptainsLog/2026/2026-W26.md` exists with frontmatter, Weekly Summary scaffold, Weekly Notes section, and the captured note
5. Capture another note — should append to the same file
6. Click the tray icon (top of menu bar) — window should hide
7. Click again — window should show and focus
8. Cmd+Enter from the capture form should also submit

### Architectural decisions

- **Async-throughout storage trait.** Tauri commands are async; future GoogleDrive backend will be async; `tokio::fs` is the natural fit. No regret on the boilerplate.
- **LocalFilesystem as concrete `State<>`, not `Box<dyn StorageBackend>`.** Phase 1 has one backend; dyn dispatch adds friction with no current benefit. Refactor to a trait-object state if we ever switch backends at runtime.
- **Frontmatter not re-written on note append.** `last_modified` is set once at file creation. Full file parsing + frontmatter updates come in Phase 2 when we actually need them.
- **Timestamps captured server-side.** `chrono::Local::now().fixed_offset()` in the Rust process, not the frontend, so clock skew can't drift the journal.
- **Journal root hardcoded** to `~/Documents/CaptainsLog/`. First-run setup (writing `settings.json`) will replace this in a follow-up.

### Known Phase 1 limitations (Phase 2 fodder)

- Single window — no dedicated quick-capture popup yet. Main window IS the capture surface for now.
- No label autocomplete (plain comma-separated input).
- No UI to view past notes.
- No first-run setup flow (journal root hardcoded).
- Default app icon used in tray (works, not on-brand). Anchor/compass template image to come.

### Commits today

1. `ec7e026` — Phase 1 kickoff: scaffold Tauri 2.0 + SvelteKit app
2. `00b8ca8` — Theme infrastructure: tokens, fonts, signature button class
3. `ab432e8` — Phase 1 core: storage layer, notes API, capture command + UI
4. `e6efd98` — Tray icon + journal sync
5. (this commit) Label index module

### Label index (added after the initial Phase 1 milestone)

- `labels.rs` — `LabelIndex` (versioned), `LabelEntry`, `extract_all_labels(note)` covering both the labels field and inline `#hashtags` in body, `record_note_labels` integration helper. 19 unit tests.
- `commands.rs::create_note` now updates `.metadata/labels.json` after a successful note save (best-effort — label index failure logs a warning but doesn't fail the user's note save).
- `storage.rs` — added `StorageError::Serde` variant for JSON serialization errors.
- Sorts entries by `last_used` desc, then `count` desc, then alphabetical (per `docs/label-system.md`'s autocomplete ranking).
- Corrupt `labels.json` falls back to empty + warns to stderr. Full rebuild-from-scan is Phase 2.

### Test totals at end of session

- Backend: **47/47 unit tests passing** (15 storage + 13 notes + 19 labels)
- Frontend: `svelte-check` clean (135 files, 0 errors)

### Next session

- Manual end-to-end test (above checklist)
- Split quick capture into its own popup window (label `"capture"`, route `/capture`, smaller dimensions, tray opens it instead of main window)
- First-run setup flow (writes `settings.json`, lets user pick journal location)
- Replace tray icon with a proper macOS template image (anchor from RPG assets, recolored to black-with-alpha)
- Label autocomplete UI (Phase 2) — the index is in place; the UI hooks come when we wire CodeMirror or the JIRA-style chip input

## 2026-06-22 → 2026-06-23 — Phase 2 sprint: daily-driver polish

Two-day push from "captures notes" to "actually usable as a journal." Every Phase 2 item that doesn't require a journal browser is done; production .app builds work; macOS notifications fire with action buttons; the close flow follows native conventions. Roughly 40 commits over the window — git log is the granular history; this entry captures the arcs.

### What landed

**Setup + persistence**

- First-run wizard (`/` route) with 4 steps — welcome, name, location, reminder — and a small RPG `pointer-hand-straight` sprite that bobs gently next to each input (extracted from `ui-guide-hands` atlas, rotated 90° CW).
- Two-tier settings: `app-settings.json` per-machine (journal root + theme) lives in `~/Library/Application Support/`; `settings.json` per-journal (name + reminder) lives in `<journal>/.metadata/`. So the journal can move between machines later (Phase 6 sync) without dragging machine-specific config along.
- `/settings` route with full edit form. Theme toggle previews live (no save needed); reminder + name persist on Save; `LocalFilesystem` hot-swaps in-process when journal_root changes (no app restart).
- `settings-changed` event broadcast from `update_settings` / `complete_first_run` — `+layout.svelte` listens to re-apply theme on the capture popup (separate webview), and `WeekStripe` listens to make Noot appear/disappear immediately after a reminder toggle.

**Theme v2 — Embered + Week Stripe**

- Both themes redesigned after a six-lens adversarial critique (aesthetic / accessibility / brand / macOS-native / daily-wear / Phase-2 fit). Warm-tinted neutrals across both modes, split `--border-decorative` (orange) vs `--border-structural` (warm-neutral), WCAG 2.2 contrast fixes on the light focus ring and `--text-muted`, new `--bg-code` surface waiting for the Phase 2 markdown editor.
- The Week Stripe: 4px Prodigy orange progress meter at the top of the main window. Track + fill, grows across the week (days elapsed / 7), updates every 60s. Earns its position by being load-bearing, not decoration.
- Noot reminder marker: when a reminder is configured, a small `npc-noot-small` mascot from the `ui-login-credentials` atlas hangs on the stripe at the reminder day/time position. Visual proof the reminder is wired.
- Hardcoded values (focus glow ×5 instances, sapphire bg, marble colors, emerald button text) all tokenized. Marble button got theme-aware tokens — fixes the original "Browse button vanishes in light mode" bug that triggered the redesign.

**Weekly Summary UI**

- `/summary` route — the 4-field Lattice-style form (Key accomplishments / Plans / Challenges / Anything else). Cmd+S / Cmd+↩ to save. Adds a `### Labels` subsection at the end of the Weekly Summary markdown for tagging the week itself (separate from per-note labels).
- Backend: `WeeklySummary` struct + `parse_weekly_summary` / `render_weekly_summary` / `replace_weekly_summary_in_file` (preserves Notes below the summary block), `get_weekly_summary` / `update_weekly_summary` commands, 7 new tests including roundtrip.

**Label autocomplete**

- Chip-based JIRA-style input (`lib/LabelInput.svelte`). Live dropdown filtered as you type, stable per-label color from the accent palette (djb2 hash → palette index), arrow-key nav, Enter / Tab / Comma / Space to commit, Backspace on empty input removes the last chip, "Create new label" option for novel tags. Fed from `labels.json`, optimistically appends new labels to the in-memory pool. Wired into both `/capture` and `/summary`.
- `labels.json` schema migrated to camelCase with snake_case `serde(alias)` for backwards compat.

**Window lifecycle + persistence**

- `tauri-plugin-window-state` for size/position memory across launches. `VISIBLE` flag dropped from the default state set + `skip_initial_state("capture")` because the plugin's restore would otherwise force-show the capture popup on every launch, overriding tauri.conf.json's `visible: false`. Took two debug rounds to land — the failure mode was non-obvious (popup opened on every launch + tray clicks couldn't re-open it after red-X).
- Capture window red-X / Cmd-W is intercepted (`api.prevent_close()` + `hide()`) so the WebviewWindow handle stays alive for the tray to keep toggling. Without this, the OS destroys the window and `get_webview_window("capture")` returns None forever.

**Notification odyssey**

The most circuitous arc of Phase 2. Sequence:

1. **Started with `tauri-plugin-notification`** — the cross-platform Tauri plugin. Found out the hard way that `NotificationAction` / `ActionType` are gated behind `#[cfg(mobile)]` in the desktop builder. No action buttons on macOS desktop from the plugin.
2. **Integrated `mac-notification-sys`** directly for action buttons — `MainButton::SingleAction("Write")` + `close_button("OK")` + `app_icon(scroll_path)`. Notifications fired, scroll icon rendered correctly — but action buttons didn't show and the banner auto-dismissed in ~5s. Diagnosis: `mac-notification-sys` wraps the deprecated `NSUserNotification` API; modern macOS makes "Alert Style" a per-app user preference (Banners auto-dismiss, Alerts persist + show buttons).
3. **Migrated to `mac-usernotifications`** (modern `UNUserNotificationCenter` wrapper, same author as the deprecated crate — explicit successor). Production .dmg built — but it CRASHED on first reminder save in dev mode. Cause: `UNUserNotificationCenter.current()` internally calls `bundleProxyForCurrentProcess` (LaunchServices), which aborts when running from a bare binary. NSBundle swizzling (works for the legacy API) doesn't satisfy this deeper lookup.
4. **Hybrid path** — `is_running_in_app_bundle()` runtime check via `current_exe()`. Production `.app` → UN. `tauri dev` bare binary → fall back to the deprecated NSUserNotification (no crash, no buttons, banner auto-dismiss). Both paths share the bundle-id NSBundle swizzle since the dev path needs it.
5. **Built the .dmg** — installed, no notifications at all, app didn't appear in System Settings. Multi-lens investigation found the root cause: rustc's linker-only signature gave the binary `Identifier=captainslog-9fa46538b1a63b55` (auto-generated), not `com.prodigygame.captainslog`. macOS Sequoia/Tahoe's `usernotificationsd` keys notification permission off the codesign Identifier — when it doesn't match the bundle id, every UN auth call gets silently denied with `Entitlement 'com.apple.private.usernotifications.bundle-identifiers' required`. Fixed by adding `bundle.macOS.signingIdentity = "-"` to tauri.conf.json (runs a real `codesign --force --sign - --identifier com.prodigygame.captainslog` pass after the linker) + a custom `src-tauri/Info.plist` with `NSUserNotificationAlertStyle=alert` and `ITSAppUsesNonExemptEncryption=false`.
6. **One residual UX nit:** even with `Alert` set in Info.plist, macOS still defaults new apps to "Temporary" notifications which auto-dismiss and hide action buttons behind hover. Sequoia ignores the older key for UN-only apps. Added an in-app settings hint with a one-click deep link (`x-apple.systempreferences:com.apple.preference.notifications?id=com.prodigygame.captainslog`) that takes the user straight to the right preference panel. Opener plugin needed an explicit scope (`opener:allow-open-url` with the `x-apple.systempreferences:*` URL pattern).

**Close flow — Option B**

- Red X on main window now **hides** instead of destroys, AND switches the app to `NSApplication.Accessory` activation policy. Dock icon disappears; tray stays. App lives in the menu bar between sessions.
- Tray icon left-click still toggles the capture popup. Right-click shows a native context menu with **Show Captain's Log** (restores `.Regular` + Dock icon + main window) and **Quit Captain's Log**.
- `enable_macos_default_menu(false)` + custom app menu with a `MenuItem`-style Quit (not `PredefinedMenuItem::quit` — that one dispatches AppKit's `terminate:` directly and bypasses our event listener). Tray's Quit and Cmd+Q share `QUIT_MENU_ID` so one `on_menu_event` arm handles both.
- `DirtyRegistry` Tauri-managed state — each form-bearing route publishes its dirty state via a `set_window_dirty` invoke command. `try_quit` reads the snapshot at quit time; if anything's dirty, native NSAlert with Cancel-as-default per HIG, listing affected surfaces ("the weekly summary and the quick-capture note").
- Frontend `lib/dirty.ts` helper — edge-triggered + 150ms-debounced push from a single `$effect`, `onDestroy` clears the bit on route unmount.
- Notification "Write" action now routes through `restore_main_window` (sync activation policy → show → focus) so opening from a notification while the app is in `.Accessory` mode brings the Dock icon back. Otherwise the window would appear in a half-state (visible, no Dock, Cmd-Tab-invisible).

### What was tougher than expected

- **The notification odyssey.** Five distinct failures in sequence (no action buttons → no buttons + auto-dismiss → bare-binary crash → no permission in dev → no permission in prod). Each fix required understanding a new layer of the macOS notification stack. The codesign Identifier mismatch was the most surprising — looked like a complete black box until we captured the daemon's exact rejection message in `log stream`.
- **The dirty-tracking flow.** First implementation only checked the capture key on red X, missed the summary. Diagnostic logging surfaced the design issue (not a bug — I'd literally only wired one of two surfaces). Pivoted to the auto-save plan instead of patching the prompt.
- **The window-state plugin's defaults.** `StateFlags::all()` includes `VISIBLE`, and its `restore_state()` force-shows windows where saved visibility was true. Took me by surprise that opening a fresh build popped the capture popup open every launch. The synthesis from a multi-lens workflow led directly to the `.skip_initial_state(...)` + flag-subtraction fix.

### What's still rough

- **No journal browser.** Reading past notes still means opening the `.md` files in another app. The big remaining Phase 2 item.
- **No markdown rich-text editor.** Just textareas. Phase 2-or-3.
- **Red-X dirty prompt is theatrically incomplete.** Only checks capture, ignores summary. Solution is the **auto-save** plan below, not adding more prompt branches.

### Next session — auto-save (Phase 1 of 3)

The dirty-state machinery built for the close flow is the wrong tool. Better answer is to never have dirty state in the first place — Lattice / Notion / Google Docs auto-save on idle, and the user expects this.

**Phase 1 (next, this commit):** Summary auto-save with visual indicator.

- 1.5s debounce after typing stops → fires `update_weekly_summary` (existing command, no backend change).
- Status indicator beside Save button: `Saving…` (in-flight) / `Saved 2:34 PM` (success, settled) / `Unsaved changes` (typed, debounce pending) / `Couldn't save — retry?` (error).
- Manual Save still works as a force-immediate save.
- After auto-save success, summary leaves the dirty registry → no quit prompt for summary.

**Phase 2 (separate commit):** Capture draft persistence.

- New backend commands: `load_capture_draft` / `save_capture_draft` / `clear_capture_draft`.
- Draft file at `<journal>/.metadata/capture-draft.json` (distinct from real notes — auto-submitting half-typed notes would be wrong).
- Capture popup restores from draft on mount, debounced save on change, clear on Submit success.
- "Draft saved" indicator at the bottom of the popup.
- After draft save, capture leaves the dirty registry.

**Phase 3 (cleanup):** Strip the red-X prompt entirely.

- With both auto-saves landed, the unsaved-work guard becomes theoretical (only fires inside the 150ms debounce gap).
- Red X = pure hide on both main + capture, silently.
- Cmd+Q keeps the guard as a backstop for the rare "save just failed" case.

### Commits since the last journal entry

Roughly 40, headlines:

- Phase 2 wiz + settings + theme toggle + label autocomplete + weekly summary UI (~10 commits)
- Theme v2 + Week Stripe + Noot + guide hand (`9a568f3` + iterations)
- Notification migration arc (`1493943` → `0100069` → `c174975` → `e386f85` → `2259d59` → `608bcd1` → `ae08312`)
- Close flow Option B (`3f16273`) + diagnostic logging (`4b386a7`)

### Test totals at end of sprint

- Backend: **75/75** unit tests passing
- Frontend: `svelte-check` clean (160 files, 0 errors)
- Production .app: builds cleanly with proper codesign + Info.plist; verified end-to-end notification flow with action buttons

---

## 2026-06-23 (continued) — Phase 2 close-out: auto-save, journal browser, spell-check squiggles

Picked up where the prior sprint entry left off — the three auto-save phases, then the journal browser that turns "view past notes" from "open the .md in another app" into a first-class navigator. Closed with the spell-check polish so squiggles actually render under misspelled words inside Tauri's WKWebView.

### What landed

**Auto-save trio**

- **Phase 1 — Weekly Summary auto-save.** 1.5s debounce after typing → `update_weekly_summary` fires (same command the manual Save uses). Inline status indicator: `Saving…` / `Saved 2:34 PM` / `Unsaved changes` / `Couldn't save — retry?`. Manual Save still works as a force-immediate. After successful auto-save the summary leaves the `DirtyRegistry` so the quit prompt no longer flags it.
- **Phase 2 — Capture popup draft persistence.** Drafts at `<journal>/.metadata/capture-draft.json`. New `StorageBackend::delete_metadata` trait method to support the "draft becomes a real note → clean up the draft" pathway. Three new commands: `load_capture_draft` / `save_capture_draft` / `clear_capture_draft`. Subtle italic "Draft saved 2:34 PM" status below the actions row. Bonus item Chris asked for mid-sprint: a Ruby **Discard** button with `confirm()` modal that cancels pending saves, deletes the draft, and hides the popup. `dialog:allow-confirm` capability added.
- **Phase 3 — Strip the red-X prompts.** With auto-save covering both surfaces, the unsaved-work prompt was theatre. Red X on either window now hides silently. Cmd+Q / tray Quit keeps the `DirtyRegistry`-driven guard as a backstop for the rare debounce-gap case.

**Journal browser**

- `/journal` route with a year/week tree sidebar (newest first, current year auto-expanded, current week marked with an orange dot, selected week highlighted in maroon).
- Raw markdown editor on the right. Loads `read_week(year, week)`, writes via new `write_week` command. Same 1.5s debounce auto-save as /summary. New `list_years` / `list_weeks` commands feed the sidebar.
- Switching weeks flushes any pending edits to the previously-selected week BEFORE loading the new one — otherwise a debounce firing after the switch would write the wrong content.
- Save-race guard captures `selectedYearWeek` BEFORE the invoke await so a mid-save week change re-baselines correctly.

**Spell-check (visual feedback)**

The Edit-menu fix was straightforward: install a real macOS app menu including standard Undo/Redo + Cut/Copy/Paste/Select All. AppKit's responder chain needs the Edit menu present for the right-click "Show Spelling and Grammar" affordance to surface in editable controls. Also seed `WebContinuousSpellCheckingEnabled` / `WebGrammarCheckingEnabled` / `NSAllowContinuousSpellChecking` via NSUserDefaults at app launch — WKWebView checks these on each editable focus and they default to `false` in a fresh bundle.

Squiggles required more work. WKWebView in Tauri silently doesn't paint the red wavy underline (upstream bug `tauri-apps/tauri#7705`). Solution after a 4-dimension synthesis workflow: draw squiggles ourselves.

- New `check_spelling` Tauri command hops to the main thread via `app.run_on_main_thread()` + `tokio::sync::oneshot`, calls `NSSpellChecker::sharedSpellChecker().checkSpellingOfString_startingAt(...)` in a loop, returns `Vec<{start, length}>` in UTF-16 code units (matches JS string indexing, so emoji-safe).
- New `<SpellcheckTextarea>` Svelte 5 component is a drop-in textarea wrapper. Mirrors the textarea content into a positioned backdrop `<div>` behind it. Misspelled spans get `<mark class="sq">` with `text-decoration: underline wavy red`. Mirror text is transparent — the real textarea's text sits on top; the squiggles peek through from behind. 400ms debounced check (separate from the 1.5s autosave cadence), monotonic `requestSeq` to drop stale results, `syncScroll()` to keep the backdrop aligned during scrolling, trailing-newline fix to avoid last-line drift. CSS custom properties (`--sq-padding`, `--sq-font-family`, etc.) let each consumer match its own typography — the journal editor uses monospace via these.
- Wired into /summary (4 textareas), /capture body, /journal editor.

**Home page polish**

Small but visible: buttons reordered to Weekly Summary / Browse Journal / Settings, "Browse Journal" got its Capital J, Prodigy mark wordmark added as a centered footer. Pulled the source PNG from the RPG game's experiments folder.

### What was tougher than expected

- **Spell-check squiggles.** First tried passing `spellcheck="true"` and assuming the native checker would do the rest — that part DID light up the right-click menu, but visual underlines stayed missing. Five hours of investigation across NSSpellChecker semantics, NSUserDefaults priming, WKWebView's quirks, and finally the mirror-div technique. The workflow agents nailed the synthesis: pick NSSpellChecker (best dictionary quality, zero bundle cost), bridge via objc2-app-kit feature gate (small compile-time hit), draw squiggles on a transparent mirror layer. End result is barely noticeable from native squiggles in normal use, with the bonus that right-click "Show Spelling and Grammar" still works because the underlying textarea kept `spellcheck="true"`.

### What's still rough

- **Raw markdown only.** Bold/italic/lists don't render — you type and see the markdown source. Phase 2.5 is exactly this gap.
- **No inline `#` autocomplete in body text.** Carries over from Phase 2 wishlist. Also Phase 2.5.

### Test totals at end of day

- Backend: **75 → 75** (no new Rust tests this day; all changes were either pure-frontend or wired through existing commands)
- Frontend: `svelte-check` clean (163 files, 0 errors, 0 warnings after a couple of a11y nudges on the new modal)

---

## 2026-06-24 — Phase 2.6: send weekly summary to manager

Single-day feature: a `Send to manager` button on `/summary` that hands a draft to the user's default mail handler. No SMTP credentials, no OAuth — the user reviews and sends the draft from their real mail identity so threading and the Sent folder work normally. Commit `86d804b`.

### Why this shape

Two upfront design workflows, both fanned out into parallel research streams:

1. **Email-sending mechanism survey** — `mailto:` deep-link via tauri-plugin-opener vs SMTP via the `lettre` crate vs AppleScript driving Mail.app. SMTP requires storing an app password (and Google killed legacy password auth in mid-2025, so it's a dead end on a 1–2 year horizon for Gmail/Workspace). AppleScript locks the app to Mail.app users and demands an Info.plist with `NSAppleEventsUsageDescription` + an Automation entitlement + a TCC prompt. `mailto:` is already 95% wired (the opener plugin's in the dep tree), keeps credentials with the OS, and lets the user review the message before sending. The one real footgun — macOS LaunchServices truncates mailto URLs around ~2 KB — is mitigated cleanly by writing an `.eml` to a temp dir when the encoded URL would exceed 1800 bytes and opening THAT via `opener::open_path` instead. Same UX, no length cliff.
2. **Architecture design** — sent-state per week as a sidecar at `.metadata/sent-log.json` keyed by ISO year-week, with a content-hash field so "edited since send" is detectable. One entry per week (overwrite on resend). Hash includes the four summary fields + labels, length-prefixed.

### What landed

- **Settings**: new `managerEmail` + `managerName` on `JournalSettings`, persisted to `.metadata/settings.json`. Settings UI gets a Manager email field (with a soft "doesn't look like an email" warning that doesn't block save) and a Manager name field (used purely for greeting personalization).
- **Sent-log sidecar**: `.metadata/sent-log.json` keyed by `"YYYY-Www"`. Each entry `{ sentAt, contentHash, sentTo }`. Read-and-overwrite on every send.
- **Email module**: `compose_weekly_email(ComposeParams)` returns either `Mailto(String)` or `Eml(PathBuf)` based on encoded URL length. Body opens with `Hello {managerName},` (or `Hello,`), then an intro line that links to the public Captain's Log repo so the manager can poke around. Four `##` markdown headings follow; empty sections are dropped. Subject is `Weekly update - week of …` on first send, `Update to weekly update - …` on resend (detected by an existing sent-log record at compose time).
- **Commands**: `get_sent_record`, `compose_weekly_email`, `mark_weekly_summary_sent`, `get_summary_hash`. The frontend calls `get_summary_hash` after every save so the gate-comparison stays fresh against the disk.
- **Send button on /summary**: marble (secondary) button to the right of green Save. Disabled tooltip explains the reason: "Set a manager email in Settings" / "Save your changes first" / "Sent {to} on {date}". On a known-edited week the label flips to "Send updated version" and a small orange-tinted line below the row reads "Last sent Jun 24, 4:12 PM (edited since)".
- **Confirmation modal**: previews addressee + week label before opening the draft. Escape and backdrop click dismiss; Cmd-S / Cmd-Enter are swallowed while the modal is open so a stray hotkey can't fire a save mid-confirm.
- **Capability scope**: `opener:allow-open-url` extended with `{ url: "mailto:*" }`. New `opener:allow-open-path` scoped to `$TEMP/captainslog/**`. Verified `$TEMP` resolves to `std::env::temp_dir()` via Tauri's path resolver source.
- **`.eml` temp janitor**: fire-and-forget on app start, prunes anything in `$TEMP/captainslog/` older than 24h. Errors logged; never blocks startup.

### Two adversarial review rounds

Ran the implementation through a multi-dimension adversarial workflow twice (8 + 4 candidate findings → 2 + 1 confirmed real after refute-pass).

**Round 1 confirmed two real bugs in the initial implementation:**

- `hash_weekly_summary` joined fields with a single `\n` separator. Multi-line textareas (bulleted lists everywhere) meant moving a bullet from Key Accomplishments into Plans could produce a byte-identical hash → Send button would refuse to re-send the legitimately-edited summary. Fixed by length-prefixing each field + each label individually. Added a regression test that hits this exact "move a line between sections" case.
- Escape didn't dismiss the confirmation modal even though the comment claimed it did. Focus stays on the Send button when the modal opens, so Escape keydowns went straight to `<svelte:window>` and were never caught. Fixed by routing Escape through `handleKeydown` gated on `showConfirmModal`.

**Round 2 (after Chris's refinement requests):**

- The frontend modal and the email subject used different formats for the same week — "Week of June 22 – June 28, 2026" in the modal vs "week of Jun 22 - Jun 28, 2026" in the subject. Two formatters, one source of truth violated by docstring claiming they mirrored. Aligned the backend to the frontend (full month names + en-dash). Side effect: the en-dash now triggers RFC 2047 encoded-word wrapping for the `.eml` subject (`=?UTF-8?B?…?=`) so strict mail parsers don't choke on non-ASCII headers. Used the `base64` crate (already transitive in the dep tree) so adding the encode path was 4 lines.

### Chris's refinements (post-smoke-test, pre-commit)

- Capitalize months in the modal copy (`weekLabel.toLowerCase()` was flattening "June" → "june"). Replaced with an `inlineLabel()` helper that only lowercases the leading "W".
- Add a manager NAME field (separate from email). Use it as the greeting.
- Subject for resends should signal that this is a revision. Settled on "Update to weekly update - week of …" per his suggestion.
- Email body should open with a greeting + a one-line explanation linking back to the repo. Markdown formatting (when Phase 2.5's CodeMirror lands) should flow into the email verbatim — confirmed by a `markdown_in_summary_passes_through_verbatim` test that uses urlencoding decode to verify literal `**bold**` survives the mailto round-trip.

### What was tougher than expected

- **Hash boundary collisions.** Standard `\n` separator failed silently for exactly the workflow this app is built for (move a bullet between sections). The original commit's regression test only used non-newline strings, so it passed while the bug was still live. Adversarial review caught it. Length-prefixing is unglamorous but boundary-safe; the same fix applies to comma-joined labels (a label containing a comma would otherwise collide with two labels).
- **Backend ⇄ frontend label drift.** The two formatters started identical and drifted in two separate commits, each one looking innocuous in isolation. Docstring even claimed they were kept in sync. The fix wasn't the engineering — it was making the contract explicit in the comment so the next drift forces a test failure.

### What's still rough

- **`.eml` fallback hasn't been smoke-tested with a real long-body week.** All tests pass synthetically; needs a real 5+KB summary at some point.
- **No "did the mail app actually open?" check.** Adversarial review surfaced this and we rejected it as too theoretical for a single-user macOS desktop app. If the heuristic turns out wrong, swap to a post-handoff confirmation step.

### Commits

- `86d804b` Phase 2.6: send weekly summary to manager via default mail app

### Test totals at end of day

- Backend: **75 → 112** (37 new tests across `email::tests` + `sent_log::tests` + `commands::tests::week_label_*` + `settings::tests::journal_settings_legacy_without_manager_email_parses` etc.)
- Frontend: `svelte-check` clean (163 files, 0 errors, 0 warnings)

### Next session

Phase 2.5 — CodeMirror 6 markdown editor. Replaces the textareas on `/summary`, the capture popup body, and `/journal`. Sequenced before the onboarding revisit so the editor refactor is in place before adding new prose surfaces in the wizard.
