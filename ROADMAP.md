# Captain's Log — Roadmap

## Current phase: 2.5 — editor upgrade (active, mid-pivot)

Phase 1 MVP and Phase 2 polish are complete. Phase 2.6 ("Send weekly summary to manager") shipped on 2026-06-24. Phase 2.5 Steps 1-4 + the formatting toolbar shipped today (CodeMirror 6 source-mode editor on all three surfaces, native WebKit spell-check, 10-button formatting toolbar with Cmd+B/I/K/E + Cmd+Shift+7/8 shortcuts, clickable Markdown links).

**Active pivot** (decided 2026-06-24): the source-mode editor — visible `**`, `~~`, `#`, `-`, `>` markers in the user's text — is a UX blocker for non-technical adopters (HR, artists, accountants, PMs) coming within 60 days. Pivoting from source mode to **aggressive Slack/Typora-style marker hiding** on the same CodeMirror 6 engine. Storage stays markdown on disk (5 of 6 evaluation lenses converged on that — power-user workflow, LLM bundle, Phase 5/6/7 portability, long-term maintenance, implementation realism). Display becomes true rich-text with markdown hidden as decorations.

**Rollback baseline tagged at `pre-slack-wysiwyg`** (commit `ac101c8`) — clean restore point before the Architecture B build begins.

**Phase 2.7 (onboarding + Settings revisit)** stays queued after Phase 2.5 completes.

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

## Phase 2.5 — Editor upgrade (active, mid-pivot)

Replaces the plain `<textarea>` on `/summary`, the capture popup body, and `/journal` with a real Markdown editor; switches the Send-to-manager email body to HTML so recipients see real formatting and clickable links. **Disk format stays raw markdown** (decision locked across power-user workflow / LLM bundle / Phase 5/6/7 portability / long-term maintenance lenses).

### Steps 1-4 + toolbar ✅ (shipped 2026-06-24)

- [x] **Step 1 — `MarkdownEditor.svelte` + `/capture` swap** (commit `fb40bda`). Added CodeMirror 6 deps (`@codemirror/{state,view,commands,language,lang-markdown}` + `@lezer/markdown`). Hand-rolled ~40-line Svelte 5 wrapper with one-way `value` prop + `onChange` callback. Body of `/capture` swapped from textarea to MarkdownEditor. Markdown round-trip verified byte-for-byte via `xxd` on the draft file.
- [x] **Step 2 — Clickable Markdown links** (commit `05201d8`). New `markdown-links.ts` CM6 ViewPlugin that walks the Lezer syntax tree, applies `domEventHandlers.mousedown` to `Link` / `Autolink` / `URL` nodes with Cmd-click handlers calling Tauri's `openUrl()`. Capability scope extended to allow `http://*` and `https://*`. Three link forms work end-to-end: `[text](url)`, `<https://example.com>`, and GFM bare URLs.
- [x] **Step 3 — Native WebKit spell-check on contenteditable** (commit `cfb2ce3`). Investigation arc surfaced that `tauri-apps/tauri#7705` (the textarea spell-check bug we worked around with SpellcheckTextarea) does NOT apply to contenteditable surfaces. CodeMirror's editing surface IS a contenteditable div. Setting `EditorView.contentAttributes.of({ spellcheck: 'true' })` routes the entire pipeline through WebKit + NSSpellChecker natively — same engine Apple Mail and Pages use. Squiggles paint natively, right-click suggestions pre-populated, contractions (`dont` → `don't`) caught the way users expect. Custom IPC + Decoration.mark plugin + `SpellcheckTextarea.svelte` all deleted (~400 LOC net delete).
- [x] **Step 4 — Propagate MarkdownEditor to `/journal` (monospace) + `/summary` (four instances)** (commit `b27b263`). Monospace, 14px font, 1.5 line-height, 16px padding wired via `--md-*` CSS variables on the new MarkdownEditor invocation. `/summary` fields use `--md-min-height` to approximate prior `rows={3|4|5}` initial heights + `resize: vertical` for user-drag-grow affordance. `id` prop forwarded to `.cm-content` for `<label for={id}>` accessibility. `SpellcheckTextarea.svelte`, `spellcheck.rs`, the `check_spelling` Tauri command, and the `objc2-app-kit` NSSpellChecker feature all retired. Net delete: ~280 lines.
- [x] **Toolbar + journal cheat sheet** (commit `6d60b58`). 10-button formatting strip above each MarkdownEditor on `/capture` and `/summary` (Heading cycle / Bold / Italic / Strikethrough / Bulleted list / Numbered list / Block quote / Link / Code / Help). Keyboard shortcuts: Cmd+B / Cmd+I / Cmd+K / Cmd+E / Cmd+Shift+7 / Cmd+Shift+8. Shared command module (`markdown-formatting.ts`) backs both toolbar onClicks and the keymap so wrap/unwrap logic lives in one place per format. New `Icon.svelte` with 10 Lucide-derived inline SVGs (no icon library dep). New `showToolbar?: boolean = true` prop on MarkdownEditor; `/journal` opts out and adds an inline cheat-sheet link to its placeholder copy.

### Step 5+ — Architecture B pivot (active, ~10-14 days)

**Rollback line: `pre-slack-wysiwyg` (commit `ac101c8`)** — clean restore point.

Source-mode editor (`**bold**` markers visible, faded) is a UX blocker for non-technical users coming within 60 days. Pivoting from source mode to **aggressive Slack/Typora-style marker hiding** while keeping CodeMirror 6 + markdown-on-disk. Markers (`**`, `*`, `_`, `~~`, `` ` ``, `#`, `-`, `1.`, `>`, `[`/`](url)`) become atomic-hidden ranges via `Decoration.replace`; user sees rendered rich text only, types markdown shortcuts but never sees the syntax in the result. Storage axis stays locked at markdown (5 of 6 evaluation lenses converged here — power-user workflow, LLM bundle, Phase 5/6/7, long-term maintenance, implementation realism); display axis goes all-the-way Slack, not partway Obsidian-style.

