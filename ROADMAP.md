# Captain's Log — Roadmap

## Current phase: 2.5 + 2.5b ✅ done — next up Phase 2.7 (onboarding + Settings revisit)

Phase 1 MVP and Phase 2 polish are complete. Phase 2.6 ("Send weekly summary to manager") shipped 2026-06-24. Phase 2.5 (editor upgrade, Architecture B live-preview) shipped 2026-06-25 — Slack/Typora-style marker hiding on CodeMirror 6 with markdown-on-disk; live-preview engine, widgets (date chip + picker, bullets, task checkboxes), toolbar overhaul, /journal Preview/Source toggle, layout chrome polish, and an architecture doc all landed in a single session.

**Next up: Phase 2.7** — onboarding wizard expansion + Settings layout revisit. Phase 2.5 Steps 5-7 (HTML email body + preview modal) remain deferred behind 2.7 since the Send flow already works in plaintext via 2.6.

---

## Phase 0 — Planning & Scaffolding ✅

- [x] Project folder + docs (README, ROADMAP, DESIGN, DEVELOPMENT-JOURNAL, STYLE-GUIDE, docs/*)
- [x] Tech stack locked (Tauri 2.0 + Svelte 5 + TypeScript + SvelteKit static adapter)
- [x] Brand styling pulled from RPG game (Paytone One + ABeeZee fonts, Lucide icons, 4px grid, signature drop-shadow button)
- [x] Git initialized; private repo at github.com/ChrisACarpenter/captains-log

## Phase 1 — MVP: "Can I capture a Note?" ✅

- [x] Tauri app shell that runs on macOS
- [x] Menu bar icon (Lucide book template image, macOS-native dark/light adaptive)
- [x] Quick-capture popup (one click from menu bar → one click to submit) *(dedicated popup window landed in Phase 2)*
- [x] Writes Notes into the current week's markdown file at `journals/YYYY/YYYY-Www.md`
- [x] Creates the weekly file (with empty Summary scaffold) if it doesn't exist
- [x] Basic Labels field with manual entry (no autocomplete yet)
- [x] Body text supports inline `#hashtags` as labels (basic parsing, no autocomplete yet)
- [x] All labels write to `journals/.metadata/labels.json`
- [x] First-run setup: name, journal location *(shipped in Phase 2 Option B)*
- [x] Theme infrastructure (CSS variables for dark + light; dark hardcoded for v1)

**Success criterion met:** captured multiple notes across an afternoon; markdown files contain frontmatter, Weekly Summary scaffold, Weekly Notes section, timestamped notes with labels and inline `#hashtags`.

## Phase 2 — Polish: "Can I actually use this daily?" ✅

### Done

- [x] Dedicated quick-capture popup window (label `capture`, 460×460, hidden by default, opens via tray)
- [x] Dock icon in addition to menu bar — both shipped
- [x] First-run wizard (4 steps: welcome → name → location → reminder)
- [x] Two-tier settings — app-level (`~/Library/Application Support/.../app-settings.json`) + journal-level (`<root>/.metadata/settings.json`)
- [x] Folder picker via `tauri-plugin-dialog`
- [x] Optional weekly reminder notification (real macOS notifications via `tauri-plugin-notification`)
- [x] In-process scheduler restart on settings save (no double-restart needed)
- [x] RPG petbook icon as the app icon (Dock, Finder, Cmd+Tab)
- [x] RPG scroll icon as the reminder notification icon
- [x] Settings panel — `/settings` route, full form (name, location, reminder, theme), all changes apply in-process
- [x] Light/dark theme toggle in Settings (live preview, persisted in app-settings.json)
- [x] Label autocomplete (chip-based JIRA-style dropdown with per-label color from accent palette, fed by `labels.json`)
- [x] `labels.json` schema normalized to camelCase (snake_case alias kept for backwards compat)
- [x] Weekly Summary UI (the 4-field Lattice template at `/summary`)
- [x] Hot-swap `LocalFilesystem` when journal_root changes — settings panel changes apply in-process, no app restart needed
- [x] **Theme v2 — Embered + Week Stripe.** Both themes overhauled after a 6-lens adversarial critique. Warm-tinted neutrals across both modes; split border tokens (decorative orange vs structural neutral); tokenized previously-hardcoded values (focus glow, sapphire bg, marble colors); WCAG 2.2 contrast fixes on light focus ring + text-muted; new `--bg-code` surface for Phase 2 markdown editor.
- [x] **Week Stripe** at the top of the main window — a 4px Prodigy-orange progress meter (track + fill) that grows across the week. Earns its position by being load-bearing on day 1, not decorative chrome.
- [x] **Noot reminder marker** — when a weekly reminder is set, a small Noot mascot hangs on the stripe at the reminder day/time position. (`npc-noot-small` extracted from `ui-login-credentials` atlas.)
- [x] **Wizard guide hand** — rotated `pointer-hand-straight` sprite (from `ui-guide-hands`) bobs gently next to the active input on first-run setup steps.
- [x] **Weekly Summary labels field** — new `### Labels` subsection in the markdown, chip-based LabelInput on the form, full parse/render/replace pipeline + 3 new tests.
- [x] **Settings-changed event broadcast** — `update_settings` and `complete_first_run` now emit a Tauri event so the capture popup re-applies the new theme without restart, and the week stripe makes Noot appear/disappear immediately after a reminder toggle.
- [x] **Window state persistence (`tauri-plugin-window-state`)** — both windows remember size + position across launches. `VISIBLE` flag dropped from defaults so the capture popup respects `visible: false` on launch.
- [x] **macOS notifications via `UNUserNotificationCenter`** — migrated from the deprecated `NSUserNotification` (`mac-notification-sys`) to the modern API (`mac-usernotifications`). Hybrid path: production `.app` → UN with action buttons + permission prompt + persistence; `tauri dev` bare binary → legacy fallback (no crash, no buttons, but functional for testing the wiring).
- [x] **Production .app codesigning** — `bundle.macOS.signingIdentity = "-"` in tauri.conf.json so Tauri's bundler runs a real `codesign --force --sign - --identifier com.prodigygame.captainslog` pass. Required for `usernotificationsd` to accept the UN auth request (it keys permission off the codesign Identifier, not CFBundleIdentifier).
- [x] **Persistent Alert Style hint** — settings page explains that macOS defaults new apps to "Temporary" notifications (auto-dismiss, hide action buttons) and provides a one-click deep link to the Captain's Log notification preference panel via `x-apple.systempreferences:`.
- [x] **Close flow — Option B** (`.Accessory` activation policy). Red X on main hides + flips to `.Accessory` (Dock icon hides, tray persists). Tray right-click menu with **Show Captain's Log** / **Quit Captain's Log**. Custom macOS app menu (with Cmd+Q routed through our handler, not the predefined Quit that bypasses it). Cross-window `DirtyRegistry` + native NSAlert prompt at quit time if any surface has unsaved work.

- [x] **Auto-save Phase 1** — Weekly Summary debounced auto-save (1.5s after typing stops). Status indicator beside the Save button: `Saving…` / `Saved HH:MM` / `Unsaved changes` / `Couldn't save — retry?`. Manual Save still works as force-immediate. After auto-save, summary leaves the dirty registry.
- [x] **Auto-save Phase 2** — Capture popup draft persistence. Drafts at `<journal>/.metadata/capture-draft.json`; load on mount, debounced save on change, clear on Submit. `Draft saved HH:MM` indicator below the actions row. New `delete_metadata` trait method on `StorageBackend`. Also adds a Ruby **Discard** button with native confirmation that cancels pending saves, deletes the draft, and hides the popup.
- [x] **Auto-save Phase 3** — Stripped the red-X prompts. Red X now hides main + capture silently (no dialog). Cmd+Q / tray Quit still uses the unsaved-work guard as a backstop for the rare debounce-gap case.
- [x] **Journal browser (`/journal`)** — sidebar with collapsible year/week tree (newest first, current year auto-expanded), raw-markdown editor on the right. Auto-saves on the same 1.5s debounce as `/summary` via the new `write_week` command. Switching weeks flushes any pending edits to the previously-selected week before loading the new one. Current week marked with an orange dot; selected week highlighted in maroon.
- [x] **Open and edit past Notes** — same `/journal` route. The textarea is the entire weekly file's raw markdown; edits write back via `write_week`. (Structured per-Note editing is a future polish item; raw markdown is the minimum viable.)
- [x] **macOS spell-check** — `spellcheck="true"` on every prose surface (capture title + body, all 4 summary textareas, journal editor); `spellcheck="false"` on name + path inputs to silence noise on proper nouns and filesystem paths.

**Success:** Captain's Log has replaced any other journaling system I was using. **Achieved.**

## Phase 2.5 — Editor upgrade ✅ (shipped 2026-06-25)

Slack/Typora-style live-preview editor on CodeMirror 6, markdown stays on disk byte-identical. Replaces the plain `<textarea>` on `/summary`, `/capture`, and `/journal` with one shared MarkdownEditor.

### Steps 1-4 + initial toolbar ✅ (2026-06-24)

- [x] **Step 1** — `MarkdownEditor.svelte` Svelte 5 wrapper on CM6, `/capture` body swapped (commit `fb40bda`). Byte-identical round-trip verified.
- [x] **Step 2** — Clickable Markdown links via `markdown-links.ts` ViewPlugin + Cmd-click → Tauri `openUrl()` for `[text](url)`, autolinks, and GFM bare URLs (commit `05201d8`).
- [x] **Step 3** — Native WebKit/NSSpellChecker on the CM contenteditable; ~400 LOC custom IPC + `SpellcheckTextarea` deleted (commit `cfb2ce3`).
- [x] **Step 4** — MarkdownEditor propagated to `/journal` + `/summary`'s four fields; `--md-*` CSS vars + `resize: vertical` + `id` forwarding for label-for accessibility. Net delete ~280 LOC (commit `b27b263`).
- [x] **Initial toolbar** — 10-button strip + Cmd+B / Cmd+I / Cmd+K / Cmd+E / Cmd+Shift+7 / Cmd+Shift+8 keymap; shared `markdown-formatting.ts` command module; new `Icon.svelte` (commit `6d60b58`).

### Architecture B — live-preview engine ✅ (2026-06-25)

- [x] **Inline marker hiding** — bold, italic, strike, inline code, links collapsed via `Decoration.replace` + atomic ranges. Inline code rendered as a pill chip (`display: inline-block` so parent strikethrough can't bleed through).
- [x] **Fenced code blocks** — `` ``` `` + Enter (or 3rd backtick keystroke) auto-expands a body block with cursor on the body line. Backspace at body start deletes an empty fence or exits upward; trailing blank line auto-inserted when flush against doc edge; cursor-skip filter blocks typing on the opening/closing fence lines (prevents CodeInfo / broken closer). Line decoration on all body lines, left accent stripe.
- [x] **Slack-style blockquote** — 3px accent left bar, italic + muted color, scoped with `:not(.cm-md-fenced-line)` so nested fenced code stays readable.
- [x] **Atomic ranges + RangeSetBuilder sort** — load-bearing fix: sort by `deco.startSide` (replace=499.999M, mark=500M, line=−200M). Hand-rolled rank caused RangeSetBuilder to silently throw and decorations to vanish.
- [x] **5-lens adversarial passes** — cursor-after-closing-wrap edge correction, italic-on-fenced-in-quote CSS fix, code-button-while-in-FencedCode strips the fence, `updateTick` explicit-assignment reactivity, nested-list mutual exclusion for active-state.

### Toolbar overhaul ✅

- [x] **Active-state detection** — `detectActiveFormats` walks syntax tree from selection.head; buttons get `.is-active` + `aria-pressed`. Edge correction skips wrap nodes when the cursor sits at their right/left boundary.
- [x] **Multi-line Cmd+E** — cursor lands at body end with trailing newline (was: stuck on hidden opening fence).
- [x] **C3 fix** — `transformLines` skips opening/closing fence lines so quote/bullet/numbered can't corrupt a fenced block.
- [x] **C4 fix** — heading cycle preserves H4-H6 via narrow strip + early-return.
- [x] **M2 fix** — link placeholder changed from `url` to `https://`.
- [x] **New buttons + shortcuts** — Task list (Cmd+Shift+L), Today's date (Cmd+;); plus strike (Cmd+Shift+X), quote (Cmd+Shift+9), heading (Cmd+Alt+0).
- [x] **Skeptic-round fixes** — H6 regression (regex strip-vs-prepend mismatch, H4-H6 guard added); empty-line task on blank doc (`addOnBlanks` branch + `sawNonBlank` tracking); Cmd+; for date vs Cmd+Shift+; for time (Google Sheets convention).

### Date chip + picker ✅ (Confluence-style)

- [x] **`date-chip.ts` ViewPlugin** — scans visible viewport for `\b\d{4}-\d{2}-\d{2}\b`, skips code spans, replaces with a WidgetType chip showing formatted date ("Jun 25" / "Jun 25, 2026"). Atomic range.
- [x] **`DatePickerPopover.svelte`** — hand-rolled month grid (~200 LOC), full keyboard nav (arrows day, PgUp/Dn month, Shift+PgUp/Dn year, Enter commits, Esc closes), outside-mousedown closes, Floating-UI-style position computation with bottom-flip-to-top + viewport clamp.
- [x] **Click routing** — dispatch directly on the owning MarkdownEditor's view (NOT via window event + activeViews Set — would misroute commits across /summary's 4 instances).
- [x] **Position-bake fix** — `WidgetType.eq()` includes from/to so DOM rebuilds on text shift (otherwise stale offsets in click handlers).
- [x] **Cursor-at-matchEnd allow** — newly-inserted date chip renders immediately; viewport-edge clamp keeps popover on-screen in `/capture`'s small window.

### /journal Preview/Source toggle ✅

- [x] Segmented control + Cmd+Shift+S + localStorage persistence (`captainslog:journalViewMode`). Default Preview, editable in both modes. `{#key viewMode}` forces editor remount (CM bakes extensions at construction).
- [x] Source mode keeps monospace + 14px; Preview uses body font + 16px + toolbar.
- [x] **Empty-state placeholder** — `/journal` no longer auto-selects current week on mount. Was a real data-mutation bug (typing into current week's file just by opening `/journal`).

### /summary polish ✅

- [x] livePreview on all four fields; field min-height unified to 112px; labels use Unicode horizontal ellipsis (…); placeholders dropped from `- ` to empty.

### List widgets (Day 8-10) ✅

- [x] **BulletWidget** — replaces `-` ListMark of BulletList items with `•` (muted, fixed-width). Numbered ListMarks left alone (digits are meaningful).
- [x] **TaskCheckboxWidget** — replaces 3-char TaskMarker with a clickable 16px square; toggles via direct `view.dispatch`. Sibling `cm-md-task-done` mark applies strikethrough + muted color to checked task body.
- [x] **Dynamic Tab cap** — `maxListIndentAllowed` walks backward to the would-be parent line, caps indent at `parent_content_offset + 3` (CommonMark sub-item range). Top-level lone items can't Tab.
- [x] **Lazy-continuation fix v2** — `applyListMarkerToCurrentLine` handles both empty-line and only-marker (`2. `) cases. When the line above is non-blank and different-family, prepends `\n` so Lezer parses as a fresh list instead of a lazy continuation or Setext underline. Verified empirically with `@lezer/markdown` + GFM.

### Layout chrome ✅

- [x] **Cat companion** — upper-left, clickable (opens random YouTube cat search via `tauri-plugin-opener`), "Meow!" tooltip, hidden on `/journal` (browser overlap).
- [x] **Help + Nerds Only popups** — moved to lower-LEFT (so scrollbar appearance doesn't shift them). Two pill-shaped 11px buttons. Backdrop dismiss + Escape + close button; focus restored on close. Bodies cover three surfaces, keyboard shortcuts grouped by category, menu-bar capture, Noot description, and a Nerds Only stack (Tauri / SvelteKit / Svelte 5 runes / CM6 / Lezer + GFM / CommonMark / live-preview model / repo link).

### File audit + cleanup ✅

- [x] 20 existing weekly files audited via parallel workflow: 3 clean, 16 en-dash drift in `# Week of` titles (bulk-fixed to hyphens), 1 broken (W26 test/scratch — deleted). 8 throwaway 2024/2025 test files generated for multi-year sidebar verification, then deleted.

### Architecture documentation ✅

- [x] `ARCHITECTURE.md` (~4000 words): overview, three surfaces, storage model, live-preview architecture, toolbar + commands, multi-instance routing lesson, known limitations.

### Deferred from Phase 2.5 (rolled into Phase 2.7 or later)

- [ ] **HTML email body via `pulldown-cmark`** — original Step 5. Send flow already works in plaintext via 2.6; HTML upgrade is non-blocking.
- [ ] **Preview modal on `/summary`** — original Step 6. Same renderer as the send path, iframe srcdoc for isolation.

## Phase 2.5b — Editor follow-ups ✅ (shipped 2026-06-25, evening)

The two known follow-ups from 2.5's hand-off:

### Cursor preservation across Preview/Source toggle ✅

- [x] **Compartment-based extension swap** — `MarkdownEditor.svelte` wraps the `livePreview` extension in a per-instance CodeMirror 6 `Compartment`. A `$effect` watching the prop dispatches `view.dispatch({effects: livePreviewCompartment.reconfigure(...)})` on change. The EditorView stays mounted across mode flips; cursor position, selection, scroll position, and undo history all survive.
- [x] **/journal removed `{#key viewMode}`** — earlier shape forced full remount on every toggle.

### Cross-route file invalidation ✅

- [x] **Rust event broadcast** — `write_week`, `update_weekly_summary`, and `create_note` now emit a `weekly-file-changed` event with `{ year, week }` payload after a successful write (helper `emit_weekly_file_changed` in `commands.rs`).
- [x] **/journal + /summary listeners** — both routes subscribe via `listen('weekly-file-changed', ...)` and call `reconcileWithDisk` when the event matches the selected/current week. Clean-form-clean-disk → silent no-op (covers the /capture-appends-a-note path so /journal updates without user action). Clean form, disk differs → silent reload. Dirty form, disk differs → `externalUpdate` banner with "Reload (lose my edits)" + dismiss buttons. Never silently destroys edits.
- [x] **Own-save race suppression (pendingCommit)** — Tauri's invoke-response and event emit travel separate IPC paths; a pure `saveStatus === 'saving'` gate is racy. Each route now tracks a `pendingCommit` slot (the bytes/signature the in-flight save is writing) set before invoke and cleared in the success path. `reconcileWithDisk` no-ops when disk matches the post-baseline state OR the pre-baseline `pendingCommit` — robust to either ordering.
- [x] **Concurrent-saveNow suppression** — the typing `$effect` no longer downgrades `saveStatus` from `'saving'` to `'dirty'`; the `saveNow` gate now reschedules the autoSaveTimer instead of dropping the save. Together this prevents two saveNow calls overlapping (which would clobber the single `pendingCommit` slot).
- [x] **Normalization-aware compare (/summary only)** — Rust trims field bodies and strips `#` prefixes from labels on read. The frontend stores pre-normalize values in `snapshot`/`pendingCommit` (so `isDirty` correctly compares to what the user typed); `reconcileWithDisk` runs `normalizedSig` on both sides before comparing, so a normalization-only delta is treated as "no real change" — no spurious banner, no silent field rewrite on every own-save echo.
- [x] **Post-baseline hash refresh held inside `saveStatus = 'saving'`** — /summary's `get_summary_hash` await happens before the status flips to `'saved'`, so the gate stays armed for the full critical section. Earlier shape let a rescheduled saveNow slip through during the hash refresh and clobber `pendingCommit`.
- [x] **/summary onDestroy clears autoSaveTimer** — mirrors /journal's pattern, so navigating away mid-debounce (e.g. clicking Done while dirty) can't fire saveNow on a destroyed component.

### Known limitations

Moved to the global [Deferred / TBD](#deferred--tbd) list below.

## Phase 2.6 — Send weekly summary to manager ✅ (shipped 2026-06-24)

One-click handoff to the OS-default mail handler. No SMTP credentials, no OAuth — the user reviews and sends the draft from their real mail identity, so threading and the Sent folder work normally. Commit `86d804b`.

- [x] **Manager email + manager name fields in Settings** — `managerEmail` + `managerName` on `JournalSettings`, persisted to `.metadata/settings.json`. Greeting personalizes when name is set; falls back to plain "Hello,".
- [x] **Sent-log sidecar** — `.metadata/sent-log.json` keyed by ISO year-week: `{ sentAt, contentHash, sentTo }`. One entry per week (overwrite on resend).
- [x] **`hash_weekly_summary` helper** — SHA-256 of the canonicalized four summary fields + labels, length-prefixed per field so multi-line content can't collide across section boundaries.
- [x] **`compose_weekly_email` command** — builds a `mailto:` URL by default; falls back to an `.eml` file in `$TEMP/captainslog/` when the percent-encoded URL would exceed ~1800 bytes. RFC 2047 encodes the `.eml` subject so the en-dash week label doesn't trip strict parsers.
- [x] **Email body** — opens with `Hello {managerName},` (or `Hello,`) + an intro line that links to the public Captain's Log repo. Sections (key accomplishments / plans / challenges / anything else) follow as `##` markdown headings; empty sections are dropped. Labels render as `Labels: #tag1, #tag2` at the bottom.
- [x] **Subject branching** — `Weekly update - week of …` on first send, `Update to weekly update - week of …` on resend (detected by existing sent-log entry at compose time).
- [x] **`get_sent_record` + `mark_weekly_summary_sent` + `get_summary_hash` commands** — gate the Send button and stamp the sent-log entry after a successful handoff.
- [x] **Capability scope** — `opener:allow-open-url` accepts `mailto:*`; `opener:allow-open-path` scoped to `$TEMP/captainslog/**`.
- [x] **Send button on `/summary`** — next to Save. Gated on: manager email set, no unsaved edits, not already sent with matching content hash. Tooltip explains the disabled reason.
- [x] **Confirmation modal** — previews the addressee + week label before opening the draft; Escape and backdrop click dismiss.
- [x] **Sent state UI** — `Sent {when}` when the button is locked by a matching record; `Send updated version` with a stale-state indicator when the content hash drifts after a save.
- [x] **`.eml` temp janitor** — startup task prunes `$TEMP/captainslog/*.eml` files older than 24h.
- [x] **Backend ⇄ frontend label parity** — `format_week_label` matches the frontend's `weekLabel` exactly (full month names + en-dash), so the modal and the email subject read the same string.

## Phase 2.7 — Onboarding + Settings revisit

The first-run wizard captures the bare minimum today (name, journal location, reminder); after 2.6 the data model grew enough — and the Settings screen is long enough — that both deserve a polish pass.

- [ ] **"Tell me about you" wizard step** — name, Bamboo title (with the word *Bamboo* linking to Prodigy's BambooHR site), Jira project keys (comma-separated, e.g. `MAGE`, multiple allowed).
- [ ] **"Tell me about your manager" wizard step** — manager name + email. Both fields reuse the columns added in 2.6.
- [ ] **Settings layout** — convert the single long-scroll form into tabs or section breaks (Your details / Manager / Journal location / Reminder / Theme). Adding 3+ new fields without grouping is the trigger.
- [ ] **Persistence** — Bamboo title + Jira project keys join `JournalSettings`; same `.metadata/settings.json`.

## Phase 3 — Search & Navigation

- [ ] Full-text search across all weekly files
- [ ] Filter by label, date range, file
- [ ] Click search result → opens correct week, scrolls to correct Note
- [ ] Year/week tree handles many years gracefully

## Phase 4 — Link Enrichment + Label Library

- [ ] Detect URLs in Notes (Jira, GitHub, Slack, Confluence to start)
- [ ] Fetch metadata via MCP connectors
- [ ] Store enriched metadata inline or in a `.metadata/links/` cache
- [ ] Display enriched cards in the rendered view (status, title, last update)
- [ ] Room to grow to other systems beyond the initial four
- [ ] **Label library viewer + bulk management** — browse + filter all labels in use across the journal. Sits on top of the existing `.metadata/labels.json` index (already maintained by `record_note_labels`). UX TBD; will need at minimum a filter input (substring + maybe tag-cloud-by-recency) and a way to drill from a label into the matching Notes/Summaries. Depends on Phase 3 search so labels can reuse the same result-list + week-jump plumbing. **Bulk rename/merge/delete** is a natural extension of the same screen — do both at once if it shapes up as one cohesive workflow.

## Phase 5 — Performance Review Module

The reason this app exists.

- [ ] Date range picker (calendar UI)
- [ ] "Bundle this range as a single markdown file" export
- [ ] Bundle includes Notes + Summaries + enriched link metadata
- [ ] Bundle prepends a configurable instruction block for the LLM
- [ ] Configurable review-question templates (e.g. the 8-question Prodigy mid-year template)
- [ ] One-click "draft my review" flow that produces editable first-draft answers

## Phase 6 — Sync & Sharing

- [ ] Google Drive sync option (everyone at Prodigy has a Google account)
- [ ] Conflict resolution for multi-device edits
- [ ] At-rest encryption (so synced files aren't readable without the app)

## Phase 7 — Cross-Platform

- [ ] Validated Windows build
- [ ] Validated Linux build
- [ ] CI for all three platforms

---

## Deferred / TBD

- [ ] **Editor edge cases from Phase 2.5** (tracked, not blockers):
  - Cmd+Home / Cmd+End / Cmd+F landing on a fence line + arrowing breaks the cursor-skip filter assumption (mitigated today by `lineDelta > 1` guard).
  - IME on body-line-start backspace edge case.
  - Multi-cursor + most widget commands bail rather than handle each range.
  - Setext headings not detected by active-state.
- [ ] **Cross-route invalidation edge cases from Phase 2.5b**:
  - `/journal` reschedule loop is bounded only by save settling — if `invoke('write_week')` gets genuinely stuck, the autoSaveTimer reschedule loop spins at 1.5s intervals. Cheap but not zero. Acceptable for local-SSD writes (< 100ms typical).
  - External-writer-during-own-save race — if an external writer modifies the file while our save's invoke is in flight, the listener may either silently adopt the external content (clean form) or be overwritten by our save completing (we wrote last). Inherent two-writer race; the event mechanism only enables refresh, not coordination.
- [ ] **Higher-resolution petbook source** — current app icon is upscaled from a 96×96 source PNG. Larger sizes (256/512/1024) are softer than they could be. Replace `src-tauri/icons/source-petbook.png` and re-run `npx @tauri-apps/cli icon …` if a higher-res asset surfaces.
- [ ] **Spacing, motion, and component library finalization** — colors, typography, iconography, and core component patterns are locked in [STYLE-GUIDE.md](STYLE-GUIDE.md). Still TBD: final spacing scale tokens, animation/transition spec, complete reusable component spec library. No specific phase — bolt on whenever a new screen forces the question.
- [ ] **Plugin / extension API** — let other tools read/write Captain's Log data.
- [ ] **iOS/Android companion app** — flagged but probably not worth doing soon.
- [ ] **Multi-user / team features** — flagged but likely never.
