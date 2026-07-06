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

---

## 2026-06-24 (evening) — Phase 2.5 Steps 1-7 + the editor pivot

Long session. Phase 2.5's first arc (CodeMirror 6 source-mode editor on all three surfaces) landed step-by-step, the formatting toolbar shipped, then a real UX reckoning forced a mid-phase architectural pivot. Closing the day with a tagged rollback line and a 10-14 day Architecture B plan ready to start tomorrow.

### What landed

**Step 1 — `MarkdownEditor.svelte` + `/capture` swap** (commit `fb40bda`).
- Added `@codemirror/{state,view,commands,language,lang-markdown}` and `@lezer/markdown` deps via `npm install`. Hand-rolled ~40-line Svelte 5 wrapper with a one-way `value` prop + `onChange` callback (NOT `$bindable` — CM6 transactions own the doc; bind: would fight the transaction model and reset cursors).
- External-value sync via $effect compares against the current doc and only dispatches when they differ — breaks the echo loop that would otherwise fire on every onChange round-trip.
- Smoke-test surfaced a CSS quirk: `.cm-editor`'s default `min-height: auto` made it grow with content and push the popup boundaries. Fixed by defaulting `--md-min-height` to 0 so the editor can shrink below intrinsic content size inside a flex-column parent; `.cm-scroller`'s `overflow: auto` takes over with internal scroll.
- GFM extension enabled: task lists (`[ ]`/`[x]`), strikethrough, tables, and bare-URL autolinks now parse cleanly. Still source mode — no rendered checkboxes.

**Step 2 — Cmd-click Markdown links** (commit `05201d8`).
- New `markdown-links.ts` CM6 extension. `EditorView.domEventHandlers.mousedown` checks for Cmd/Ctrl modifier, walks the Lezer syntax tree at the click position, finds the surrounding `Link` / `Autolink` / `URL` node, extracts the href, hands to `tauri-plugin-opener::openUrl`.
- Capability scope extended: `opener:allow-open-url` now allows `http://*` and `https://*` alongside `mailto:*` and `x-apple.systempreferences:*`. URL safety lives at the IPC boundary — anything outside the allow-list gets rejected by the opener plugin.
- Three link forms work end-to-end: `[text](url)`, `<https://example.com>`, GFM bare URLs.

**Step 3 — The spell-check architecture investigation** (commit `cfb2ce3`).
The most twisty arc of the day. Started by porting the textarea-era mirror-div spellcheck (`SpellcheckTextarea.svelte`) over CodeMirror's `.cm-content` via Decoration.mark. Built `spellcheck-cm.ts` with a StateField + ViewPlugin that fires `check_spelling` IPC on a 400ms debounce, mapped returned `{start, length}` ranges to `Decoration.mark` with wavy-red text-decoration. Worked for `teh wrold` — Chris reported `dont`, `wont`, `cant` weren't getting flagged.

Investigation workflow surfaced that **NSSpellChecker's `checkSpellingOfString:startingAt:` only emits the `Spelling` channel.** Missing-apostrophe contractions are routed through `NSTextCheckingTypeCorrection` (one-at-a-time per call by design — it's the inline autocorrect pipeline, not a batch document checker). Switched to `checkString:range:types:` with `Spelling | Correction | Grammar`. Wrote a `spellcheck_probe.rs` example binary to call NSSpellChecker directly with test strings and dump returned ranges + types — revealed that even with the broader mask, NSSpellChecker only returns one Correction at a time UNLESS there's also a Spelling result alongside (some heuristic where the engine "decides" the user is making real mistakes when at least one Spelling fires).

Architecture moment: synthesis surfaced that the whole custom IPC + Decoration.mark plugin was the wrong layer. CodeMirror's editing surface IS a contenteditable div. WebKit's spell-check on contenteditable WORKS — `tauri-apps/tauri#7705` only applies to textareas. Verified by setting `EditorView.contentAttributes.of({ spellcheck: 'true' })` and ripping out the IPC. Native squiggles painted, right-click suggestions pre-populated, contractions caught the way the user expects — same NSSpellChecker that Apple Mail and Pages use.

Net deletes for Step 3: `spellcheck-cm.ts` (~150 LOC), `spellcheck_probe.rs` (~120 LOC), the `.cm-misspelled` CSS, the diagnostic eprintln, the entire custom IPC + Decoration.mark plugin. `SpellcheckTextarea.svelte`, `spellcheck.rs`, and `objc2-app-kit` stay alive ONLY because `/summary` and `/journal` haven't migrated to CodeMirror yet. They get retired in Step 4.

**Step 4 — Migration + the big delete** (commit `b27b263`).
Propagated `MarkdownEditor` to `/journal` (monospace, 14px, 1.5 line-height, 16px padding via `--md-*` CSS variables) and `/summary` (four instances with `--md-min-height` approximating prior `rows={3|4|5}` defaults + `resize: vertical` for user-drag-grow). Added an `id` prop forwarded to `.cm-content` via `contentAttributes` so `<label for={id}>` clicks focus the editor.

The deletes: `app/src/lib/SpellcheckTextarea.svelte` (~220 LOC), `app/src-tauri/src/spellcheck.rs` (~125 LOC), the `check_spelling` Tauri command registration, the `pub mod spellcheck;` declaration, the `objc2-app-kit` dep with its `NSSpellChecker` feature in `Cargo.toml`. Kept `seed_spellcheck_defaults()` in `lib.rs` (uses `objc2-foundation`'s NSUserDefaults — load-bearing for WebKit's continuous spell-checker globally).

Net: -459 lines across 8 files. All three surfaces now use the same `MarkdownEditor.svelte` with native WebKit spell-check.

**Toolbar + journal cheat sheet** (commit `6d60b58`).
10-button formatting strip above each MarkdownEditor on `/capture` and `/summary`: Heading (cycle H1→H2→H3→none) / Bold / Italic / Strikethrough / Bulleted list / Numbered list / Block quote / Link / Code / Help. Keyboard shortcuts (all free in CM6 defaultKeymap): Cmd+B / Cmd+I / Cmd+K / Cmd+E / Cmd+Shift+7 / Cmd+Shift+8.

