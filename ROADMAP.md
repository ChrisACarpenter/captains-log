# Captain's Log — Roadmap

## Current phase: 2.5 — editor upgrade (next)

Phase 1 MVP and Phase 2 polish are complete. Phase 2.6 ("Send weekly summary to manager") shipped on 2026-06-24 — the app now does a one-click handoff to the OS-default mail handler with per-week sent-state tracking, personalized greeting, and a content-hash-based "edited since send" gate. **Phase 2.5 (CodeMirror 6 markdown editor) is next**, then **Phase 2.7 (onboarding + Settings revisit)** before Phase 3 — both queued in order below.

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

## Phase 2 — Polish: "Can I actually use this daily?"

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

## Phase 2.5 — Editor upgrade (next)

Replaces the plain `<textarea>` on `/summary`, the capture popup body, and `/journal` with a real Markdown editor; switches the Send-to-manager email body to HTML so recipients see real formatting and clickable links. Disk format stays raw markdown (LLM handoff in Phase 5 requires this).

**Design decisions** (locked 2026-06-24):

- **Editor library: CodeMirror 6** in a hand-rolled ~30-line Svelte 5 wrapper. Wins on zero round-trip drift — the buffer IS the markdown file byte-for-byte. WYSIWYG alternatives (Milkdown / TipTap / Lexical) all mutate the source on save.
- **Editor mode: source-with-syntax-highlighting** for v1 (`**bold**` markers visible, faded). Live-preview decorations (Obsidian-style cursor-outside-collapse) deferred to a follow-up after a week of real use.
- **Email format: HTML-only `.eml`** rendered server-side via `pulldown-cmark`. `mailto:` branch deleted (RFC 6068 §5 — mailto bodies are text/plain only). All sends go through `.eml`; the existing length-based fallback split disappears. Compose pane now shows the rendered email — `<a href>` tags, real bold, styled headings.
- **Send-side mitigation: in-app Preview modal** on `/summary` using the same `pulldown-cmark` renderer, so the user can audit the rendered form before the Mail.app round-trip. Replaces the Phase 2.6 raw-bytes audit affordance at a different layer.
- **Spell-check: full Decoration.mark rewrite**. Retires `SpellcheckTextarea.svelte` entirely once CM6 lands on all three surfaces; squiggles become a real CodeMirror extension. Forward-compatible with eventual live-preview work even though deferred. Existing `check_spelling` Rust command stays unchanged.

**Implementation order** (each step is independently testable):

- [ ] **Step 1 — `MarkdownEditor.svelte` + `/capture` swap.** Add `@codemirror/{state,view,commands,language,lang-markdown}` + `@lezer/markdown`. Hand-rolled wrapper with one-way `value` prop + `onChange` callback (not `$bindable` — CM6 transactions own the doc). Replace the body `SpellcheckTextarea` on `/capture`. Prove the markdown round-trip with `xxd` on the draft file before propagating.
- [ ] **Step 2 — Clickable links in the editor.** New `markdown-links.ts` CM6 ViewPlugin that walks the Lezer syntax tree, applies `Decoration.mark` to `Link` and `URL` nodes with Cmd-click handlers calling Tauri's `shell.open()`. Regex viewport scan for bare URLs.
- [ ] **Step 3 — Spell-check Decoration.mark plugin.** New `spellcheck-plugin.ts` CM6 extension. Fires `check_spelling` on a 400ms debounce, maps returned `{start, length}` ranges to `Decoration.mark` with `text-decoration: underline wavy red`. Wire alongside `MarkdownEditor` on `/capture`. Validates that the new design carries the existing UX.
- [ ] **Step 4 — Propagate to `/journal` (monospace) and `/summary` (four instances).** Keep monospace via `style` passthrough on `/journal`. Four independent EditorView mounts on `/summary` are fine — CM6 shares state + view as module-level singletons.
- [ ] **Step 5 — HTML email via `pulldown-cmark`.** Add the crate. Rename `render_body` to `render_body_plaintext` (debug/LLM view). New `render_body_html` runs each section through `pulldown_cmark::html::push_html` and wraps in a minimal inline-CSS shell. `write_eml_file` flips `Content-Type` to `text/html`, quoted-printable encodes the body, adds `X-Unsent: 1`. Delete `MAILTO_MAX_BYTES` + the mailto branch in `compose_weekly_email`.
- [ ] **Step 6 — Preview modal on `/summary`.** New `EmailPreview.svelte`, calls a new `render_email_preview` command (DRY — same renderer as the send path). HTML rendered inside an `iframe srcdoc` for style isolation. "Preview" button next to "Send to manager".
- [ ] **Step 7 — Retire `SpellcheckTextarea`, smoke-test, commit.** Delete the old component once CM6 covers all three surfaces. Smoke matrix: markdown byte-identity on `/capture`, week-switch flush on `/journal`, all four sections + Preview + Send on `/summary`, real email to a Gmail + Outlook account.

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

## Phase 2.7 — Onboarding + Settings revisit (after Phase 2.5)

Sequenced after Phase 2.5 so the editor refactor doesn't trip over onboarding state mid-flight. The first-run wizard captures the bare minimum today (name, journal location, reminder); after 2.6 the data model grows enough — and the Settings screen is long enough — that both deserve a polish pass.

- [ ] **"Tell me about you" wizard step** — name, Bamboo title (with the word *Bamboo* linking to Prodigy's BambooHR site), Jira project keys (comma-separated, e.g. `MAGE`, multiple allowed).
- [ ] **"Tell me about your manager" wizard step** — manager name + email. Both fields reuse the columns added in 2.6.
- [ ] **Settings layout** — convert the single long-scroll form into tabs or section breaks (Your details / Manager / Journal location / Reminder / Theme). Adding 3+ new fields without grouping is the trigger.
- [ ] **Persistence** — Bamboo title + Jira project keys join `JournalSettings`; same `.metadata/settings.json`.

## Phase 3 — Search & Navigation

- [ ] Full-text search across all weekly files
- [ ] Filter by label, date range, file
- [ ] Click search result → opens correct week, scrolls to correct Note
- [ ] Year/week tree handles many years gracefully

## Phase 4 — Link Enrichment

- [ ] Detect URLs in Notes (Jira, GitHub, Slack, Confluence to start)
- [ ] Fetch metadata via MCP connectors
- [ ] Store enriched metadata inline or in a `.metadata/links/` cache
- [ ] Display enriched cards in the rendered view (status, title, last update)
- [ ] Room to grow to other systems beyond the initial four

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

- [ ] **Higher-resolution petbook source** — current app icon is upscaled from a 96×96 source PNG. Larger sizes (256/512/1024) are softer than they could be. Replace `src-tauri/icons/source-petbook.png` and re-run `npx @tauri-apps/cli icon …` if a higher-res asset surfaces.
- [ ] **Spacing, motion, and component library finalization** — colors, typography, iconography, and core component patterns are locked in [STYLE-GUIDE.md](STYLE-GUIDE.md). Still TBD: final spacing scale tokens, animation/transition spec, complete reusable component spec library. Address as we build screens in Phase 2.
- [ ] **Bulk label management UI** — rename/merge/delete labels across all files. Phase 2 if it becomes a pain point; later if not.
- [ ] **Plugin / extension API** — let other tools read/write Captain's Log data.
- [ ] **iOS/Android companion app** — flagged but probably not worth doing soon.
- [ ] **Multi-user / team features** — flagged but likely never.