Implementation order:

- [ ] **Day 1-3** — Build the aggressive-hiding ViewPlugin in `MarkdownEditor.svelte`. Decoration.replace + atomic ranges hide all marker tokens. Selection-watcher governs the brief "cursor on the closing marker that completes a pair" reveal edge case. Ship to `/capture` only first (smallest blast radius). Toolbar + keymap stay visible and working.
- [ ] **Day 4** — Propagate to `/summary`'s four fields. Manual QA matrix (paste from Slack, paste from VS Code, undo/redo across atomic ranges, backspace at marker boundaries, list creation/deletion, link insertion via toolbar).
- [ ] **Day 5-7** — `/journal` rich-text redesign. Same Live Preview editor for past weeks, full rich-text editing parity with `/summary` (Chris confirmed daily past-week editing is part of his workflow). Per-view "reveal source" toggle (NOT global — global creates a mode users toggle by accident and never recover from). Wire to existing auto-save + `dirty.ts`.
- [ ] **Day 8-10** — Edge-case hardening. Atomic-range escape on selection-drag, backspace at marker boundaries, line-level constructs (headings need `Decoration.line` for the style + `Decoration.replace` for the `# ` marker). Code fences, strikethrough, blockquote consistency. Verify `markdown-links.ts` Cmd-click still works.
- [ ] **Day 11-12** — Polish + architecture note in `docs/`. Update CLAUDE.md. Document the storage axis (markdown, locked) vs display axis (aggressive hiding now, true-WYSIWYG-via-TipTap as a future graduation if needed).
- [ ] **Day 13-14** — Buffer + final QA + hand-off. Use it for at least a week before declaring Phase 2.5 done.

### Deferred (post-pivot, still part of Phase 2.5)

- [ ] **Step 5 (post-pivot) — HTML email via `pulldown-cmark`.** Add the crate. Rename `render_body` to `render_body_plaintext` (debug/LLM view). New `render_body_html` runs each section through `pulldown_cmark::html::push_html` and wraps in a minimal inline-CSS shell. `write_eml_file` flips `Content-Type` to `text/html`, quoted-printable encodes the body, adds `X-Unsent: 1`. Delete `MAILTO_MAX_BYTES` + the mailto branch in `compose_weekly_email`.
- [ ] **Step 6 (post-pivot) — Preview modal on `/summary`.** New `EmailPreview.svelte`, calls a new `render_email_preview` command (DRY — same renderer as the send path). HTML rendered inside an `iframe srcdoc` for style isolation. "Preview" button next to "Send to manager".
- [ ] **Step 7 (post-pivot) — Final smoke + commit.** Smoke matrix: markdown byte-identity on `/capture`, week-switch flush on `/journal`, all four sections + Preview + Send on `/summary`, real email to a Gmail + Outlook account.

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