Architecture:
- `markdown-formatting.ts` holds the command functions (toggleBold, toggleItalic, toggleStrikethrough, toggleInlineCode, toggleBulletList, toggleNumberedList, toggleQuote, cycleHeading, insertLink). Same functions back both toolbar onClicks AND the keymap — wrap/unwrap logic in exactly one place per format. The numbered-list, bullet-list, quote, and heading commands all skip blank lines so paragraph-style multi-line selections don't get empty `- ` / `> ` / `# ` stamps in the gaps.
- `MarkdownToolbar.svelte` is the visual strip — rendered inside `MarkdownEditor.svelte`, above the `.md-editor` wrapper, so the strip stays fixed when `/summary`'s `resize: vertical` handle is dragged.
- `Icon.svelte` is a tiny inline-SVG component with 10 Lucide-derived paths. No icon library dep (8 icons didn't earn one).
- MarkdownEditor gains a `showToolbar?: boolean = true` prop. Default on; `/journal` passes false. No consumer changes for `/summary` or `/capture`.

`/journal` cheat-sheet link added to the existing placeholder copy ("New to markdown? Open the cheat sheet."), opened via Tauri's opener plugin against the already-allowed `https://*` scope.

Adversarial review workflow (5 dimensions, parallel verify): zero confirmed findings. One rejected nice-to-have surfaced the blank-line code asymmetry in the line-prefix commands, which got fixed here as a hygiene win.

### The marker-color experiments — and the architectural reckoning

After the toolbar shipped, Chris asked to tint markdown markers (`**`, `~~`, `#`, `-`, `>`, etc.) with the brand maroon (the Discard button color) so non-markdown users could pick them out of prose at a glance. Initial implementation: a custom HighlightStyle for `tags.processingInstruction` with `color: var(--brand-maroon)`.

Three bugs followed in quick succession:

1. **Bold stopped rendering bold.** Diagnosis: ABeeZee (the brand body face) only ships at weight 400. The default `font-weight: bold` on the strong tag had no real glyphs to use, and WebKit's faux-bold synthesis isn't visible. Fix: explicit `font-family: system-ui, ...` override for `tags.strong` so bold spans switch to SF Pro mid-paragraph. Subtle in face, unmistakable in stroke weight.

2. **Italic, strikethrough, link underlines all vanished.** Diagnosis: I had registered `defaultHighlightStyle` with `{ fallback: true }`. CodeMirror's `getHighlighters` IGNORES the fallback once any non-fallback highlighter is registered. My custom marker style was the non-fallback; it silently killed every default rule. Fix: register both as PRIMARY (no fallback option) — CM6 docs guarantee "the styling applied is the union of the classes they emit."

3. **The maroon was unreadable on the dark theme.** `#6c1e38` is a dark burgundy, designed as a button BACKGROUND with cream text on top. As text color on the dark editor surface it disappeared. Added a theme-aware `--md-marker-color` token — dark theme uses a brightened rose (`#e07a9a`), light theme uses the actual brand maroon.

After all three fixes, Chris stepped back and asked the question that mattered: *"is having visible markdown in these fields even the right direction?"* HR, artists, accountants, PMs are the colleagues who'll use this in months. None of them have seen `**bold**` in their lives. Having markers visible in any form — even tastefully tinted — would be a turnoff before they got past their first paragraph.

Reverted the marker color + code monospace + atom (task) tinting + quote line decoration experiments. Kept only the bold + heading font-family swap (it's a functional fix for ABeeZee's missing bold weight, not a styling experiment). Tagged the clean state as `pre-slack-wysiwyg` (commit `ac101c8`).

### The multi-lens architectural workflow

With token cost explicitly off the table ("we have the tokens to burn"), spun up a comprehensive workflow:
- 3 parallel research streams: codebase survey, Slack/Typora/iA Writer editor-pattern research, storage axis (markdown vs JSON vs sidecar)
- 5 candidate architectures: A (CM6 + Live Preview), B (CM6 + aggressive Slack/Typora hiding), C (TipTap with markdown storage), D (TipTap with JSON storage), E (TipTap with JSON + .md sidecar)
- 6 evaluation lenses scored each architecture 1-10: non-technical user / long-term maintenance / Phase 5 LLM handoff / portability / implementation realism / Chris-as-power-user

Lens vote:
| Lens | Winner |
|---|---|
| Non-technical user | D (TipTap + JSON) |
| Long-term maintenance | **A** |
| Phase 5 LLM handoff | A & B (tie) |
| Portability + escape hatch | A & B (tie) |
| Implementation realism | **A** |
| Chris-as-power-user | **A** |

5 of 6 lenses converged on the markdown-storage axis. Synthesis: *"This is not a tie — five lenses to one, with the dissenting lens speaking for a user population that doesn't exist yet."*

Three of Chris's answers to the synthesis questions shifted the display axis recommendation from A (Obsidian-style active-line markers) to B (aggressive Slack/Typora hiding):
- Non-tech colleague adoption: **imminent (<60 days)** — turns the dissenting lens from hypothetical into load-bearing
- Daily `/journal` editing: **yes** — `/journal` needs full rich-text editing parity with `/summary`
- Markdown storage: **as long as Slack-grade UX is achievable on it** — confirmed yes, B preserves it

Locked the call: **Architecture B — CodeMirror 6 + aggressive Slack-style marker hiding + markdown on disk.** Storage stays portable; display goes all-the-way. Estimated ~10-14 days of focused work with the last 20% (atomic-range escape, selection-drag across hidden marks, line-level constructs like headings) explicitly flagged as where 80% of risk lives.

### Tomorrow

Architecture B begins. Day 1-3 chunk: aggressive-hiding ViewPlugin in `MarkdownEditor.svelte` shipped to `/capture` only, behind no flag (the source-mode default gets replaced). Toolbar + keymap stay visible and working. Chris committed to thorough self-testing — type-check, cargo check, read-the-diff-cold, simulate the user flow step-by-step, adversarial edge-case pass (selection-drag, backspace at boundaries, atomic-range escape, undo across decoration boundaries), then dev-run + eyeball before handing off.

### Commits today

- `fb40bda` Phase 2.5 Step 1: CodeMirror 6 markdown editor on /capture body
- `05201d8` Phase 2.5 Step 2: Cmd-click follows Markdown links via Tauri opener
- `cfb2ce3` Phase 2.5 Step 3: native WebKit spell-check on /capture editor
- `4c24823` Fix flaky .eml filename race in tests
- `b27b263` Phase 2.5 Step 4: CodeMirror on /journal + /summary; retire SpellcheckTextarea + spellcheck.rs IPC
- `6d60b58` Phase 2.5 toolbar: formatting buttons + keymap + journal cheat sheet
- `ac101c8` Revert source-mode marker styling experiments — pre-Slack-WYSIWYG baseline (TAGGED `pre-slack-wysiwyg`)

### Test totals at end of day

- Backend: **112** unit tests passing (unchanged — Step 3-4 deleted tests with the spellcheck module but Step 4's net diff held the suite at 112)
- Frontend: `svelte-check` clean (177 files, 0 errors, 0 warnings)

### Key lessons / things to remember

- `tauri-apps/tauri#7705` is **textarea-specific**, not contenteditable. CodeMirror's editing surface is a contenteditable div → native WebKit spell-check works without custom IPC.
- NSSpellChecker's `Correction` channel is single-issue-per-call by design (inline autocorrect pipeline) — `checkString:range:types:` only emits multiple Correction results when a Spelling result fires alongside. WKWebView's right-click menu only consults the Spelling channel.
- `defaultHighlightStyle` registered with `{ fallback: true }` is IGNORED entirely if any non-fallback highlighter is registered. Always register defaultHighlightStyle as a PRIMARY when adding custom styles on top, otherwise italic/strikethrough/link-underlines silently vanish.
- ABeeZee (the brand body face) has no bold weight. Any `font-weight: bold` rule on ABeeZee text falls through to WebKit faux-bold which is not visibly distinct. Explicit `font-family: system-ui` override for bold spans is the cleanest fix; brand consistency suffers slightly, weight visibility wins.
- For Tauri opener capability scoping: `$TEMP` resolves to `std::env::temp_dir()` (confirmed by reading Tauri's path::parse source). Path patterns support `**` glob recursion.
- The two-axis framing (storage vs display) is the right way to think about editor architecture decisions. They combine freely. Bundling them in "WYSIWYG = JSON storage" loses the design space.

---

## 2026-06-25 — Phase 2.5 Architecture B: live preview, end-to-end

Single longest session of the phase. Yesterday's tagged baseline (`pre-slack-wysiwyg`) was source-mode with a toolbar; today it's a Slack-style live-preview editor with hidden markdown markers, atomic decorations, active-state toolbar buttons, a Confluence-style date chip + picker, list widgets, a `/journal` Preview/Source toggle, layout chrome, and ~4000 words of architecture documentation. No commit yet — the whole arc is sitting in the working tree until I do a clean cold-read pass tomorrow.

### Live-preview engine

The heart of the day. `MarkdownEditor.svelte` got a new set of ViewPlugins that walk the Lezer syntax tree per visible range and emit Decoration sets: `Decoration.replace` for marker characters (`**`, `*`, `~~`, `[`, `]`, `(...)`, `>`, `#`, opening/closing fence lines), `Decoration.mark` for the surrounding span styling (bold weight, italic, strikethrough, link color), and `Decoration.line` for line-level constructs (fenced-code block, blockquote, heading sizes). Active-line edge-correction: when the cursor sits inside a wrap, the markers on THAT wrap un-hide so the user can see what they're editing.

Fence handling has its own little state machine inside the input filter, because the Lezer parser doesn't classify a half-built fence as `FencedCode` until the closing triple is present:

- Type ` ``` ` + Enter → auto-expands to a 3-line block (opening fence, empty body line, closing fence), cursor on the body line.
- The 3rd backtick keystroke ALSO auto-expands, even without Enter — covers the case where you just want a block right now.
- Backspace at body-line start: empty fence → delete the whole block; non-empty → exit up into the line above (cursor lands at end of preceding line).
- Trailing blank line is auto-inserted when a closing fence is flush against the document edge, otherwise the cursor has nowhere to land after Cmd-End.
- Cursor-skip filter: keystrokes on the opening or closing fence lines are rerouted down to the body. Without this, a user typing on the opening fence creates a `CodeInfo` substring (and on the closing fence breaks the closing-fence match → the parser collapses the whole block into a paragraph).

Three Decoration patterns shipped:

- **Inline code chip** (`` `like this` ``): pill-shaped span, monospace, subtle background, `display: inline-block`. The inline-block matters — without it, a strikethrough on a parent paragraph draws through the chip too. Inline-block creates a new line-box and the strike doesn't span it.
- **Fenced-code block**: line decoration on every line in the range, plus a 3px left accent stripe and a slightly elevated background. Opening and closing fence lines are the SAME decoration (so the block reads as one continuous element) but their text is replaced with empty widgets, leaving just the accent stripe at top/bottom.
- **Slack-style blockquote**: 3px accent left bar, italic, muted color. Critically scoped with `:not(.cm-md-fenced-line)` so a fenced code block embedded inside a quote stays readable — italic over monospace looks broken, and the muted color washed out the syntax-highlighting tokens.

### The RangeSetBuilder startSide lesson

Several hours mid-morning chasing a "decorations randomly vanish at certain cursor positions" bug. Root cause: I had a hand-rolled rank field on each decoration (`{rank: 0|1|2}`) intending to control z-order between replace / mark / line. CodeMirror's `RangeSetBuilder.add(from, to, deco)` requires inputs to be **sorted by `deco.startSide`**, not by any custom comparator I hang off the deco's `spec`. When two decorations shared a starting offset and my hand-rolled rank put them in the wrong order relative to startSide, the builder threw a `RangeError: Ranges must be added sorted by from position and side` — which CodeMirror was catching silently somewhere upstream and dropping the entire decoration set.

Fix: drop the hand-rolled rank entirely, sort the working list by the actual `Decoration` startSide values (`replace = 499_999_999`, `mark = 500_000_000`, `line = -200_000_000`), and feed them into the builder in that order. Decorations stopped disappearing and the codepath got 30 lines shorter.

### Toolbar overhaul

Yesterday's toolbar was static — buttons fired commands, no visual state. Today it became active-state-aware and learned two new tricks.

- **`detectActiveFormats(state)`**: walks the Lezer syntax tree from `selection.main.head` upward, returns a `Set<FormatName>` of every format that contains the cursor. Buttons get `.is-active` + `aria-pressed="true"` when their format is in the set. Edge-correction: when the cursor sits at the right edge of a wrap node (e.g. just after the closing `**` of bold), the tree walk would still report Strong as active. Skip wrap nodes whose `to === cursor` so the toolbar agrees with what the user is about to type.
- **Multi-line Cmd+E**: previously dropped the cursor on the hidden opening fence (broken UX — invisible cursor, every keystroke triggered the skip-filter). Now the command computes the end-of-body position with a trailing newline, dispatches the cursor there, and the user lands ready to type.
- **C3 — transformLines fence guard**: the prefix-line commands (quote, bullet, numbered) walk selected lines and prepend a marker. If the selection straddled or sat inside a fenced block, they corrupted it. Added a `isInFencedBlock(state, line)` check and skipped opening/closing fence lines plus body lines.
- **C4 — heading cycle preservation**: cycling `# → ## → ### → none` was implemented as a regex that matched only H1-H3. An H6 line had its `# ` prepended (no match to strip) and got bumped to H7 nonsense. Added an early-return for unsupported levels and a narrow strip that handles H4-H6 verbatim.
- **M2 — link placeholder**: `[text](url)` placeholder said the literal word `url`. Clicking away left the user with a link to `url`, which the Cmd+click handler obligingly tried to open. Changed to `https://` — at worst the user has a broken URL, but the intent is clear.
- **Two new buttons**: Task list (Cmd+Shift+L), Today's date (Cmd+;).
- **Five new shortcuts**: strike (Cmd+Shift+X), quote (Cmd+Shift+9), heading cycle (Cmd+Alt+0), task (Cmd+Shift+L), date (Cmd+;).

Three skeptic-found bugs during the adversarial pass:

- **H6 regression**: the strip regex didn't match H4-H6 but the prepend ALSO didn't guard. Caught before commit.
- **Empty-line task on a blank doc**: hitting Cmd+Shift+L with the cursor on an empty line at the top of a blank document did nothing. The transform skipped blank lines globally. Added an `addOnBlanks` branch + a `sawNonBlank` accumulator so the first blank still gets the marker.
- **Cmd+; vs Cmd+Shift+;**: I had Cmd+Shift+; on date. Google Sheets convention is Cmd+; for date and Cmd+Shift+; for time. Swapped to match — if I ever add a "current time" command, the obvious shortcut is free.

### Date chip + Confluence-style picker

`date-chip.ts` is a ViewPlugin that scans the visible viewport for `\b\d{4}-\d{2}-\d{2}\b` (anchored on word boundaries to avoid matching `2026-06-25-01-23` partials), skips matches inside code spans via the syntax tree, and emits a `Decoration.replace` with a `WidgetType` rendering a clickable pill. The pill shows a short form when the date is in the current year (`Jun 25`) and a long form otherwise (`Jun 25, 2026`). Atomic range registered so the cursor jumps over the chip on arrow keys instead of falling into the hidden underlying offset.

`DatePickerPopover.svelte` is a hand-rolled month grid — ~200 lines, no calendar lib. Keyboard nav: arrows move days, PgUp/Down moves months, Shift+PgUp/Down moves years, Enter commits, Esc closes. Outside-mousedown closes (mousedown not click — click loses to the chip's own mousedown if the picker just opened from a chip click). Position computation is Floating-UI-style: prefer bottom-left of the chip, flip to top if there isn't room, clamp horizontally to viewport with an 8px pad.

**The routing-doc-changes lesson** — load-bearing for `/summary`, which has four MarkdownEditor instances. Initial design dispatched commits through a `window` event with a Set of active views registered on plugin construction. With four editors on one route, a date chip in field 2 firing a window event would route to whichever view's listener won the race. Symptoms: editing a date in "Plans" would replace text in "Key Accomplishments." Rewrote to pass the owning view down through the WidgetType constructor and dispatch directly with `view.dispatch(...)`. Window event + activeViews Set deleted.

**Position-bake fix**: WidgetType subclasses default `eq()` to `true` for any other instance of the same type. The widget DOM was being reused across content shifts — a date chip that had been at offset 312 on first render kept its handlers' bound `from`/`to` when the underlying date moved to offset 327 after text was inserted above. Override `eq(other) { return other.from === this.from && other.to === this.to }`. DOM rebuilds on shift; handlers see correct offsets.

**Strict cursor bounds**: original implementation hid the chip whenever the cursor was inside `[from, to]` of the date match. Made dates impossible to "see" right after typing them (cursor sat at `to`, blocked the chip from rendering). Loosened to `cursor > to || cursor < from` — chip renders the moment the typing cursor leaves the match.

**Viewport-edge clamp**: opening the chip near the top of the `/capture` popup (which is a small window) clipped the popover above the screen. The position computation flipped to top but didn't clamp top to a min viewport-pad — fix was a single `Math.max(viewportPad, computedTop)`.

### /summary, /journal toggle, layout chrome

**/summary** got livePreview enabled on all four fields, all four field min-heights unified to 112px (the four `rows={3|4|5}` defaults from the textarea era were inconsistent enough to look sloppy), labels updated with Unicode horizontal ellipsis (`…`), placeholders cleared from `- ` to empty (the auto-bullet leaked into screenshots of empty fields).

**/journal Preview/Source toggle**: segmented control, Cmd+Shift+S keyboard shortcut, `localStorage` persistence under `captainslog:journalViewMode`. Defaults to Preview. Editable in BOTH modes — Source is for power-user formatting, not view-only. The implementation uses Svelte's `{#key viewMode}` block to force a full remount of the editor on mode change, because CodeMirror bakes its extension list at construction and there's no clean way to swap a livePreview extension in/out at runtime. Source mode uses monospace + 14px; Preview uses the body font + 16px + the formatting toolbar.

**`/journal` no longer auto-selects the current week on mount.** This was a real data-mutation bug: open `/journal`, type anywhere on the page (the editor caught focus on mount), and the current week's file got modified. Now the right pane shows an empty-state placeholder until the user explicitly clicks a sidebar entry. Selection is intentional; opening is read-only.

**Layout chrome**: cat companion in the upper-left, clickable, opens a random YouTube cat search via `tauri-plugin-opener`. "Meow!" tooltip on hover. Hidden on `/journal` because the journal browser's left column has its own visual weight and the cat overlapped awkwardly. Help + Nerds Only popups moved from lower-right to lower-LEFT so the appearance of a scrollbar on smaller windows doesn't shove them around. Both pill-shaped, 11px font. Backdrop dismiss + Escape + close button. Focus restored to the trigger on close.

Help body covers the three surfaces (Capture / Summary / Journal), what a note IS, keyboard shortcuts grouped by category, the menu-bar capture icon, Noot ("they're here from the RPG to help"), tips. Nerds Only covers Tauri / SvelteKit / Svelte 5 runes / CodeMirror 6 / Lezer + GFM / CommonMark storage / typography / the live-preview model / repo link.

### List widgets

The final visual upgrade of the day. Plain markdown bullets (`-`) and tasks (`- [ ]`) are now widgets:

- **BulletWidget**: replaces the `-` ListMark of a BulletList item with a `•` (muted color, fixed-width). Numbered list ListMarks are LEFT ALONE — `1.` `2.` `3.` carry information that `•` would erase.
- **TaskCheckboxWidget**: replaces the 3-char `[ ]` or `[x]` TaskMarker with a clickable 16px square. Click toggles via direct `view.dispatch` (same routing lesson — no window events).
- **Strikethrough-on-checked**: a sibling `Decoration.mark` (`cm-md-task-done`) applies `text-decoration: line-through` + muted color to the body of a checked task. The inline-code chip's `display: inline-block` fix earned its keep here — `[x] don't \`code\` this` strikes the body but the chip keeps its identity.

**Dynamic Tab cap**: indenting a list item with Tab used to be uncapped. CommonMark only treats an indented item as a sub-item of the previous list item if its content-offset is within `parent_content_offset + 3` (the 1-3 spaces "sub-item range"). Beyond that, the parser treats it as a sibling at a deeper indent, which Lezer renders correctly but produces weird wrapping. `maxListIndentAllowed(state, line)` walks backward from the current line to find the would-be parent list item's content offset and caps the new indent at `parent + 3`. Top-level lone items (no preceding list context) can't Tab at all — no parent to nest under.

**Lazy-continuation fix v2**: discovered when smoke-testing the "press Enter twice to exit a list" behavior. Two edge cases:

- After hitting Enter on `- foo`, the new line `- ` (just the marker) is auto-inserted. If the user then types nothing and hits Enter, CommonMark's lazy-continuation rule absorbs the trailing `- ` into the preceding paragraph instead of starting a fresh BulletList. Same problem when the line is fully blank.
- Worse, if the line ABOVE the new `- ` is a non-blank paragraph of a different family (e.g. text directly after a heading), Lezer parses the trailing `- ` as a **Setext heading underline** — silently re-classifying the paragraph above as an H2 with no marker visible to the user.

Fix in `applyListMarkerToCurrentLine`: when the line above is non-blank and a different list family from what we're inserting, prepend a `\n` before the marker so Lezer sees a blank-line separator and parses a fresh BulletList. Verified empirically by parsing each case through `@lezer/markdown` + GFM with a small script in the scratchpad.

### Architecture documentation

Wrote `ARCHITECTURE.md` (~4000 words). Covers: overview, the three surfaces (Capture / Summary / Journal), the storage model (CommonMark on disk, `.metadata/` sidecars), the live-preview architecture (ViewPlugins, decoration patterns, atomic ranges, RangeSetBuilder ordering), the toolbar + commands (single source of truth for wrap/unwrap logic), the routing-doc-changes pattern (the four-editor `/summary` lesson made explicit so I don't backslide), and known limitations.

### File audit + cleanup

Audited 20 existing weekly files in parallel — agent fan-out, one shell per file. 3 clean, 16 with en-dash drift in `# Week of` titles (the email-subject en-dash fix from yesterday backflowed into the file titles in a way I didn't expect; bulk-fixed to hyphens), 1 broken W26 test/scratch file deleted. Also generated 8 synthetic test files for 2024 + 2025 (4 each, varied realistic content) to exercise the multi-year sidebar — verified, then deleted.

### Key lessons / things to remember

- **RangeSetBuilder requires inputs sorted by `Decoration.startSide`, not by a custom `spec` field.** The startSide constants are `replace = 499_999_999 < mark = 500_000_000`, and line decorations sit at `-200_000_000`. Hand-rolled rank fields will fight the builder silently — when builder ordering is wrong it throws upstream and the entire decoration set is dropped, producing a "decorations randomly vanish" symptom that looks nothing like a sort error. If decorations disappear at specific cursor positions, suspect this first.
- **CommonMark lazy-continuation absorbs an empty `- ` line that follows a non-blank line of a different family.** When auto-continuing a list, the inserter must prepend `\n` if the line above is non-blank and from a different list family. Otherwise the marker disappears into the preceding paragraph.
- **Setext-heading underline silent re-classification.** A paragraph followed by `- ` becomes an H2 (`-` is a Setext underline). This is upstream of the lazy-continuation fix: even when the inserter does the right thing, a user manually typing `- ` directly under a paragraph will silently bump the paragraph to H2. The blank-line separator fixes both.
- **Routing doc changes: direct `view.dispatch` on the owning MarkdownEditor, not window events + an activeViews Set.** With multiple editors on one route (`/summary`'s four fields), window events misroute under load. Pass the view down through WidgetType constructors and dispatch on it directly.
- **`WidgetType.eq()` must include the underlying offsets** (`from`/`to` or whatever the widget's handlers close over). Default `eq() => true` means CodeMirror reuses the DOM across text shifts, baking stale offsets into handlers. Override `eq` to compare offsets and the DOM rebuilds correctly.
- **Dynamic Tab cap for list indenting = `parent_content_offset + 3`** (the CommonMark sub-item range), NOT a flat indent number. Walk backward to find the parent list item's content offset; cap the new indent at that + 3. Top-level lone items can't Tab — there's no parent to nest under.
- **`/journal` auto-selecting the current week on mount is a typing-data-leak.** Opening a route shouldn't mutate the current week's file just because the editor catches focus. The rule is: selection is explicit; opening is read-only. The empty-state placeholder is the right shape.
- **Inline-code chip needs `display: inline-block`** so a parent strikethrough (e.g. a checked task) doesn't draw through the chip. Inline-block creates a new line-box; the strike line doesn't cross it.
- **CodeMirror bakes its extension list at construction** — there's no clean runtime swap for livePreview-on / livePreview-off. The Svelte `{#key viewMode}` block forcing a remount on Preview/Source toggle is the right shape, not a hack.
- **Cmd+; = date, Cmd+Shift+; = time** (Google Sheets convention). When I add a time command, the shortcut is already reserved by convention; don't poach it for something else.

### Tracked follow-ups

Not blockers — captured here so they don't get lost:

- Cursor preservation across `/journal` Preview/Source toggle. The `{#key viewMode}` remount resets the cursor to 0. Need to capture `selection.main.head` before remount and restore after.
- Cross-route stale preview between `/summary` and `/journal` on the same week. Pre-existing race from the file-IO layer; surfaces now because both surfaces render the same week. Probably wants a shared in-memory cache keyed by year-week.
- Cmd+Home / Cmd+End / Cmd+F land the cursor on a fence line, then arrow-key cursor-skip filter assumes a 1-line delta from the previous position. The current `lineDelta > 1` guard mitigates but doesn't fully solve.
- IME composition on a body-line-start backspace — the empty-fence-delete branch fires mid-composition and eats the composition state.
- Multi-cursor + most widget commands bail rather than handle each range. Acceptable for now; full multi-cursor support is its own arc.
- Setext headings not detected by the active-state walker. Atypical in practice (almost everyone uses ATX `#`), but the toolbar's heading button won't light up on a Setext H1/H2.

### Commits today

None yet. The whole arc is in the working tree pending a cold-read pass tomorrow morning before staging. Will likely commit as 5-7 logically grouped commits rather than one mega-commit: live-preview engine, toolbar, date chip, journal toggle, list widgets, layout chrome, architecture doc.

### Test totals at end of day

- Backend: **112** (unchanged — today was almost entirely frontend).
- Frontend: `svelte-check` clean (181 files, 0 errors, 0 warnings). Four new files (`date-chip.ts`, `DatePickerPopover.svelte`, the list widget module, the layout-chrome popups) accounting for the +4.

### Tomorrow

Cold-read the diff. Stage and commit in logical chunks. Then look at the tracked follow-ups list and pick off the cursor-preservation and cross-route-stale-preview items — both are small fixes that have outsized UX impact. After that, the editor arc is functionally done and Phase 2.5 closes; next phase is the onboarding revisit that got deferred when 2.5 expanded.

---

## 2026-06-25 (evening) — Phase 2.5b: the two tracked follow-ups, with three adversarial rounds

Picked up the two end-of-day follow-ups. What I thought would be ~60 minutes of polish turned into ~3 hours because each adversarial verification pass surfaced a real bug I had missed. The patches landed clean; the journey is the lesson.

### Cursor preservation across Preview/Source toggle

Straightforward. CM6's `Compartment` is purpose-built for prop-driven extension swaps. Wrapped the live-preview extension in `livePreviewCompartment.of(...)` inside `MarkdownEditor.svelte`, added a `$effect` that watches the `livePreview` prop and dispatches `view.dispatch({effects: livePreviewCompartment.reconfigure(...)})` on change. Removed the `{#key viewMode}` wrapper in `/journal/+page.svelte` so the editor no longer remounts on toggle.

Result: cursor, selection, scroll position, AND undo history all survive Cmd+Shift+S now.

The Compartment is declared INSIDE the component (per-instance), not at module scope. On `/summary` with four MarkdownEditor instances, each gets its own compartment; reconfigure() dispatches are per-view. No cross-instance contention.

### Cross-route file invalidation — the part that grew tentacles

Setup is straightforward: when a writer (`/summary`'s `update_weekly_summary`, `/journal`'s `write_week`, `/capture`'s `create_note`) writes the file, Rust emits a `weekly-file-changed` event with `{year, week}`. The other routes listen and reconcile by re-reading from disk.

UX shape:
- Disk matches in-memory baseline → silent no-op.
- Disk differs, no unsaved edits → silent reload.
- Disk differs, dirty form → show "modified externally" banner with Reload + dismiss.

Wrote it, ran type-check, ran cargo check — both green. Felt done. Decided to spend tokens on adversarial verification before declaring victory. **Glad I did.**

### Round 1 — own-save race (real bug, found by skeptic)

Six parallel skeptics, each given a different angle. Three of them independently flagged the same issue: **Tauri's invoke-response and event-emit travel separate IPC paths. There's no contract that the event arrives after the invoke promise resolves.** My initial design gated the listener with `if (saveStatus === 'saving') return;` — but by the time the listener callback runs, `saveStatus` may have flipped to `'saved'` already.

Worse: if the user types during the save's await (perfectly normal — they don't pause), the post-baseline `initialContent` no longer matches their freshly-typed `content`. The listener compares `disk` (= committed bytes from our own save) to `content` (= user's freshly-typed bytes) → diff → `isDirty` → **false-positive "modified externally" banner pointing at the user's own save**. And the banner has a Reload button that would silently destroy the typed-during-save characters.

Fix: track `pendingCommit` (the bytes our in-flight save is writing) set BEFORE invoke and cleared in `finally`. The listener no-ops when disk matches EITHER the post-baseline state OR `pendingCommit`. Robust to either ordering.

### Round 2 — TWO more real bugs the patch revealed

Re-ran verification on the patched code. Found two more bugs:

**Bug A — pendingCommit is a single slot.** If invoke takes longer than the autosave debounce (1.5s), a second saveNow can fire while the first is in flight, overwriting `pendingCommit` with the new bytes. The first save's event arrives, listener checks disk against `pendingCommit` (which now holds the SECOND save's bytes), mismatch, false-positive banner returns.

Trigger path: anything that makes invoke slow (large weekly file, OS fsync pressure, IPC contention). The autosave `$effect` was unconditionally setting `saveStatus = 'dirty'` on every keystroke — even during a save — so the gate `if (saveStatus === 'saving')` in saveNow had already been cleared when the second timer fired.

**Bug B — Rust normalizes on write.** The skeptic read the actual Rust source: `render_weekly_summary` calls `trim_body` (trim_end) on each field; `extract_subsection` calls `.trim()` (both ends) on read; labels go through `.trim().trim_start_matches('#')` + empty-filter. So the bytes on disk are POST-normalization. My frontend was storing PRE-normalization values in `snapshot` and `pendingCommit`.

Consequence: every `/summary` save with a trailing newline or a `#release`-style label would either (a) show a false-positive banner OR (b) silently rewrite the field with the normalized version on every own-save echo — CodeMirror would reset cursor/selection mid-typing. Critical bug.

Patches:
- For Bug A: typing `$effect` no longer downgrades `saveStatus` from `'saving'` to `'dirty'`. The saveNow gate now reschedules the autoSaveTimer instead of dropping the call. Together this prevents concurrent saveNow calls.
- For Bug B: added a `normalizedSig` helper in `/summary` that mirrors Rust's normalization rules. Applied at compare-time only (never to form fields or snapshot directly), so user's pre-normalize input survives in the editor between saves, but the disk-compare correctly treats normalization-only deltas as "no real change."

### Round 3 — two MORE real bugs

Re-verified. Found:

**Bug C — /summary's `get_summary_hash` await is AFTER `saveStatus = 'saved'`.** During the hash-refresh await, `saveStatus` is 'saved' but `pendingCommit` is still set. A rescheduled saveNow firing in this window passes the gate, overwrites `pendingCommit`, starts a concurrent invoke. Then the FIRST save's `finally` runs and clobbers the SECOND save's `pendingCommit`. Race re-opens.

Fix: move the hash refresh BEFORE `saveStatus = 'saved'`, so the gate stays armed for the full critical section. The `lastSavedAt`/`saveStatus` flip happens only after every await in the save lifecycle has completed.

**Bug D — /summary's `onDestroy` doesn't clear `autoSaveTimer`.** The reschedule arm can leave a dangling timer scheduled for ~1.5s out. The Done button calls `goto('/')` and is only gated on `saveStatus === 'saving'`, so the user can navigate away with a pending timer. The timer fires on the destroyed component and tries to save stale state.

Fix: mirror /journal's `onDestroy` and clear the timer. Trivial.

### Key lessons

**Tauri IPC ordering is not contractual.** Don't rely on "the invoke resolves before the event arrives" for correctness. Use payload comparison (pendingCommit) that's robust to either ordering.

**The autosave `$effect` is load-bearing for the save state machine.** Mutating `saveStatus` from the typing path can silently break gates elsewhere. The fix was to read `saveStatus` and only downgrade when it's a sensible transition (not from `'saving'`).

**Skeptics > self-review.** Three independent rounds found four real bugs across the same code I had just verified-with-type-checks-clean. The reading-against-actual-source angle (skeptic #2 reading notes.rs to find the normalization rules) is something I would have skipped in a self-review.

**The reschedule-on-gate-trip pattern coalesces typing-through-save cleanly.** Instead of dropping the second save (data loss) or running it concurrently (race), defer it until the in-flight save settles. Cheap, correct, doesn't require Set-of-pending or per-save tokens.

### Files touched

- `app/src/lib/MarkdownEditor.svelte` — Compartment, $effect for prop-driven extension swap.
- `app/src/routes/journal/+page.svelte` — removed `{#key viewMode}`; added listener + reconcileWithDisk + pendingCommit + banner; autosave $effect guarded; saveNow reschedules on in-flight; onDestroy already clears autoSaveTimer.
- `app/src/routes/summary/+page.svelte` — listener + reconcileWithDisk + pendingCommit + normalizedSig + banner; autosave $effect guarded; saveNow reschedules on in-flight; hash refresh held inside 'saving'; onDestroy clears autoSaveTimer.
- `app/src-tauri/src/commands.rs` — added `WeeklyFileChanged` struct, `emit_weekly_file_changed` helper, three call sites (`create_note`, `write_week`, `update_weekly_summary`) take AppHandle and emit after successful write.

### Test totals at end of evening

- Backend: **112** (unchanged — Rust changes were small additions that don't need new tests; existing test suite covers the file write paths).
- Frontend: `svelte-check` clean (182 files, 0 errors, 0 warnings).
- Adversarial workflow rounds run: 3.
- Real bugs found and fixed: 5 (round 1: own-save race; round 2: pendingCommit single-slot, pre-normalize values; round 3: post-saved hash window, dangling autoSaveTimer).

### Tomorrow

This is the actual end of Phase 2.5. Cold-read the full editor + invalidation diff. Stage in logical chunks. Run the smoke tests Chris has been doing in-app — type during a save, toggle modes mid-edit, /capture-while-/journal-open. Commit. Then move to Phase 2.7.

## 2026-06-26 — Phase 2.7 + 2.7b (onboarding, multi-day reminders, cross-app UX polish)

A long session that covered all of Phase 2.7 plus a parallel polish pass (2.7b) that ran longer than 2.7 itself.

### Dock icon reset bug (incidental, opened the day)

When the main window restored from `.Accessory` activation policy (used by the menu-bar capture flow) back to `.Regular`, the dock icon dropped to the generic placeholder. Root cause: macOS clears `NSApplication.applicationIconImage` on the policy flip. Fix: `restore_dock_icon()` re-loads `icon.icns` from the bundle and re-sets it via objc2 — runs every time `restore_main_window` flips back to `.Regular`. Small, isolated, no design call needed.

### Phase 2.7 — Onboarding wizard expansion

Pre-design step before code: walked through the wizard end-to-end with Chris to lock the 5-step flow and the Ed images. Final shape:

1. **Intro** — Ed waving, copy welcoming the user.
2. **About you** — name, Bamboo title (the word *Bamboo* hyperlinks to Prodigy's BambooHR site), Jira project keys (comma-separated, normalized server-side, defaults `MAGE` if empty).
3. **About your manager** — name + email (reuses the columns added in Phase 2.6).
4. **Settings** — journal location picker, reminder time + day(s).
5. **Complete** — Ed cheering, "Open your journal" CTA.

Each step has a unique Ed image (waving / pointing / writing / pointing again / cheering) — Chris approved the placements after one iteration. Wizard state is local to the route; only commits to `app-settings.json` on the final step. Cancel = wizard goes home and writes nothing.

### Multi-day reminders + tabbed Settings

Triggered by a feature request from the MAGE devs after Chris demoed the app: "can we have reminders on multiple days, not just one?" Decision: build in two slices — first multi-day data model + UI, then the Settings tab restructure that the new field made necessary.

**Data model:** `dayOfWeek: u8` → `daysOfWeek: Vec<u8>` in `ReminderSettings`. Serde back-compat shim via `ReminderSettingsRaw + From impl`:

- If `daysOfWeek` is present (even `[]`), it wins — explicit empty array means user has opted out of reminders, not "fall back to legacy single value".
- Else fall back to the legacy `dayOfWeek` field.

This nuance bit us in adversarial round 1 — `unwrap_or_default` silently overrode the empty array with the legacy value. Fixed with an explicit `match` on `Option<Vec<u8>>`.

**DST scheduling:** the original `next_reminder_time` used `Duration::days(7)` (fixed seconds), which drifts by an hour across spring-forward/fall-back. And `with_hour(2).unwrap()` panicked on the spring-forward gap. New `resolve_local_datetime` returns `None` on the gap (caller bumps by 7 days, preserves weekday) and earliest local time on the ambiguous fall-back hour. Naive-date arithmetic + per-target localization inside `next_reminder_time_for_day` keeps the actual local clock time stable across the boundary.

**UI:** day-pill picker (Mon Tue Wed Thu Fri Sat Sun, multi-select). Pills use `--accent-primary` when selected; bumped pill text to `#1f0a02` (dark) when the day-pill failed contrast at 13px on `--accent-primary` (3.25:1 → AA fail; dark text → 9.5:1, pass). Adversarial round.

**Settings tabs:** General / Reminders / Theme, persisted in `localStorage` so a Settings round-trip preserves the active tab. General sub-divided into "Your details…" / "Manager details…" / "Journal location…" sections (Chris-specified ellipses).

### Three adversarial verification rounds

Same workflow as Phase 2.5: Plan → Implement → Skeptic agents → triage → fix. Real bugs found this session:

- **Empty `daysOfWeek` clobbered by legacy** — described above.
- **DST spring-forward panic** — `.expect()` on `with_hour` crashed the scheduler.
- **DST 1-hour drift** — `Duration::days(7)` is fixed seconds.
- **Own-save race in `pendingCommit`** — `get_summary_hash` await was *after* `saveStatus = 'saved'`, leaving the external-update gate open for a window where the file mtime had updated but we hadn't refreshed the hash. Fix: hash refresh moved *before* the status flip.
- **Concurrent `saveNow` clobbering pending state** — second save kicked off while first was in flight downgraded `saveStatus` from `'saving'` to `'dirty'`. Fix: `saveNow` reschedules instead of overwriting in-flight state.
- **Pre-normalization signature mismatch** — Rust trims fields on save; frontend was hashing pre-normalize values, so external-update detection misfired. Fix: `normalizedSig` helper that compares post-normalization.
- **/summary `onDestroy` missing `autoSaveTimer` cleanup** — dangling timer could fire on destroyed component. Mirrored `/journal`'s pattern.
- **Day-pill contrast** — described above.

8 real bugs total across the three rounds. The Skeptic-Agents workflow paid for itself again.

### Dark-theme contrast audit + 30+ fixes

Sweep across the app after Chris noted some text looked washed-out in dark mode. Worst offenders:

- **`--accent-primary` on dark surfaces as text** — orange at the brand value is fine as a fill but fails AA when used for text on `--bg-surface` (dark). Added `--accent-primary-text: #ff8e51` (dark theme override) and migrated every usage where the accent was actually being read.
- **`--accent-pink` as text on dark surfaces** — same problem. Added `--accent-pink-text: #ff80c0`.
- **`text-muted` on `bg-elevated`** — 5+ sites. Bumped to `text-secondary` where the contrast didn't clear AA.
- **`--accent-teal` in `LabelInput`'s palette** — 1.61:1 on dark surfaces, no fix possible without diverging from the brand teal. Dropped from the palette entirely. Existing labels using teal keep working (still in token set, just not in the picker).

After the sweep, ran a Chrome devtools contrast pass across every screen in dark mode and didn't find a remaining AA fail.

### UI legacy sweep + token cleanup

Cleared the kind of cruft that accumulates faster than you realize:

- **`var(--space-5)` was never defined** — found two callsites (Settings `.section` gap, `/summary` `.modal-actions`) silently rendering 0px. Switched both to `var(--space-6)`.
- **`999px` raw values** for pill radii → `var(--radius-pill)`.
- **`13px` raw value in `DatePickerPopover`** → `var(--text-caption)`.
- **Four dead tokens** removed from `app.css` after grep showed zero callsites.
- **Dead `.status-success`** class and dead `class="editor"` reference removed.
- **`.card`** promoted from `/summary` to `app.css` as a global utility class.
- **`.text-input`** promoted similarly (was duplicated across 4 routes).
- **`.sent-status`** promoted (used by `SendToManagerButton` on both `/summary` and `/journal`).
- **Stale Phase 2.5 comments** trimmed from `/journal` and `/summary`.

### Shared component extractions

Threshold: extract when ≥2 callsites with ≥80 LOC of duplication. Four extractions today:

- **`ExternalUpdateBanner`** — snippet-based message slot, shared between `/journal` and `/summary`.
- **`SaveStatus`** — 5 states (idle/dirty/saving/saved/error), renders as button (with `onRetry`) or span (without), all text overridable via props. Made it possible to put save status in a consistent location across `/journal`, `/summary`, `/capture`.
- **`InputField`** — wraps `<label>` + `.text-input` + hint/warning. Supports `labelSnippet` for inline links (used for the *Bamboo* hyperlink in the wizard) and `hintSnippet` for inline markup. Hit one TypeScript snag — used `HTMLInputAttributes['autocomplete']` instead of the `AutoFill` type (not exported). Replaced with the type literal directly.
- **`SendToManagerButton`** — by far the biggest extraction (~370 LOC). Owns `sentRecord`, `currentHash`, `managerEmail` state, listens to `weekly-file-changed` for the active `(year, week)`, contains the confirmation modal + send flow. Two bindable props (`sentStatusText`, `sentStatusIsStale`) let the parent render the "Last sent …" line wherever it wants. Side effect: bringing send-to-manager to `/journal` became a 2-line change (gated on a selected week).

### WeekStripe scrollbar-gutter fix

Chris noticed Noot's position and the day-of-week stripe shifted slightly between `/journal` (scrollable) and `/summary` + `/settings` (non-scrollable). Root cause: document scrollbar reserved a gutter on the scrollable route, shrinking the viewport by ~15px and pulling the centered stripe with it. Fix: `html { scrollbar-gutter: stable; }` in `app.css`. Reserves the gutter on every route. Tested across all four routes.

### Button/UX standardization pass

Chris-specified convention nailed down:

- **Primary action always left, Cancel/Back/Discard always right.**
- **Cancel/Back/Discard always `btn-ruby` (maroon).**
- **Save status always leftmost in the actions row** on `/journal`, `/summary`, `/capture` — same scan location across all three routes.

Per-route changes:

- **`/capture`** — Submit ↔ Discard order swapped. SaveStatus moved into `.actions` row.
- **`/settings`** — Cancel (was Done, gray) → `btn-ruby`, swapped with Done order. Section titles got Chris's ellipses ("Your details…").
- **`/journal`** — Back button now always visible (was hidden when no entry selected). "Write Weekly Summary" link in the placeholder replaced with `btn-emerald` button. Send-to-manager button added via `SendToManagerButton` (gated on selected week). Removed the redundant "← Home" link from the sidebar header.
- **`/summary`** — Done → Back rename (and `btn-ruby` color). Final button order: Save, Back, Send to manager.

### Sent-status repositioning (took two passes)

Chris wanted the "Last sent Jun 26 at 11:22 AM (edited since)" line above the actions row, right-justified, on both `/journal` and `/summary`. First attempt: added `margin-top: var(--space-4)` to `.sent-status`. Fixed `/journal` but the same margin doubled the spacing on `/summary`.

Root cause: `.actions` on `/summary` was inside `.form`, which had `flex gap: var(--space-6)`. The margin stacked on top of the flex gap. On `/journal`, `.actions` was at the top level so no parent gap applied.

Final fix: extracted `.actions-area` wrapper that contains both `.sent-status` and `.actions`. Lifted it *outside* `.form` on `/summary`. Mirrored the same structure on `/journal`. Zeroed `.sent-status` margins; parent `.actions-area` owns the spacing via flex `gap`. Now visually identical on both routes.

### Code-volume rough cut

- Tauri/Rust: small additions (`daysOfWeek`, DST helpers, `restore_dock_icon`, `bambooTitle`, `jiraProjectKeys`, `normalize_jira_keys`).
- Frontend: ~1500 LOC added (wizard, 4 new components, Settings tabs), ~800 LOC removed (extraction targets, dead code, legacy duplication).
- New tokens: `--accent-primary-text`, `--accent-pink-text`, `--bg-error-tint`, `--bg-error-tint-soft`, `--border-error`.
- New utility classes promoted to `app.css`: `.text-input`, `.card`, `.sent-status`.
- Tests: backend test count unchanged; `svelte-check` clean.

### Adversarial verification rounds

3 again. 8 real bugs found and fixed (listed above). No skeptic round flagged a bug the prior round had missed, which is the signal I look for to know the implementation has stabilized.

### Tomorrow

Two paths queued up:

1. **Phase 2.8 — Custom Themes.** Already designed. Wide token surface (10-12 user-editable tokens), hex inputs with swatches, auto-derived contrast tokens, Theme = Light / Dark / Custom, persist in `app-settings.json`, export/import via Tauri dialogs (link to htmlcolorcodes.com from the picker for color reference), "Reset to Light/Dark" buttons.
2. **Phase 2.9 — HTML email rendering + Preview modal.** The deferred Phase 2.5 Steps 5-6 finally getting a slot. `pulldown-cmark` for the body, iframe srcdoc for the preview.

Chris's call which order. Custom Themes is the more user-facing of the two; HTML email is the more architecturally interesting.

---

## 2026-06-29 — Phase 2.9b shipped

Phase 2.9 was dark-released on the 26th because the `.eml` send path opens Apple Mail read-only — every send became a Message → Edit-as-New-Message dance instead of the 1-click flow we had pre-2.9. 2.9b finishes the job: Mail tab in Settings, three first-class send modes (Gmail / Native Mac Mail / Outlook), universal Preview modal with clipboard, plus two scheduler bugs that surfaced while testing the rollover behavior.

### Nine slices in one session

1. **`JournalSettings.user_email`** — new field. Pins Gmail's `/mail/u/{address}` slot so multi-account users land in the right inbox; also feeds AppleScript's `sender` property in Native Mac Mail mode.
2. **Settings → Mail tab** — radio for `mail_send_mode` (Gmail / Native Mac Mail / Outlook), body-format toggle, Native-only HTML toggle, Outlook flavor (Business / Personal). `serde(default)` on every new field so older settings.json files load with Gmail defaults — no migration code, no users yet.
3. **Gmail compose dispatch** — `https://mail.google.com/mail/u/{ACCOUNT}/?view=cm&tf=cm&to=…&su=…&body=…`. `su` (NOT `subject` — Gmail uses the legacy name). All params encoded with `percent_encoding::NON_ALPHANUMERIC`. Warn-and-allow modal when the encoded URL exceeds 2000 chars (Gmail silently truncates above that).
4. **Outlook compose dispatch** — Business host `outlook.office.com/mail/deeplink/compose`, Personal host `outlook.live.com/mail/0/deeplink/compose`. Subject param IS `subject` here. Multi-account handled by Microsoft's account picker.
5. **Native Mac Mail via AppleScript** — spawn `osascript -` and pipe the script via stdin (sidesteps argv length cap on long bodies). Permission-denied detection on `-1743` / `Not authorised` substring in stderr surfaces an "Open Automation Settings" link to `x-apple.systempreferences:com.apple.preference.security?Privacy_Automation`. Backslash + double-quote escaping on every substituted value.
6. **Send-button rewire + universal Preview** — single dispatch point on `/summary` and `/journal` runs through `compose_weekly_email` and branches on `mail_send_mode`. Preview modal is always available now (Chris's decision — was previously HtmlEml-only). Native HTML mode shows the rich render in a sandboxed iframe; the other modes show plaintext in a `<pre>`. Heads-up tip styled to match the existing Reminders-tab notification-permission tip exactly.
7. **Reminder sleep-drift fix** — scheduler's `tokio::time::sleep_until` doesn't survive macOS hibernation cleanly. On wake, "now" can be hours past the scheduled instant and the next-fire calculation went stale. Reworked: each wake recomputes `chrono::Local::now()` and re-derives the next fire from there, so a Friday-4pm reminder that misses its slot because the laptop was asleep fires immediately on wake instead of sliding to the next configured day.
8. **Clipboard on Preview** — `tauri-plugin-clipboard-manager` `writeHtml` (HTML + plaintext fallback) in Native HTML mode, `writeText` everywhere else. Inline confirmation in the modal.
9. **Gmail flipped to default** — `MailSendMode::default() = Gmail`. Doesn't need Automation permission, works anywhere Chrome/Safari are signed into Gmail.

### Bonus: week-rollover fix

The 2.9b kickoff doc flagged a bug Chris was tripping on: capturing a Note first thing Monday morning was writing into the prior week's file. Root cause: `/capture` resolved the current ISO week once at component mount, and the same value rode through every subsequent submit. After a weekend hibernation the cached week was 7 days stale. Fix: resolve the week at the moment of write, not at mount. Same fix applied to the reminder scheduler (which had the same shape). New round-trip tests around the ISO-week boundary pin the behavior.

### Key risks mitigated

- **Gmail URL truncation** — silent above ~2000 encoded chars. Warn-and-allow modal lets Chris see the threshold instead of the recipient getting a half-clipped paragraph.
- **AppleScript permission denial** — first-time `osascript` failures surface a clear in-app link to System Settings → Privacy → Automation, not a raw exit code.
- **Multi-account Gmail routing** — `user_email` is preferred over the `/u/0` fallback so Chris's work and personal Gmail tabs stop fighting.
- **Sleep drift** — both `/capture` and the reminder scheduler now recompute time on every fire, not at boot.

### What Chris should smoke-test before sending his real report

- Switch through all three modes in Settings → Mail and confirm the Preview modal updates (HTML in Native HTML mode; plaintext elsewhere).
- Click **Copy to clipboard** in each mode — paste into a scratch buffer and confirm Native HTML mode pastes with formatting, others paste plain.
- In Native Mac Mail mode with HTML toggle ON, send to himself first — confirm Mail opens an editable draft (no Edit-as-New-Message dance) with the correct `sender`.
- In Gmail mode, click Send and confirm Gmail opens in `/mail/u/<his email>/…` (not `/u/0/…`) with To/Subject/Body all populated.
- Try sending a long summary (the kind that historically triggered the `.eml` fallback) and watch for the URL-length warning.
- Capture a Note on Monday morning and confirm it lands in the current week's file, not the prior one.
- Set a Friday-4pm reminder, sleep the laptop over a weekend, wake it Monday and confirm the reminder fires (or is correctly rescheduled to next Friday — whichever the spec lands on).

### Verification

- `cargo test`: 181 passed, 0 failed.
- `svelte-check`: 0 errors, 0 warnings, 196 files.
- No migration code shipped, by design — no users yet, `serde(default)` covers existing local settings.json files with Gmail defaults.

### Tomorrow

**Phase 2.8 — Custom Themes** is next. Already designed (see the 2026-06-26 entry above). Wide token surface, hex inputs with swatches, Theme = Light / Dark / Custom, JSON export/import via Tauri dialogs.

## 2026-06-29 (afternoon) — Phase 2.9c (Compose + paste, Settings restructure, editor rendering)

Started smoke-testing 2.9b's Gmail mode. Quick win confirmed: `writeHtml`-via-Preview-Copy → paste into Gmail web compose → recipient gets a fully formatted email. Two clicks in CaptainsLog (Send → Copy in Preview), one Cmd+V in Gmail, one Send in Gmail. Worked beautifully.

That triggered the "can we formalize this as a one-click mode?" thread that became 2.9c.

### Compose + paste mode (the headline)

The pattern: when sending, atomically (a) open the chosen client's compose with To + Subject pre-filled but body **empty**, and (b) write the rich-HTML body to the clipboard. The user lands in compose, presses Cmd+V in the body, hits Send. 2 clicks in CL + 1 keystroke + 1 click in client. Universal across Gmail / Outlook web / Mac Mail.

Architecture:
- **Setting:** `JournalSettings.mail_body_delivery: MailBodyDelivery` (`Prefilled` | `ClipboardPaste`, default `Prefilled`). Single global field, applies orthogonally to send mode. Picked Shape 2 from a small brainstorm workflow that compared three UX shapes — per-mode sub-radio, global radio, fold-into-existing-Body-format. Global radio is the simplest mental model (the user picks "do I want formatted or not" once); the other shapes either triple the settings surface or overload "format" with delivery-mechanism semantics.
- **Backend:** `MailSend.body_in_clipboard: bool`. Threaded into the dispatch branch in `email.rs::compose_weekly_email`. When set, body arg becomes empty string for the URL/AppleScript builders; truncation warning is forced off (empty body can't overflow). Native Mac HTML `.eml` path takes precedence (peer override — the `.eml` already carries a styled body without needing a paste).
- **Frontend:** in `confirmSend`, branch at the top: when `mailBodyDelivery === 'clipboard-paste'` AND not Native HTML, invoke `render_weekly_summary_preview` then `await writeHtml(html, text)` BEFORE invoking `compose_weekly_email`. If `writeHtml` throws, abort the openUrl entirely and surface an in-modal `clipboardPasteError` block with "Open Preview" recovery link. Silent empty-draft sends would be a worse failure mode than a visible error.
- **UX:** Send button label flips to `Copy + Open Gmail` / `Copy + Open Mac Mail` / `Copy + Open Outlook` when clipboard mode is active. Modal mode-tip swaps to "Opens X with an empty body and copies the formatted message. Press Cmd+V in the draft, then Send."

Also loosened the existing Preview-modal Copy button to ALWAYS use `writeHtml(html, text)` (it was branching on `previewShowsHtml` before). HTML-aware paste targets get rich content; plaintext targets get the plaintext fallback via OS pasteboard negotiation. This is the manual workaround Chris discovered during smoke testing; the new mode just automates it.

### Settings → Mail tab restructure

Chris's direction: Body delivery up top, Send-to-manager path at the very top of the Mail tab, Body format hidden when Compose + paste is selected. Reorganized to:

```
Settings → Mail
─────────────────────────────────
How should Send work?
  Send-to-manager path: [Gmail (recommended) ▼]
  Body delivery:        ◯ Prefilled draft
                        ◉ Compose + paste (formatted)
  [Body format hidden when clipboard-paste is selected]

Gmail / Native Mac / Outlook (per-mode sections with tips + sub-controls)
```

Three orthogonal choices, all in one top section. The Body format radio (Clean text / Markdown source) is only visible when Body delivery is Prefilled — Compose + paste hand-delivers rich HTML, plaintext flavor is moot. Native Mac's old 3-way "Body format" radio (Clean text / Markdown source / Styled HTML) was conflating `mail_body_format` with `mail_native_html`. Split into a clean two-way Body format radio at the top + a standalone "Send as Styled HTML draft (.eml)" checkbox in the Native Mac section, labeled as an independent peer override of Body delivery.

Forward-pointer tips on Gmail and Outlook sections now point at the new Compose + paste mode instead of the manual Preview → Copy workflow.

### Editor rendering bugs (the long tail)

While smoke-testing the mail path, Chris kept hitting list-rendering bugs. Wound up burning down a pile:

**Setext heading on `-`** — typing a paragraph, Enter, then `- ` re-rendered the paragraph above as an H2 (Setext-style underline). Fixed with `{ remove: ['SetextHeading'] }` in the markdown extension config. CaptainsLog only emits ATX (`#`) headings; Setext is pure conflict-with-list-syntax with no upside for us.

**Tab inserts literal `\t` instead of focus-traversing** — replaced `indentWithTab` from `@codemirror/commands` with a custom `listAwareTab` KeyBinding. Walks the lezer tree from the cursor head; inside `BulletList` / `OrderedList` / `ListItem` it calls `indentMore` / `indentLess`. Outside it returns `false` so the browser handles native Tab focus traversal. Fallback regex on the current line text for the cursor-on-blank-line-inside-list edge case where the tree resolves above the marker.

**Auto-continue Enter regression** — first pass disabled the markdown package's keymap (`addKeymap: false`) because I incorrectly attributed an empty-list rendering bug to it. The actual culprit was hang-indent CSS clipping bullets (next section). Re-enabled by importing `markdownKeymap` and slotting it into our `keymap.of` array AFTER `listAwareTab` but BEFORE `defaultKeymap`. Without that precise ordering, `defaultKeymap`'s `insertNewline` swallows Enter first and the auto-continue never fires.

**Hang-indent for wrapped list lines — second attempt** — first attempt was `padding-left: <depth>*2ch; text-indent: -2ch` on the line. Looked mathematically sound but ended up clipping the inline-block bullet widget in WebKit (the digits in numbered lists also went missing). Mechanism still unclear to me — text-indent should shift the inline-block by the negative amount but keep its width visible. Empirically: it doesn't, at least with how the bullet widget is structured.

Second attempt: `padding-left: <depth>*2ch` on the line, `margin-left: -2ch` on the marker widget itself. The marker pulls ITSELF back out of the padding to sit at the line's left edge. Content area starts at the padding boundary; wrapped rows naturally align there. Bullet widget keeps its full visible width — no clipping. Works for bullets and single-digit numbered lists; double-digit (`10.+`) ordered items visually overlap the content by 1ch — acceptable for now.

**Numbered list digits illegible in dark mode** — CodeMirror's default-highlight style colors `tags.processingInstruction` (which lezer-markdown assigns to every ListMark) with a near-background-color value. Bullet glyphs sidestepped this because BulletWidget is a Decoration.replace — the source `-` is gone, the widget is the only thing rendering. Numbered markers stayed as source text and inherited the unreadable color.

Tried two failed fixes first: (1) override `tags.processingInstruction` in our HighlightStyle, (2) wrap with `Decoration.mark` + class + inline style. Both got out-cascaded by CodeMirror's default-highlight selector for reasons I never fully traced.

Third attempt: full mirror of the bullet pattern. New `OrderedListMarkerWidget` class. `Decoration.replace` swaps the source digits for a `<span class="cm-md-list-num">{markerText}</span>`. Same DOM shape as the bullet, same CSS hooks (`color: --text-secondary; opacity: 0.75`). The widget's span is the ONLY thing the cascade sees — no syntax-highlight rule to fight against. Worked first try.

Lesson worth keeping: **when CodeMirror's default highlight style wins the cascade against your custom CSS, replace the source with a widget instead of wrapping it.** Replacement gives you sole authority over the rendering. Wrapping leaves CM's classes in play.

**Task list double-marker** — `- [ ] foo` was rendering as `• ☐ foo` (bullet AND checkbox). Fixed in two parts: (a) detect task list items by checking the parent `ListItem` for a `Task` child node, (b) when a task, still emit a `Decoration.replace({})` to hide the source `-`, but with NO widget — so the checkbox alone is the visual marker. First pass landed without (b) and left `- ☐ foo` (source dash visible). Quick follow-up fixed that.

### Verification

- `cargo test`: 191 passed, 0 failed (5 new tests for the empty-body URL/AppleScript builders + back-compat for `mail_body_delivery`).
- `svelte-check`: 0 errors, 0 warnings, 196 files (one false positive midway — Svelte's parser tripped on a literal `<style>` inside a script comment; rephrased the comment).
- Manual: smoke-tested Gmail / Native Mac / Outlook modes in both Prefilled and Compose + paste. Send-to-self in Gmail mode confirmed end-to-end. Real-world mail testing will surface anything we missed.

### Tomorrow

Chris's call: ship Phase 2.9c, start using the mail flow in earnest. **Phase 2.8 — Custom Themes** is the next planned phase (still as designed in the 2026-06-26 entry). Real-world mail use will likely surface bugs; if they accumulate they get folded into a 2.9d cleanup pass.

One known limitation worth tracking: ordered-list hang-indent for double-digit markers (`10.+`) visually overlaps the content by 1ch. Punted intentionally; revisit if anyone actually writes a 10+ item ordered list in a weekly summary.

## 2026-06-30 — Phase 2.8 (Custom Themes) + Colorful Labels

Long session. Custom Themes shipped in 6 build slices + 2 follow-on fixes; Colorful Labels rode on top of it as a small feature with its own 4 slices + 3 blocker fixes. By end of day Chris was using a Custom palette in the live app, smoke-testing without finding crash-level bugs.

### Phase 2.8 — Custom Themes

**Token surface.** 12 user-editable primaries (3 backgrounds, 3 text, 2 borders, 4 accents) feeding into ~23 OKLCH-derived dependents. The plan workflow surfaced the audit + algorithm + UI shape in parallel; the lens reports converged on culori (~12 KB, MIT, OKLCH-native) for the color math. Chris locked the 4 design questions (sapphire as the 12th editable accent, `.captheme.json` extension, error palette re-derives from user's `--accent-pink`, first-time activation seeds from active theme silently with an inline hint) — all "Recommended" defaults.

**Engine.** `$lib/theme.ts` exports `deriveTokens(primaries, base) → DerivedTokens`, `applyCustomTheme(derived)`, `clearCustomTheme()`, `contrastRatio(fg, bg)`, plus `SHIPPING_DARK_PRIMARIES` / `SHIPPING_LIGHT_PRIMARIES` constants. Walks OKLCH: detect base polarity from `--bg-surface` luminance, inherit hue from the surface (warm-cream surfaces → brown-black text automatically), iterate L until AA hits (4.5:1 text, 3:1 focus/UI). 38 vitest cases pin the behavior — including the reproduction tests (shipping Light and Dark are reproduced within OKLab dE ≤ 0.04 when seeded with their primaries).

**Convergence fallback.** The first iteration of `iterateForContrast` silently returned the last candidate when it couldn't hit the target — that violated the locked "AA on every derivation" constraint. Fix: return `{value, ratio, converged}` and on non-convergence swap in better-of-`{#000, #fff}` against the host surface. Independently derive `targetEnd` from `inferBaseFromSurface(host)` rather than the global base arg, so a pale surface marked Dark still walks toward black. New test case at `bgSurface = #7e7e7e` (true mid-grey, where the walk bottoms out) actually exercises the fallback path. Earlier `#aaaaaa` would converge on the primary walk path and never hit the fallback — coverage gap caught by the recheck.

**Persistence.** `AppSettings.theme` widened to `Light | Dark | Custom`. New `CustomTheme` struct with 12 hex fields. Per-field deserializer (`deserialize_hex6`) accepts only `^#[0-9a-fA-F]{6}$`, normalizes to lowercase. `serde(default)` so older `app-settings.json` files load cleanly with `theme: Dark, custom_theme: None`. Switching to a preset preserves the saved `custom_theme` payload — verified explicitly by `update_settings`.

**Editor UI.** 4 section groups in the Theme tab (Backgrounds / Text / Borders / Accents), per-token row with 28×28 swatch + monospace hex input + inline AA contrast warnings under offending rows. Live preview on every keystroke. `customEditorDirty` flag flips on first edit, cleared on save/cancel. Re-clicking the active radio is a no-op. Switching Custom → Light → Custom preserves in-flight edits.

**Export / Import.** `.captheme.json` schema (`$schema`, `name`, `author`, `base`, `tokens` with 12 hex keys). Tauri save/open dialogs. Strict validation on import. An "import pending" flag triggers a Cancel-guard prompt — clicking Cancel before Done asks before discarding the imported palette (the toast wording also shifted from "Theme loaded." to "Theme loaded — click Done to keep it." to set the right expectation).

**Day-pill regression chase.** The token prep pass missed `.day-pill.active` hardcoded `color: #1f0a02`. First fix swapped to `var(--btn-primary-text)` — clean on Dark, but Light theme's `--btn-primary-text: #ffffff` defaulted to white on orange at ~3.1:1 (caption text fails AA). Fix landed: Light theme block now defines `--btn-primary-text: #1f0a02` so day-pills + primary buttons stay legible in both shipping themes. Custom themes still get the derived dark-or-white based on accent saturation.

**Tray-menu escape hatch.** Chris noted "you can really get yourself into trouble making literally everything the same color" — added a **Preset Theme** submenu to the tray icon's right-click menu with Dark / Light items. Loads `AppSettings`, flips `theme`, broadcasts `settings-changed`. `custom_theme` payload survives — escape is reversible from Settings once the user can see again. Helper runs on the Tauri async runtime since `on_menu_event` is sync.

### Colorful Labels (2.8 follow-on)

Same session, layered on Custom Themes. The toggle gives each label a per-name hue (djb2 hash → 0-360 hue, fixed chroma 0.12, theme-aware lightness).

**Design pivot mid-build.** The first slice plan persisted generated colors lazily — render a chip, fire-and-forget `set_label_color` to bake the hex into `labels.json`. Skeptic caught the theme-burn: a Dark-generated hue at L=0.70 becomes invisible under Light (~1.6-2.1:1 contrast). Pivot: **no lazy-persist.** `colorfulChipStyle` computes via `generateLabelColor()` at render time using the active theme's surface; `labels.json` `color` field is reserved for explicit user overrides from the future Label Manager. The `set_label_color` Tauri command stays in place for that future use. Same name → same hue across sessions (deterministic hash) but adapts to whichever theme is active when rendered.

**Concurrency hardening.** Two label-mutating commands (`create_note` and `set_label_color`) both load-mutate-save `labels.json`. They were taking `storage.read()` locks; a fire-and-forget color write racing a count bump could lose one or both. Fix: both now take `storage.write()` locks. Also: `LocalFilesystem::write_metadata` now stages to `<name>.tmp` and renames over the target — readers either see pre-write or post-write content, never a torn file. 3 new concurrency tests (10 parallel `set_label_color` calls all persist; color + count both survive on overlapping label; stranded `.tmp` doesn't corrupt the destination).

**Svelte 5 reactivity gap.** `colorfulChipStyle` reads `document.documentElement` at render time for the active theme + `--bg-surface`. Svelte can't track DOM reads as reactive dependencies, so a Custom-theme `bgSurface` tweak left chips painted with stale colors until remount. Fix: `themeNonce = $state(0)`; `settings-changed` listener bumps it on every emit; `colorfulChipStyle` reads `themeNonce` at the top of its body (bare expression, enough to register the dep). Now the chain reactively propagates: `update_settings` → `settings-changed` → `themeNonce++` → `colorfulChipStyle` re-runs → fresh DOM read → fresh chip color.

**Concurrency test coverage caveat.** The new tests reach past the Tauri command wrappers and call the `_impl` helpers directly. They prove the lock pattern's data integrity but not that the wrappers themselves take a write lock — verifiable by inspection. A future refactor that downgrades to `.read()` wouldn't be caught. Acceptable for now; a real wrapper test would need `tauri::test::mock_builder`.

### Lessons worth keeping

- **When CodeMirror's default highlight style wins the cascade against your custom CSS, replace the source with a widget instead of wrapping it.** (Already in the 2026-06-29 entry; came up again today indirectly when designing the OKLCH widget approach for label hues.)
- **For derivation engines, return `{value, converged}` not just `value`.** A silent best-effort that misses the target is the worst failure mode — looks like a bug, gets shipped, gets discovered by a user. Make non-convergence a typed result; fall back to a known-good value.
- **Lazy-persist is theme-coherence's enemy.** Any computed value that's a function of the active theme should be RE-computed on theme change, not cached to disk. The "save it once, read forever" pattern only works when the inputs to computation never change.

### Verification

- `cargo test`: 207 passed, 0 failed.
- `vitest run`: 38 passed (theme.test.ts).
- `svelte-check`: 412 files, 0 errors, 0 warnings.
- Manual: Chris ran the app with a Custom theme end-of-day, switched between Light/Dark/Custom via the in-app picker AND the tray menu, toggled Colorful Labels on/off, didn't surface anything crash-level. Real-world use will keep flushing bugs.

### Tomorrow

Chris is using 2.8 + Colorful Labels in earnest and will report bugs as they appear. **Phase 3 — Search & Navigation** is the next planned phase (full-text across weekly files, label/date/file filters, click-to-jump, year/week tree at scale). If 2.8 bugs accumulate they get folded into a 2.8c cleanup pass before Phase 3 starts.

## 2026-06-30 (later) — Phase 2.8c: onboarding polish + Preview modal refactor

Same day, later session. Chris asked for a settings-file delete to retrigger the first-run wizard and also flagged a Gmail + Compose+paste clipboard-skip he'd hit "but couldn't reproduce." Both threads turned into a small polish phase that absorbed a shared-component extraction pass and a SendToManagerButton Preview refactor onto the shared Modal.

### The Gmail clipboard-skip bug

Root cause was a tighter-than-it-looks frontend condition. `confirmSend` had:

```ts
if (mailBodyDelivery === 'clipboard-paste' && !mailNativeHtml) {
  await writeHtml(html, text);
}
```

Reads correctly at first glance — "if clipboard-paste mode and not Native HTML, write HTML to clipboard." The bug: the backend's peer-override is `mode == NativeMail && native_html`, NOT `native_html` alone. If `mailNativeHtml` had ever been flipped to true (e.g. from Chris experimenting with Native Mac mode earlier), the Mail tab's UI hid the toggle whenever the user wasn't in Native Mac mode — so once it was stuck on, there was no way back from Gmail mode to flip it off. Frontend skipped `writeHtml`, backend correctly emitted an empty-body Gmail URL (because `body_in_clipboard` was true), result: open compose, paste nothing, send empty. Silent failure mode is the worst kind.

Fix: drop the `&& !mailNativeHtml` guard. Clipboard always populates in clipboard-paste mode. Backend keeps its existing peer-override handling for the Native Mac `.eml` path (where the styled body's already in the message, no clipboard needed). Updated `previewShowsHtml` to match (so the Preview iframe also reflects what'll actually be on the clipboard).

Lesson worth keeping: **when frontend and backend each have their own version of "should I do X?" logic for the same setting, they will eventually disagree.** The fix is to make the frontend ask the backend (or share a derivation), not to manually re-derive it. Filed mentally for the next time this comes up — for 2.8c we just patched the divergence point.

### Shared component extractions

Onboarding had grown 5 steps; settings had 3 tabs each with their own modal patterns; send-to-manager had its own inline backdrop. Component duplication was getting silly. Extracted:

- **`Modal.svelte`** — backdrop dim + 8px blur, body-scroll lock, topmost-Escape stack (so a nested Modal closes before the parent), focus restore on close, `zLayer` prop (`base` / `nested`) for the rare case of stacked modals, `maxWidth` prop. Owns the chrome; consumer slots body markup + actions row.
- **`ConfirmDialog.svelte`** — thin wrapper that hands Modal `zLayer="nested"` and a standard message + actions row shape. Used by the unsaved-work-prompt-at-quit + Theme tab's "discard pending import" prompt + the Discard-draft confirmation.
- **`LoadingOverlay.svelte`** — reusable spinner with optional message string. Sits inside a Modal when shown.
- **`PointerFinger.svelte`** — 32×32 sprite (from `ui-guide-hands`) + bob animation. Restored from an earlier-refactor regression where someone (probably me) inlined the sprite into a single step and the abstraction got lost.
- **`StepHeader.svelte`** — h1/h2 + `.lead` block shared across all 5 wizard steps. Levels chosen per-step: h1 on Intro + Complete (top-of-flow page titles), h2 on the three form steps in the middle. Tried tightening h1's `.lead` margin during the extraction; regressed the Welcome / All-set screens visually; reverted to a single rule. Note in the file explains why; if anyone tries that "optimization" again the comment will catch it.
- **`PathPickerField.svelte`** — label + text input + Browse… button + hint + warning microcopy. Wraps the Tauri `tauri-plugin-dialog` open-folder invocation. Onboarding step 4 + `/settings` journal-location row use it; future backup/export destinations can plug in for free.

Cross-app side-effects of having the chrome centralized: dim+blur backdrops applied everywhere (some surfaces had a flat scrim before); Title Case button labels became the universal convention (was inconsistent); btn-emerald / ruby / marble color tokens carried through every confirm dialog. Also fixed the radio-circle treatment in Theme and Mail tabs — both were rolling their own; replaced with a `:has()` pure-CSS selector that hooks into the standard input + label markup so future radio groups inherit it automatically.

### SendToManagerButton Preview refactor

Chris's four asks (verbatim):
1. Is the Preview popup using our shared popup components? (No — it had its own inline backdrop.)
2. "Close preview" → "Close", "Copy to clipboard" → "Copy To Clipboard".
3. Buttons side-by-side, lower-right, Close immediately to the left of Copy — like every other modal.
4. Show the fully formatted HTML render when Body delivery is Compose + paste (was falling back to plaintext).
5. Add a "From:" line at the top showing the user's email address.

Refactored the markup to wrap with `<Modal zLayer="nested" maxWidth="min(640px, calc(100vw - 32px))">`. From line is gated on `userEmail` being set — falls off cleanly when it isn't. `previewShowsHtml` derived widened to include `clipboard-paste`, so Compose+paste now shows the iframe render and Chris can see what the recipient will actually get.

Button placement got an iteration. First pass: `justify-content: space-between` so Close was far-left and Copy was far-right. Chris pushed back — the convention everywhere else is Close + Copy together in the lower-right corner, like a system dialog. Reshuffled the DOM so the status pill renders FIRST in the actions row, gave it `margin-right: auto`, and let the row inherit `.modal-actions`' `justify-content: flex-end`. Result: pill anchors left when present, Close + Copy hug right with their normal `gap-3` spacing between them. Works for the 2-button (no pill) case AND the 3-element (pill present) case without needing per-child overrides.

Cleanup: removed `.preview-backdrop`, `.preview-modal`, `.preview-modal iframe`, `.preview-recipient`, `.preview-close` CSS (Modal owns those concerns now). Added `.preview-header-line` (the From: + To: text) and `.preview-iframe` (carried over the iframe styling from `.preview-modal iframe`). Kept `.preview-plaintext`, `.preview-note`, `.preview-actions`, `.copy-status` — still in use.

### Confirm-modal refactor (the "one last fix" of the day)

Chris caught at end of day that the OTHER popup in `SendToManagerButton` — the "Send weekly summary?" confirm that opens BEFORE the Preview — was still on its own inline `.modal-backdrop` + `.modal` markup. Embarrassing miss on my part during the first 2.8c pass; the Preview refactor was the obvious target and the confirm hid behind it.

Same pattern as Preview: wrap with `<Modal open={true} onClose={dismissConfirm} title="Send weekly summary?" maxWidth="min(520px, calc(100vw - 32px))">`. Title prop carries the heading so the inner `<h2>` goes away. The body's prose, conditional warnings/errors, and 3-button actions row (Send / Cancel / Preview, or Send anyway / Cancel / Preview in the truncation-warn case) all move inside the Modal's children slot wrapped in a `.send-confirm-body` div. Paragraph styling + `<strong>` font-treatment re-scoped to `.send-confirm-body p` / `.send-confirm-body p :global(strong)` so they don't drift across the component's other contexts.

The cleanup-side benefit: dropped a window-level `escListener` block + its `let escListener` declaration + the `onDestroy` removal. Modal's topmost-stack Escape listener owns it now for both Confirm AND Preview. Preview's `onClose={closePreview}` already bumps `previewToken` (which is the bookkeeping the legacy listener existed for), so deleting the local handler doesn't lose any cancellation behavior — `closePreview` was the right home for that logic all along.

Verified `dismissConfirm`'s `if (isSending) return;` guard still works correctly: Modal's backdrop click + Escape both route through `onClose={dismissConfirm}`, so the existing guard keeps the modal sticky while a send is in flight. No new state needed.

Lesson: **when refactoring "one of two popups in this file," check the other one in the same pass.** Chris had to flag it as a separate ask. Five-minute fix, but a polish phase isn't really "done" if half the chrome is still inline.

### Lessons worth keeping

- **When frontend and backend each derive "should I do X?" from the same setting, they will eventually disagree.** Share the derivation or have one ask the other. Don't re-implement the logic on both sides.
- **`justify-content: space-between` is not the same as "lower-right corner buttons."** Close + Copy together on the right is the convention; status pills get their own anchor (`margin-right: auto` on the pill is cleaner than `margin-left: auto` on a button).
- **A bug that only reproduces with stale state from a prior session is the worst kind to debug** — Chris said "I'm not sure how it happened to be honest." It happened because Native Mac mode hid the toggle that would have let him un-stick the flag. Defensive UX wins: never hide a control that's currently affecting behavior.

### Verification

- `svelte-check`: 420 files, 0 errors, 0 warnings.
- Manual: deleted `~/Library/Application Support/com.prodigygame.captainslog/app-settings.json` to retrigger the first-run wizard; walked through all 5 steps with the new shared components in place; smoke-tested the Preview modal on Gmail / Native Mac / Outlook in both Prefilled and Compose+paste modes; verified Close + Copy placement matches `/journal` + `/summary` + ConfirmDialog buttons; confirmed From line only renders when `user_email` is set.

### Tomorrow

Chris is going to use the app end-to-end with the new chrome + Preview refactor in earnest. **Phase 3a — Label Library viewer + bulk management** is still the next planned phase. Any 2.8/2.8b/2.8c bugs that surface get folded into a small cleanup pass before 3a kicks off.

## 2026-07-06 — Phase 3c task-list design brief

Chris back after a week off — the app rolled the ISO week over cleanly, reminders fired, no crash reports. Built the .app for internal team testing (arm64, ad-hoc signed, 7 MB DMG). Passed a docs sanity check on the in-app Help + Nerds Only popups (`help-content.ts`) — added sections for send-to-manager, weekly reminders (merged with Noot), themes/colors, Rust backend modules, and the OKLCH walker; small factual fixes for the 4th summary field and Esc scope.

Then Chris outlined Phase 3c: a task-list aggregator that pulls `- [ ]` items out of every weekly file's "Plans and priorities for next week" section, displays them on the main screen, checks them off with bidirectional sync back to markdown, and (opt-in) rolls completed tasks into next week's Key Accomplishments on ISO week boundary.

### The core design questions

Chris's spec was mostly locked but four things needed thinking:

1. **Scope** — only source from Plans-for-next-week, or any `- [ ]` line anywhere? Chris: narrow scope, room to expand later if asked.
2. **Task identity** — the load-bearing hard problem. Markdown has no stable task ID; line numbers drift; text can change externally. Needed something accurate, simple, easy-to-explain, robust.
3. **Sidecar for completion timestamps** — must survive tampering / deletion.
4. **Rollover trigger** — on ISO first-day-of-week. Handle both app-left-open-over-weekend AND app-opened-fresh-in-new-week.

Ran a 4-way research workflow (Obsidian Tasks / Logseq / Things 3 / Bullet Journal) to see how prior art handles these. Findings:

**Obsidian Tasks** is architecturally closest to us — parses `- [ ]` across markdown files. Their identity model is content-based (task text + file coordinate); they have NO persistent sidecar and NO external-edit detection. Their emoji-metadata format (📅, ✅, 🆔) has known Unicode / non-breaking-space fragility that breaks the parser — a specific don't-do that shipped for us.

**Logseq** uses inline block-reference UUIDs (`((abc123))`) for identity. Powerful but noisy in the markdown; their own export tool strips them because they hurt portability. Off-target for our "keep the markdown clean for external editors" goal.

**Things 3** doesn't use markdown so their storage is irrelevant, but their UX restraint is the lesson: no gamification, no metrics, no clever features. Their Logbook (completed tasks) is visible but muted — small but non-zero signal that hiding all completion history erases the satisfaction loop.

**Bullet Journal** was the surprise-most-useful reference. Their signifier system (dot / x / arrow / angle-bracket for open / done / migrated / scheduled) shows a richer state model exists but v1 doesn't need it. Their migration ritual — the deliberate act of moving un-done tasks between periods — maps beautifully to our week-rollover feature, but they warn hard against automating migration without reflection. Hence the receipt toast + Undo pattern in our design.

### Locked task identity

Composite key: `(weekId, normalizedTextHash)`. Normalize by trimming, collapsing internal whitespace, lowercasing, stripping trailing punctuation. Sidecar (`.metadata/task-completions.json`) is a rebuildable cache keyed by the same composite, storing only `completedAt`. Duplicate task text in the same week gets an `ordinal` disambiguator that only kicks in when duplicates exist.

Reconciliation rule: **markdown wins for checkbox state, sidecar wins for timestamps.** Every load scans the weekly files and reconciles: missing sidecar entry for a `[x]` line → backfill from file mtime; sidecar entry for a `[ ]` line → drop; sidecar entry with no matching file line → GC. External text edits that materially change the task turn it into a new task (old entry GC'd) — matches BuJo's model where "rewriting IS reconsideration."

User mental model, verbatim from the synthesizer: *"A task is a `- [ ]` line I wrote in Plans and priorities for next week. Checking it anywhere marks it done. If I rename the text substantially, it becomes a new task — same as crossing it out and writing fresh."*

### What we're stealing from prior art

- "Rebuild task index" as a first-class UI command (Obsidian Tasks proves rebuild-from-markdown always works; ours needs to be discoverable but not intrusive — Settings tab button + tip).
- Aggressive text normalization before hashing so users can retype the same task and get the same identity.
- Silent reconciliation on load with deterministic merge rules — Obsidian Tasks' silence on external edits is bad, but constant modal prompts are worse.
- "Written in Week N" badge on every task — BuJo's origin-date-not-deadline pattern, gives us staleness signal for free.
- Receipt toast + Undo on rollover — BuJo's core lesson that automation without reflection is the anti-pattern.
- "Rolled over from Week N" badge in Key Accomplishments so the lineage survives after the toast.

### What we're NOT doing

- No inline ID tokens (`[id:abc]`) in the markdown — Logseq's own docs say they strip these on export because they hurt writing experience.
- No emoji metadata sigils — Obsidian Tasks' Unicode fragility is a well-documented parser trap.
- No due dates in v1 — Things 3's restraint + BuJo's "origin dates, not deadlines" both validate.
- No richer task states (cancelled / scheduled / migrated) in v1 — plain `- [ ]` / `- [x]` only.
- No gamification.
- No inline task editing in the aggregator — open `/journal` to edit text.

### The four open questions and Chris's calls

1. **Rollover conflict** (user already wrote in this week's Key Accomplishments before rollover fires) → append below existing content under a "Rolled over from Week N" subheading.
2. **Backfill for missing completedAt on first scan** → file mtime, em-dash if unreadable. Fine to have imprecise timestamps for pre-existing checks since 3c isn't shipped yet.
3. **Duplicate task text same week** → silent ordinal disambiguator. Warning users about duplicates would nag.
4. **Rebuild command placement** → Settings → Task List tab, button with a tip. Discoverable, not intrusive.

Chris also confirmed the **hide-completed default = on** (immediate hide when checked, user can toggle off to keep completion history visible).

Full Phase 3c spec is in ROADMAP.md.

### Next

Phase 3a (Label Library viewer + bulk management) still comes first. Phase 3b (Search & Navigation) second. Phase 3c (Task list aggregator) third. If any of the Phase 2.6–2.8c code surfaces bugs during team testing this week, those fold into a small cleanup pass before 3a kicks off.

## 2026-07-06 (later) — Phase 3a: Label Library drill-down + bulk ops

Same-day continuation. Team testing on the fresh build didn't surface crash-level bugs so we jumped straight into 3a.

### Scope reality check

Started with a survey of what's already on disk (Phase 2.8b did more than I remembered): `rename_label`, `delete_label_cascade`, `set_label_color`, `get_label_stats`, `rebuild_label_index` all shipped. `LabelDetailsModal` already covers per-label color / rename / delete with a stats section and drift-detection banner. The Labels tab in Settings already lists + filters. The ACTUAL missing pieces for 3a boiled down to two things: (1) a label → notes reverse lookup with a drill-down UI, and (2) multi-select + bulk ops on the Labels tab.

Also dropped bulk-rename from scope. The original ROADMAP mentioned "bulk rename / merge / delete" but the design has converged: `rename_label` already auto-merges when the target name exists (Phase 2.8b behavior), so "bulk merge" IS "rename N labels into a canonical," and a distinct bulk-rename UI would be redundant. Chris confirmed.

### Slice 1 — Referenced In drill-down

**Backend.** New `get_notes_for_label(name)` Tauri command in `commands.rs`. Walks every weekly file (years desc, weeks desc), runs `scan_label_sites`, filters to sites containing the target label, returns one `LabelReference { year, week, kind, noteTimestamp, noteTitle }` per site. Kind is `"note" | "summary"` as bare lowercase strings so the frontend switches on the raw value with no mapping layer.

For Note references we walk backward from the labels-line byte offset to the nearest `### YYYY-MM-DD HH:MM — Title` heading via a new `extract_note_heading_before` helper. Discriminates Note headings from Summary subsection headings (`### Key accomplishments`, etc.) via the same 10-byte ISO-date-prefix check that `scan_label_sites` uses forward-direction; promoted `is_iso_date_prefix` to `pub(crate)` for the reuse.

Six new tests: 4 unit tests for `extract_note_heading_before` (timestamp+title, no-title, rejects Summary subsections, returns None when no heading precedes), 2 integration tests against `LocalFilesystem` (cross-year + cross-week ordering; both site kinds surface with note metadata). Full test suite: 248/248.

**Frontend.** Added a "Referenced In" section to `LabelDetailsModal` between Usage stats and the Color/Rename/Delete blocks. Stats + references fetch in parallel via `Promise.allSettled` so one failing doesn't hold the other's spinner. Row shape: kind badge (Notes get a warmer accent-tinted background) + note title + optional timestamp + `YYYY-Wnn` label + chevron.

The list caps at 50 rows in the DOM — a heavily-used label ("todo" on a 2-year journal) could theoretically produce 300–500 rows. When we truncate, a `TipBubble` below the list explains ("Showing the 50 most recent references (out of N). Older matches are hidden here…"). Chris and I discussed the number — 50 fits the modal comfortably, and users needing more than that are better served by opening `/journal` and browsing directly.

Click a row → `onClose()` (unmounts modal cleanly, resets `/settings` state) then `goto('/journal?year=Y&week=W')`. `/journal` reads the URL params on mount, expands the target year node in the sidebar tree (loading its weeks if it isn't the current year), and calls `selectWeek`. Defensive: bad params, or a URL pointing at a week that no longer exists on disk, falls through silently to the empty-state pane.

### Slice 2 — Multi-select + bulk delete + bulk merge

Chris's spec: checkboxes on every row, select-all, action toolbar appears when N > 0, one confirm dialog per batch (not per item), merge picker gets a radio-select of the selected labels for canonical target.

New Settings-scoped state: `selectedLabelNames: Set<string>` (name-keyed so the set survives filter/sort changes and Details-modal mutations); `showBulkDeleteConfirm` / `showBulkMergePicker`; `bulkMergeCanonical: string | null`; result banner state.

Toolbar renders above the list when there are labels at all. Left side: select-all checkbox + counter ("Select all" / "N selected") + Clear link. Right side: Delete N (ruby) and Merge into… (marble, disabled with a tooltip until 2+ selected). Toolbar right stays empty at N=0 so users who aren't selecting don't see irrelevant chrome.

Bulk delete uses the shared `ConfirmDialog`; bulk merge uses the shared `Modal` directly because we needed the primary-action-disabled state that ConfirmDialog doesn't expose. Merge picker's radio group pre-selects the highest-count label (ties broken alphabetically) so the "obvious" canonical is the default. Confirm on delete loops `delete_label_cascade`; confirm on merge loops `rename_label(source, canonical)` for every non-canonical source. Both continue past individual failures per Phase 2.8b's locked posture (don't roll back on partial failure; surface what couldn't be touched).

Result banner persists above the list after a bulk op with a `×` dismiss. Errors flip to the pink-tint variant via `.is-error`. Cleared automatically when the user modifies selection — the receipt is stale once a new op is being built.

Interaction cases: Details-modal rename/delete on a bulk-selected label prunes it from the selection on the next fetch (`onLabelMutated`); filter change mid-selection keeps outside-of-filter selections but the select-all checkbox reflects "all visible selected"; partial failures show aggregated summary listing which labels errored.

### Lessons worth keeping

- **Survey what exists BEFORE re-litigating the plan.** The ROADMAP entry for 3a mentioned "bulk rename/merge/delete" as one bullet; the actual scope was much smaller once I realized `rename_label` already auto-merges. Would've saved ~15 minutes to Explore-agent that first instead of taking the ROADMAP at face value.
- **Cap DOM row counts before shipping list UIs.** Chris asked "how big does this get?" the moment he saw the drill-down design. Realistic sizes are fine but heavy usage tails into unusable territory; a hard cap + explanation TipBubble is cheaper than adding pagination / filtering later.
- **Multi-select selection is name-keyed, not index-keyed.** Using indices would break the moment the filter changes or a Details-modal mutation reorders the list. Set-of-strings is the right shape.

### Verification

- `cargo test --lib`: 248 passed, 0 failed (was 242 before Slice 1's tests).
- `svelte-check`: 420 files, 0 errors, 0 warnings across all Slice 1 + Slice 2 changes.
- Manual: Chris smoke-tested the drill-down + bulk delete + bulk merge on his real journal. No crash-level issues; the "looks like this is all working well" verdict.

### Next

Phase 3b — full-text search across every weekly file, filter by label / date range / file, click-to-jump to the right week. Builds on the label→notes drill-down plumbing from 3a (same result-list + goto-with-URL-params shape, just generalized to arbitrary content matches).


