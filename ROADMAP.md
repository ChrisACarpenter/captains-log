# Captain's Log — Roadmap

## Current phase: Phase 4 ✅ done — link chips shipped, next up Phase 5 (Performance Review Module)

Phase 1 MVP and Phase 2 polish are complete. Phase 2.6 ("Send weekly summary to manager") shipped 2026-06-24. Phase 2.5 (editor upgrade, Architecture B live-preview) shipped 2026-06-25 — Slack/Typora-style marker hiding on CodeMirror 6 with markdown-on-disk; live-preview engine, widgets (date chip + picker, bullets, task checkboxes), toolbar overhaul, /journal Preview/Source toggle, layout chrome polish, and an architecture doc all landed in a single session. Phase 2.7 (onboarding wizard expansion + Settings tabbed redesign + multi-day reminders) shipped 2026-06-26, plus a cross-app UX polish pass (Phase 2.7b): dark-theme contrast audit + 30+ fixes, button/UX standardization, shared component extractions, and a scrollbar-gutter fix. Phase 2.9 (HTML email body + Preview modal) landed 2026-06-26 but was dark-released — Phase 2.9b (2026-06-29) finished the job by adding a Mail tab to Settings, three send modes (Gmail default, Native Mac Mail, Outlook), a universal Preview modal with clipboard, a week-rollover fix, and a sleep-drift fix on the reminder scheduler. Phase 2.9c (2026-06-29) layered on the "Compose + paste" body-delivery mode (open empty compose + write rich HTML to clipboard = 2-click formatted send across all clients), restructured the Mail tab around a single "How should Send work?" section, and burned down a stack of editor-rendering bugs around lists, numbered-marker contrast, and task-item double-markers.

Phase 2.8 (Custom Themes) shipped 2026-06-30 — 12 user-editable primaries → ~23 OKLCH-derived tokens via culori, Theme = Light / Dark / Custom, AA contrast warnings, hex-input editor, `.captheme.json` export/import via Tauri dialogs, plus a "Colorful Labels" follow-on that gives each label a per-name hue (theme-aware, regenerates on switch — no lazy-persist, so no theme-burn). A tray-menu "Preset Theme" submenu (Dark / Light) is the escape hatch for when a Custom palette makes the in-app theme picker unreadable. Phase 2.8c (2026-06-30) layered a shared-component pass on top — `Modal`, `ConfirmDialog`, `LoadingOverlay`, `PointerFinger`, `StepHeader`, `PathPickerField` extracted out of onboarding / settings / send-to-manager — refactored the SendToManagerButton Preview popup onto the shared Modal (From: line gated on `user_email`, HTML render now shown in Compose+paste mode too, Close + Copy buttons placed side-by-side at the lower-right), and fixed a Gmail + Compose+paste clipboard-skip bug where a stale `mailNativeHtml` flag short-circuited the `writeHtml` call.

Phase 3a shipped 2026-07-06 — the Label Library viewer got its "Referenced In" drill-down (new `get_notes_for_label` Rust command + a bounded list inside `LabelDetailsModal` capped at 50 rows, click-to-navigate into `/journal?year=Y&week=W`), plus multi-select on the Labels tab with a bulk toolbar (Delete N / Merge into…). The merge picker is a radio-select Modal that pre-fills the highest-count label as the canonical target and reuses `rename_label`'s auto-merge-on-collision. Bulk-rename was dropped from the original scope — rename-into-existing already covers the merge case, so a distinct bulk-rename mode was redundant.

Phase 3b shipped 2026-07-06 — full-text search across every weekly file (Weekly Summary content + Note bodies) with an optional label filter, dedicated `/search` route reachable from the `/journal` sidebar OR global `Cmd+K` shortcut, result cards grouped by surface (Summary/Note kind badge + `YYYY-Wnn` label + Note timestamp + labels), click-to-jump into `/journal?year=Y&week=W&scrollTo=<byte-offset>` with MarkdownEditor scrolling the target byte into view. MVP started narrower (Summary-only) and expanded to Notes + scroll-to-position once the pattern proved out.

**Phase 3c (task list aggregator), Phase 3d (task rearchitecture, Slices 6a–6c + auto-import), and Phase 3e (task due dates + reminders) are done.** The aggregator shipped as designed, then the whole task feature was rearchitected around a dedicated `### Tasks` section with HTML-comment anchors — tasks became first-class objects with a locked-down UI while markdown stayed the storage layer. Row actions (pencil edit + trash delete), Copy-Completed-to-Key-Accomplishments, and once-per-day auto-import all landed on top of 3d. Phase 3e then added optional due dates (calendar action, DatePickerPopover reuse, red-chip Overdue heading, sort-earliest-first, rollover carries the debt forward) and OS-notification task reminders wired to a dedicated tokio scheduler parallel to the journal reminder, all with a Noot icon. See phase sections below for the full receipts.

**Phase 4 (link chips) is done.** Shipped 2026-07-15 with a different design than the original brief: no MCP connectors, no curated per-service list, no `.metadata/links/` card store. Instead, every markdown link in the editor renders as an inline pill chip (favicon + label from the `[text]`) via a CodeMirror widget layer, backed by a generic HTML-head scraper (`og:title` / `<title>` / `og:site_name` / `<link rel="icon">`) with a `.metadata/link-cache.json` sidecar. Auth-gated URLs degrade to a hostname/globe fallback chip. Storage stays plain markdown — the chip is a rendering concern, not a data model change. See Phase 4 receipt below.

**Next up: Phase 5 — Performance Review Module.** The reason this app exists: date-range bundling of Notes + Summaries into an editable review-first-draft.

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

### Phase 2.9 — HTML email body + Preview modal ✅ (shipped 2026-06-26, re-enabled via 2.9b)

The HTML renderer (pulldown-cmark + ammonia), multipart `.eml` writer, Preview modal, request-token lifecycle hardening. Initial release was dark — Apple Mail opens `.eml` read-only, regressing the 1-click flow. Re-enabled in 2.9b through the Native Mac Mail mode's HTML toggle and a universal Preview modal that works across all three send modes.

### Phase 2.9b — Mail tab in Settings, user picks send mode ✅ (shipped 2026-06-29)

Shipped as 9 slices in a single session. Mail tab in Settings + three first-class send modes + a universal Preview + two scheduler/rollover fixes that surfaced during the work.

- [x] **`JournalSettings.user_email` field** — pins Gmail's `/mail/u/{address}` slot for multi-account routing and feeds AppleScript's `sender` so Native Mac Mail sends from the right account.
- [x] **Settings → Mail tab** — radio group for send mode (Gmail / Native Mac Mail / Outlook), body-format toggle (Clean text / Markdown source), Native-only HTML toggle, Outlook flavor (Business / Personal). `serde(default)` on every field so older settings.json files load cleanly with Gmail defaults — no migration code.
- [x] **Gmail compose dispatch** — URL template `https://mail.google.com/mail/u/{ACCOUNT}/?view=cm&tf=cm&to={TO}&su={SUBJECT}&body={BODY}`. `ACCOUNT = user_email` when set, else `0`. All params URL-encoded with `NON_ALPHANUMERIC`. Warn-and-allow modal when encoded URL > 2000 chars (Gmail silently truncates above that).
- [x] **Outlook compose dispatch** — Business host `outlook.office.com/mail/deeplink/compose`, Personal host `outlook.live.com/mail/0/deeplink/compose`. Subject param is `subject` (distinct from Gmail's `su`).
- [x] **Native Mac Mail via AppleScript** — spawn `osascript -` and pipe the script via stdin (sidesteps argv length cap). `sender` set when `user_email` configured. Permission-denied detection on `-1743` / `Not authorised` in stderr surfaces an "Open Automation Settings" link to `x-apple.systempreferences:com.apple.preference.security?Privacy_Automation`. Escapes backslashes and double quotes in substituted values.
- [x] **Send-button rewire** — single dispatch point on `/summary` + `/journal` routes through `compose_weekly_email` then branches on `mail_send_mode`. Sent-log mark-as-sent fires only on confirmed dispatch.
- [x] **Universal Preview modal** — always available regardless of mode. Native HTML mode shows the rich render in a sandboxed iframe; Gmail / Outlook / Native-plaintext modes show plaintext in a `<pre>`. Heads-up tip on the modal points at Automation Settings when Native Mac Mail is the chosen mode (style matches Reminders-tab notification-permission tip exactly).
- [x] **Clipboard button on Preview** — `tauri-plugin-clipboard-manager` `writeHtml` (HTML + plaintext fallback) in Native HTML mode, `writeText` in plaintext modes. Inline confirmation in the modal.
- [x] **Week-rollover fix** — `/capture` and the reminder scheduler now resolve the current ISO week at the moment of write, not at app boot. Previously, after a weekend system-sleep, the first Monday capture would write into the prior week's file. Backed by new round-trip tests around ISO-week boundaries.
- [x] **Reminder sleep-drift fix** — scheduler loop wakes from `tokio::time::sleep_until` recomputes "now" from `chrono::Local::now()` and re-derives the next-fire instant rather than trusting the elapsed timer. macOS hibernation no longer leaves the next fire stuck in the past.
- [x] **Gmail as default for new installs** — `MailSendMode::default() = Gmail`. Doesn't need Automation permission, works on any machine where the user already has a Gmail tab.

**Out of scope (intentional):**
- AppleScript + HTML via UI-scripting / RTF conversion. Known fragile; not worth the maintenance tax.
- Cross-platform send paths (Windows / Linux). macOS-only while CaptainsLog stays on macOS.
- Custom signatures, BCC, scheduling, multi-recipient.
- Migration code for pre-2.9b settings.json. No users yet; `serde(default)` handles new installs cleanly.

### Phase 2.9c — Compose + paste mode + Settings restructure + editor polish ✅ (shipped 2026-06-29)

Layered on top of 2.9b in the same session. Started as a brainstorm about how to get formatted Gmail sends without losing 1-click ergonomics; grew to absorb a Settings UI restructure and a long tail of editor-rendering bugs that surfaced during smoke testing.

**Compose + paste body-delivery mode**

- [x] **Global `MailBodyDelivery` setting** — new enum (`Prefilled` / `ClipboardPaste`, default `Prefilled`). Orthogonal to send mode: the user picks once and it applies to all three clients (Gmail, Native Mac Mail, Outlook).
- [x] **Send dispatch honors the mode** — `MailSend.body_in_clipboard` on the Rust struct. When set, Gmail/Outlook URL builders emit empty `body=` and AppleScript emits `content:""`. Truncation warning is auto-suppressed (an empty body can't overflow). Native Mac HTML `.eml` mode takes precedence (peer override).
- [x] **Frontend writeHtml-before-openUrl** — `confirmSend` in `SendToManagerButton.svelte` calls `render_weekly_summary_preview` then `writeHtml(html, text)` BEFORE the compose invoke. If clipboard write throws, we abort the openUrl and surface an in-modal recovery block with "Open Preview" link. No silent empty-draft sends.
- [x] **Send button label + hint flip** — `Copy + Open Gmail` / `Copy + Open Mac Mail` / `Copy + Open Outlook` when clipboard mode active. Modal mode-tip swaps to "Opens X with an empty body and copies the formatted message. Press Cmd+V in the draft, then Send."
- [x] **Loosened Clipboard button** — the Preview-modal Copy button now ALWAYS calls `writeHtml(html, text)` regardless of mode (was branching on `previewShowsHtml` before). HTML-aware paste targets get rich content; plaintext targets get the plaintext fallback via OS pasteboard negotiation.

**Settings → Mail tab restructure**

- [x] **"How should Send work?" consolidated section at the top** — Send-to-manager path dropdown FIRST, then Body delivery radio, then Body format radio (conditional). Replaces the previous split where Body delivery sat above the send-mode picker as its own section.
- [x] **Body format hidden in clipboard-paste mode** — the radio only renders when Body delivery is Prefilled. When clipboard-paste is active, plaintext flavor doesn't matter (recipient sees rendered HTML from the paste).
- [x] **Native Mac HTML toggle promoted to a standalone checkbox** — was a 3-way radio (Clean text / Markdown source / Styled HTML) coupling `mail_body_format` to `mail_native_html`. Split into a separate "Send as Styled HTML draft (.eml)" checkbox under the Native Mac section, clearly labeled as an independent peer override.
- [x] **Forward-pointer tips** — Gmail and Outlook tip bullets that used to point at the manual Preview → Copy workflow now point at the new Compose + paste mode. Native Mac tip unchanged (HTML toggle is the direct path there).

**Editor rendering fixes** (surfaced during smoke testing)

- [x] **Setext heading disable** — `markdown({ extensions: [GFM, { remove: ['SetextHeading'] }] })`. Typing a paragraph then a `-` on the next line no longer parses as an H2 underline (which retroactively re-rendered the paragraph above as a heading).
- [x] **Tab key as focus traversal outside lists** — custom `listAwareTab` KeyBinding replaces `indentWithTab`. Walks the lezer tree from the cursor: inside a `BulletList` / `OrderedList` / `ListItem` it indents (nests). Outside it returns `false` so the browser handles native Tab focus traversal (fixes a keyboard-trap accessibility issue).
- [x] **Markdown keymap re-enabled with explicit precedence** — imported `markdownKeymap` from `@codemirror/lang-markdown` and slotted it into our `keymap.of([...])` array AFTER `listAwareTab` but BEFORE `defaultKeymap`. Gives back the auto-continue Enter behavior for bullet AND numbered lists without letting `defaultKeymap`'s `insertNewline` swallow the event first.
- [x] **Hang-indent for wrapped list lines** — new technique: `padding-left: <depth>*2ch` on the line, `margin-left: -2ch` on the marker widget. Marker pulls itself back to column 0; content area starts at column 2; wrapped rows naturally align under row 1's content. Previous attempt with `text-indent: -2ch` was clipping the inline-block bullet widget in WebKit (mechanism still unclear).
- [x] **Numbered list digits readable in dark mode** — new `OrderedListMarkerWidget` mirrors `BulletWidget`: replaces source `1.` / `2.` etc. with a styled span (`.cm-md-list-num`). Replacing-via-widget bypasses CodeMirror's default-highlight rule for `tags.processingInstruction`, which was winning the cascade against any class-level color rule and leaving digits illegibly faint. Same color + opacity as the bullet glyph for visual consistency.
- [x] **Task list double-marker** — `- [ ] item` was rendering as `• ☐ item` (bullet AND checkbox). Now suppresses the BulletWidget when the parent `ListItem` has a `Task` child node, AND still emits a `Decoration.replace({})` to hide the source `-`. Renders as just `☐ item`.

**Out of scope (intentional):**
- Per-line ordered-list padding to handle 10+ items (`12.` is 3 chars but padding-left assumes 2ch). Visual overlap for 10+ items accepted for now; lists in weekly summaries rarely run that long.
- Hang-indent for blockquoted lists. The `.cm-md-list-line.cm-md-blockquote-line` joint selector is in place from 2.9b but untested under the new margin-left technique; punt to a real-world need.

### Deferred from Phase 2.5

- [x] **HTML email body via `pulldown-cmark`** — original Step 5 of 2.5. Landed in Phase 2.9 (renderer + sanitizer), re-enabled in 2.9b via Native Mac Mail's HTML toggle.
- [x] **Preview modal on `/summary`** — original Step 6 of 2.5. Landed in Phase 2.9 inside the SendToManagerButton confirm modal; 2.9b made it universal across all send modes and added clipboard copy.

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

## Phase 2.7 — Onboarding + Settings revisit ✅ (shipped 2026-06-26)

The first-run wizard captures the bare minimum today (name, journal location, reminder); after 2.6 the data model grew enough — and the Settings screen is long enough — that both deserve a polish pass.

- [x] **"Tell me about you" wizard step** — name, Bamboo title (with the word *Bamboo* linking to Prodigy's BambooHR site), Jira project keys (comma-separated, e.g. `MAGE`, multiple allowed).
- [x] **"Tell me about your manager" wizard step** — manager name + email. Both fields reuse the columns added in 2.6.
- [x] **Settings layout** — single long-scroll form split into three tabs (General / Reminders / Theme), with General sub-sectioned ("Your details…" / "Manager details…" / "Journal location…").
- [x] **Persistence** — Bamboo title + Jira project keys joined `JournalSettings`; same `.metadata/settings.json`.
- [x] **Multi-day reminders** (added during 2.7, in response to a MAGE-dev feature request) — `dayOfWeek: u8` → `daysOfWeek: Vec<u8>`, day-pill picker in Settings; serde back-compat shim accepts legacy single-value files. DST-safe scheduling via `chrono` naive-date arithmetic + per-target localization (gap → bump by 7 days preserving weekday, ambiguous → earliest).
- [x] **Dock icon restoration on `.Accessory → .Regular` flip** (incidental fix) — `restore_dock_icon()` re-sets `NSApplication.applicationIconImage` from the bundle's `icon.icns` via objc2 so the dock icon survives the activation-policy round-trip.

## Phase 2.7b — Cross-app UX polish ✅ (shipped 2026-06-26)

Parallel polish pass that ran alongside 2.7 — UI legacy sweep, contrast audit, button/UX standardization, and component extractions to remove duplication ahead of Custom Themes.

- [x] **Dark-theme contrast audit** — 30+ AA failures fixed across the app. Added `--accent-primary-text` (#ff8e51) and `--accent-pink-text` (#ff80c0) tokens for dark-surface readability; swept `text-muted` on `bg-elevated` to `text-secondary` where contrast failed; dropped `--accent-teal` from `LabelInput`'s palette (1.61:1 fail).
- [x] **Button/UX standardization** — primary action always left, Cancel/Back/Discard always right and always `btn-ruby` (maroon). Save status always leftmost in the actions row on `/journal`, `/summary`, `/capture` for a unified scan location.
- [x] **WeekStripe scrollbar-gutter fix** — `html { scrollbar-gutter: stable; }` so the day-of-week stripe doesn't shift between scrollable and non-scrollable routes.
- [x] **`.actions-area` wrapper** — sent-status line lifted above the actions row on both `/journal` and `/summary`, right-justified, parent owns spacing via flex `gap` so the line spaces identically on both routes.
- [x] **Shared component extractions** — `ExternalUpdateBanner`, `SaveStatus`, `InputField`, `SendToManagerButton` (the last brought send-to-manager to `/journal` for free, gated on a selected week).
- [x] **`app.css` token & class promotion** — `.text-input`, `.card`, `.sent-status` promoted to global utility classes; error-tint tokens (`--bg-error-tint`, `--bg-error-tint-soft`, `--border-error`) added via `color-mix()`; four dead tokens removed.
- [x] **Various fixes found by three rounds of adversarial verification** — own-save race in `pendingCommit`, `/summary` `onDestroy` missing `autoSaveTimer` cleanup, concurrent `saveNow` clobbering pending state, pre-normalization signature mismatch, undefined `var(--space-5)` in modal-actions, day-pill white-on-orange at 13px (3.25:1).

## Phase 2.8 — Custom Themes ✅ (shipped 2026-06-30)

Wide-token-surface OKLCH-derived themes. The user picks 12 primaries (3 bg + 3 text + 2 borders + 4 accents); the engine derives ~23 dependent tokens to hit WCAG AA against the chosen surfaces. Theme picker grows from Light/Dark to Light/Dark/Custom. Themes serialize as `.captheme.json` (schema v1) and round-trip via Tauri save/open dialogs.

- [x] **Token prep pass** — promoted 4 hardcoded literals in `app.css` to tokens so derivation can override them (`--btn-primary-text`, `--accent-green-text`, `--brand-orange`/`--brand-maroon` aliases).
- [x] **Derivation engine in TypeScript** (`$lib/theme.ts`) — culori (~12 KB, MIT, OKLCH-native) parses primaries to OKLCH, iterates L until AA ratios hit (4.5:1 text, 3:1 focus/UI). Reproduces shipping Light + Dark within OKLab dE ≤ 0.04 when seeded with their primaries (verified via 38 vitest cases).
- [x] **`iterateForContrast` convergence guarantee** — returns `{value, ratio, converged}`; on non-convergence falls back to better-of-{black, white} against the host surface so AA-required tokens never paint a low-contrast value. `targetEnd` derives from `inferBaseFromSurface(host)` rather than the global base arg, so a pale surface marked Dark still walks toward black.
- [x] **Rust persistence** — `AppSettings.theme` widened to `Light | Dark | Custom`. New `CustomTheme` struct with 12 hex fields; strict `^#[0-9a-fA-F]{6}$` deserializer normalizes to lowercase. `serde(default)` so legacy `app-settings.json` files load with `theme: Dark, custom_theme: None`. Switching theme to Light/Dark does NOT clobber the saved `custom_theme` payload (verified by `update_settings` invariant).
- [x] **Theme tab editor** — 4 section groups (Backgrounds / Text / Borders / Accents), per-token row with 28×28 swatch + monospace hex input + AA contrast warning under offending rows. Live preview on every keystroke. First-time activation seeds from the currently active Light/Dark with an inline hint.
- [x] **Edit lifecycle** — `customEditorDirty` flag flips on first edit, cleared on save/cancel. Re-clicking the active radio is a no-op. Switching Custom → Light → Custom preserves in-flight edits.
- [x] **Export / Import** — `.captheme.json` schema (`{ $schema, name, author, base, tokens: { 12 keys } }`). Tauri save/open dialogs. Strict validation on import (schema version, required keys, hex format) with specific error messages. Imported themes are guarded by an "import pending" flag — clicking Cancel before Done prompts before discarding.
- [x] **Polish + edge cases** — `--selection-fg` derived for legibility on pale accents. Mid-grey surface (L 0.50-0.60 OKLCH) forces explicit base pick. Saturated-surface warning when chroma > 0.15.
- [x] **Tray-menu "Preset Theme" escape hatch** — right-click the tray icon → Preset Theme submenu → Dark Mode / Light Mode. Safety net when a Custom palette makes the in-app theme picker unreadable. `AppSettings.custom_theme` survives the swap so the user can re-activate Custom from Settings once they can see it again.

### Phase 2.8b — Colorful Labels ✅ (shipped 2026-06-30)

Layered on top of 2.8. Each label gets a per-name hue when the toggle is on; deterministic by name (same label → same hue across sessions), theme-aware (regenerates against the active surface so Dark-tuned hues don't burn into disk and become invisible under Light).

- [x] **`JournalSettings.colorful_labels: bool`** (default `false`) — per-journal toggle. `serde(default)` for back-compat.
- [x] **Settings → Theme tab checkbox** — "Colorful labels" with helper copy explaining the per-name determinism.
- [x] **`generateLabelColor()` in `$lib/theme.ts`** — djb2 hash on label name → hue (0-360), fixed chroma 0.12, theme-aware lightness (0.70 dark / 0.45 light; Custom keys off `--bg-surface` OKLCH L).
- [x] **No-lazy-persist design** — chip rendering computes via `generateLabelColor()` at render time. `labels.json` `color` field is reserved for explicit user overrides from the future Label Manager (the `set_label_color` Tauri command already ships and is preserved). Eliminates the theme-burn problem: a Dark-generated hue never gets written to disk where it would become invisible under Light.
- [x] **Atomic labels.json writes** — `LocalFilesystem::write_metadata` stages to `<name>.tmp` in the same `.metadata/` directory then renames over the target. Combined with `storage.write().await` locks on `create_note` and `set_label_color`, concurrent mutations no longer race.
- [x] **`themeNonce` reactivity** — `LabelInput.svelte` subscribes to `settings-changed`; on emit, `themeNonce += 1`. `colorfulChipStyle` reads `themeNonce` at the top of its body so Svelte tracks it as a dependency — Custom-theme `--bg-surface` tweaks propagate to chip colors on the next paint, no remount needed.

**Known limitation:** double-digit ordered-list markers (`10.+`) visually overlap content by 1ch under the hang-indent technique from Phase 2.9c — unchanged in 2.8. Revisit if anyone writes a 10+ item ordered list.

### Phase 2.8c — Onboarding polish + Preview modal refactor ✅ (shipped 2026-06-30)

Layered on top of 2.8 / 2.8b in the same day. Started as a "delete my settings file" first-run smoke test that surfaced a Gmail clipboard-skip bug; grew to absorb a shared-component extraction pass and a SendToManagerButton Preview refactor onto the shared Modal.

- [x] **Gmail + Compose+paste clipboard-skip fix** — `confirmSend` in `SendToManagerButton.svelte` had condition `if (mailBodyDelivery === 'clipboard-paste' && !mailNativeHtml)` — too broad. Once `mailNativeHtml` had ever been true (from earlier Native Mac exploration), the Mail-tab UI hid the toggle while in Gmail mode so the user couldn't turn it back off; frontend skipped the `writeHtml` call, backend correctly emitted an empty-body Gmail URL → silent empty-draft send. Dropped `&& !mailNativeHtml` from the condition: clipboard always populates in clipboard-paste mode. Backend still handles the Native Mac HTML `.eml` peer override correctly.
- [x] **Shared component extractions** — `$lib/Modal.svelte` (backdrop dim+blur, body-scroll lock, topmost-Escape stack, focus management, `zLayer` + `maxWidth` props); `$lib/ConfirmDialog.svelte` (wraps Modal with `zLayer="nested"`); `$lib/LoadingOverlay.svelte` (reusable spinner); `$lib/PointerFinger.svelte` (32×32 sprite + bob animation, restored from an earlier-refactor regression); `$lib/onboarding/StepHeader.svelte` (h1/h2 + `.lead` pattern shared across all 5 wizard steps); `$lib/PathPickerField.svelte` (label + path input + Browse button + Tauri dialog plumbing, used by onboarding step 4 + `/settings` journal-location row). Replaces ~5 duplicated implementations across onboarding, settings, and send-to-manager surfaces.
- [x] **Visual unification** — dim+blur backdrops everywhere, btn-emerald / ruby / marble color tokens, Title Case button labels, consistent radio-circle treatment across Theme and Mail tabs via `:has()` pure-CSS.
- [x] **SendToManagerButton Preview refactor** — replaced the bespoke inline backdrop/modal markup with `<Modal>` (`zLayer="nested"`, `maxWidth=min(640px, calc(100vw - 32px))`). Added a `From:` line gated on `userEmail` (mirrors what mail clients hide by default — useful for the manager-facing preview). Widened the `previewShowsHtml` derived to include `mailBodyDelivery === 'clipboard-paste'` so Compose+paste users now see the fully rendered HTML in the iframe instead of plaintext. Buttons retitled (`Close preview` → `Close`, `Copy to clipboard` → `Copy To Clipboard`) and reordered so Close + Copy sit side-by-side at the lower-right matching every other modal in the app; the status pill (Copied! / error) sits at the row's left edge via `margin-right: auto` so the two buttons stay glued together.
- [x] **SendToManagerButton Confirm refactor** — the "Send weekly summary?" confirm dialog was still on its own inline `.modal-backdrop` + `.modal` markup. Refactored onto `<Modal>` with the title prop carrying the dialog heading. Dropped the redundant window-level Escape listener and `escListener` plumbing — Modal's topmost-stack listener owns Escape for both Confirm and Preview now (Preview's `onClose={closePreview}` already does the `previewToken` bump that the legacy handler was responsible for). Removed `.modal-backdrop`, `.modal`, `.modal h2` rules from the component; re-scoped the paragraph + `<strong>` body styles under a `.send-confirm-body` wrapper.
- [x] **StepHeader regression cleanup** — an earlier slice-3 onboarding extraction had tightened h1 `.lead` margin-bottom from `var(--space-6)` to `var(--space-4)` (violating "no behavior change" mandate; Welcome + All-set screens regressed). Dropped the level-based split, single `.lead { margin-bottom: var(--space-6) }` rule for both h1 and h2.
- [x] **PathPickerField hint color unified** — was using `--text-muted` while InputField used `--text-secondary`. Switched to `--text-secondary` to match.

**Verification:**
- `svelte-check`: 420 files, 0 errors, 0 warnings.
- Manual: deleted `app-settings.json` to retrigger first-run wizard, walked through all 5 steps; smoke-tested the Preview modal across Gmail / Native Mac / Outlook in both Prefilled and Compose+paste delivery modes; confirmed From line renders only when `user_email` is set and HTML preview renders for clipboard-paste mode.

## Phase 3 — Label Library + Search & Navigation + Task List

Sequenced intentionally: the Label library viewer (3a) is the natural starter because it needs label→Notes navigation, which is just a constrained search. Phase 3b's full-text search generalizes that result-list + week-jump plumbing across all weekly content. Phase 3c layers a task-list aggregator on top — pulls `- [ ]` items out of every weekly file's "Plans and priorities for next week" section into a live view on the landing page, with bidirectional sync back to source markdown and an opt-in week-rollover mechanic that folds completed tasks into the new week's Key Accomplishments.

### Phase 3a — Label library viewer + bulk management ✅ (shipped 2026-07-06)

- [x] **Browse + filter all labels** — substring filter input on the Settings → Labels tab, sorted by count desc then name asc. Shipped in Phase 2.8b's original Labels tab; still current.
- [x] **Drill from a label into the matching Notes / Summaries** — new `get_notes_for_label` Rust command walks every weekly file, returns one `LabelReference` per site (newest-first) with the enclosing Note's timestamp + optional title. New "Referenced In" section in `LabelDetailsModal` renders the list with kind badges, week labels, and click-to-navigate that closes the modal and `goto('/journal?year=Y&week=W')`. `/journal` reads those URL params on mount, expands the target year in the sidebar tree, and selects the week. List is capped at 50 rows in the DOM with a `TipBubble` explaining truncation; the modal itself stays scrollable within the shared Modal shell.
- [x] **Per-label color override picker** — already shipped in Phase 2.8b's `LabelDetailsModal` (swatch + hex input + Reset-to-auto). No 3a work needed.
- [x] **Bulk delete + bulk merge** — multi-select checkboxes added to every Labels-tab row plus a select-all in the toolbar above the list. Toolbar's right side surfaces **Delete N** (ruby, opens a shared ConfirmDialog) and **Merge into…** (marble, disabled until 2+ labels are selected). Merge picker is a nested Modal with radio-select of the currently-selected label names, pre-selected to the highest-count label; on confirm, loops `rename_label(source, canonical)` for every non-canonical — `rename_label` already auto-merges when the target exists, so no new backend needed for the merge case. Delete loops `delete_label_cascade`. Both ops continue past individual failures and aggregate results into a persistent banner above the list ("Deleted 3 labels" / "Deleted 2 of 3. Failed: bugX (error…)."). `onLabelMutated` prunes stale names out of the selection set when a bulk-selected label gets renamed or deleted through the Details modal.

**Dropped from original scope:** bulk-rename as a distinct mode. The design converged on rename-into-existing = merge, so the multi-select bulk-rename UI was redundant. Single-label rename stays accessible via the Details button on any row.

**Verification:** 6 new Rust tests (4 for the `extract_note_heading_before` helper, 2 integration tests against `LocalFilesystem` covering cross-year ordering + both site kinds with note metadata); full suite 248/248. `svelte-check` clean at 420/0/0 across all Slice 1 + Slice 2 changes.

### Phase 3b — Search & Navigation ✅ (shipped 2026-07-06)

- [x] **Full-text search across every weekly file** — new `search_journal` Rust command scans both the Weekly Summary block (four content fields joined) AND every individual Note's labels-line + body per file. Case-insensitive substring match, literal (no regex), 2-character query floor. Results ordered newest-first: years desc, weeks desc within year; within a week, Summary first (if matched) then Notes in document order.
- [x] **Label filter** — optional multi-select via `LabelInput`. OR-semantics: any label overlap counts. Applies independently to Summary (`### Labels` subsection) and Notes (`**Labels:**` line), so a week's Summary can pass while its Notes get filtered out, or vice versa. Date-range and file filters intentionally deferred — the current UX proved sufficient without them.
- [x] **Click search result → opens correct week, scrolls to correct Note** — new `scroll_offset` field on `SearchResult` carries the byte offset (0 for Summary, heading offset for Note). `/search` result click routes to `/journal?year=Y&week=W&scrollTo=<offset>`. New `scrollTargetOffset` prop on `MarkdownEditor` dispatches `EditorView.scrollIntoView(pos, { y: 'center' })` once both view and offset are ready; clamps to doc length so stale deep-links don't blow up.
- [x] **`Cmd+K` global shortcut** — window-level keydown listener in `+layout.svelte` navigates to `/search` from any main-window surface. Skipped on `/capture`; no-op on `/search` itself so a stray shortcut doesn't loop the page.
- [x] **Year/week tree handles many years gracefully** — already true from Phase 2.5b; no new work in 3b.

**Verification:** 13 Rust tests total (7 existing + 4 new for Note-body search / scroll offset / kind discrimination / label filter on notes + 2 for literal-metacharacter / cross-field / unicode). All 259 tests pass. Frontend `svelte-check` 422/0/0. Manual smoke: Summary-only searches, Note-only searches, mixed weeks, deep-link scroll-to, Cmd+K from `/journal` / `/settings` / `/summary` all landing correctly.

### Phase 3c — Task list aggregator ✅ (shipped 2026-07-07)

Aggregates `- [ ]` items from every weekly file into a live task view on the landing page, with bidirectional sync back to source markdown. Design derived from a prior-art sweep (Obsidian Tasks / Logseq / Things 3 / Bullet Journal); decision brief captured in the DEVELOPMENT-JOURNAL 2026-07-06 entry.

**Slice 1 — read-only task list** (commit `d1c2421`)

- [x] `list_tasks` Tauri command: parses `- [ ] / - [x]` lines out of the current week's Plans-and-priorities body, returns `{year, week, text, textHash, ordinal, isCompleted, completedAt, originalWeek}`. Server-side render of `render_task_text_inline` (pulldown-cmark → ammonia inline-only allowlist) so bold/italic/strike/code/br land safely into `{@html}`.
- [x] Scrollable task list below the three primary landing-page buttons. Row = ARIA checkbox + task text + provenance chip + optional timestamp chip. Empty state via shared `<TipBubble>`.
- [x] Composite identity `(year, week, textHash, ordinal)` with `normalize_task_text` = trim + collapse whitespace + lowercase + strip trailing `.,!?:;`. Ordinal disambiguates same-hash duplicates by per-hash file-order rank.

**Slice 2 — toggle** (commit `d1c2421`)

- [x] `toggle_task` command flips the checkbox marker byte in-place and updates the `.metadata/task-completions.json` sidecar. Markdown wins for state; sidecar wins for timestamp. Emits `weekly-file-changed` so `/summary` reconciles.
- [x] Sidecar posture: missing file → empty; corrupt file → empty + stderr warning. Losing the sidecar costs precise timestamps for pre-existing checks and nothing else.

**Slice 3 — add task** (commit `d1c2421`)

- [x] `append_task_to_current_week` command + landing-page **+ Add Task** modal. Validation: non-empty after trim, no embedded newlines, `MAX_TASK_TEXT_LEN = 1024` bytes, no `- [` prefix. Scaffolds the weekly file if missing.

**Slice 4 — task-list settings tab** (commit `d1c2421`)

- [x] Settings > Tasks tab with four toggles: `showCompleted` (default on), `openTasksFirst` (default on), `showCompletedTimestamp` (default off), `hideTaskList` (default off). All persist to `.metadata/settings.json` under `taskList`.
- [x] **Rebuild task index** button + Tip: walks every weekly file, backfills missing sidecar entries, prunes stale ones, and sweeps stranded incomplete tasks from any older week into the current week.

**Slice 5 — weekly rollover with provenance** (commit `d1c2421`)

- [x] `check_and_apply_rollover` command copies incomplete tasks from the immediately-previous ISO week into the current week's Plans section. `RolloverLog` sidecar (`.metadata/rollover-log.json`) tracks `last_run_to_week` (per-week idempotence) + `provenance` (`{year, week, textHash, ordinal, originalYear, originalWeek, originalCreatedAt}` — survives multi-hop rollovers).
- [x] Frontend triggers: onMount + `tauri://focus` + `visibilitychange` + `captainslog:week-changed` + 60s safety interval. Rollover receipt toast on the landing page when tasks are carried forward. `taskList.autoRolloverEnabled` toggle in settings (default on).
- [x] Rebuild sweep uses `open_first_seen` HashMap to dedupe stranded tasks across weeks — the earliest occurrence wins, and its provenance is preserved.

**Verification:** `cargo test`: 411 → 414 → 417 → … → 433 passing across slices. `svelte-check`: clean at 422/0/0. Manual smoke on real journal: multiple toggles, adds, rollovers across the ISO week boundary — no data loss, correct provenance chips.

### Phase 3d — Task rearchitecture (Slices 6a → 6c) ✅ (shipped 2026-07-10)

Slice 5 shipped the aggregator on top of "tasks embedded in the Plans-and-priorities body." Slice 6 rearchitects tasks into a dedicated `### Tasks` section with HTML-comment anchors, promotes them to first-class objects in the UI, and layers row-actions + import + auto-import on top. Locked design brief in `~/.claude/.../memory/project_captains_log_slice6_design.md`.

**Slice 6a — tasks as first-class objects** (commit `d1c2421` + `52f4199` finishing touches)

- [x] **New `### Tasks` section** in each weekly file, anchored by HTML comments so parsing survives header renames or accidental edits:
  ```
  ### Tasks
  <!-- captainslog:tasks:incomplete -->
  - [ ] Task text
  <!-- captainslog:tasks:completed -->
  - [x] Done task
  ```
- [x] **Lazy migration on first write.** Legacy files (`[ ]`/`[x]` in Plans body, no `### Tasks` section) migrate on the next mutation (toggle, edit, delete, append, import, rollover). Pre-migration bytes back up to `.metadata/pre-slice6-backups/{YYYY}-Www.md` (idempotent by file presence — never clobbers an existing backup).
- [x] **Move-on-check.** Toggling `- [ ]` moves the line from the Incomplete anchor block to the end of the Completed anchor block (and back on uncheck). Position within a state's sub-list is the identity; ordinal is recomputed on each parse.
- [x] **Positional sidecar re-key on toggle.** Moving a same-hash duplicate can shift every same-hash task's ordinal. The toggle rebuilds the `(year, week, hash)` group in the sidecar by pairing pre-toggle completed_at values (with `Option<String>` slots for manually-added tasks that never had one) with the NEW file positions in file order. No timestamp is ever handed to the wrong task.
- [x] **Landing-page visual headers.** Task list rendered in two grouped sub-lists ("Incomplete Tasks" / "Completed Tasks") matching the file's anchor partition, via a Svelte 5 snippet.
- [x] **Rollover source-dedup fix.** Rollover now dedupes source-week tasks by text_hash in file order — two identical open source tasks copy forward as one.
- [x] **`/summary` editor drops task rendering** naturally: migration moves tasks out of the Plans body → `plansAndPriorities` in the WeeklySummary IPC becomes prose-only. `update_weekly_summary` explicitly preserves the on-disk `tasks_body` so a summary save never clobbers the task list.

**Slice 6b — inline row actions: edit** (commit `52f4199`)

- [x] **Pencil icon** at the trailing edge of each task row. Click → text swaps for an inline input (autofocus + select-all via `bind:this` + `$effect`), Enter saves, Escape cancels, blur cancels. Same-text edits short-circuit with no round-trip.
- [x] `edit_task_in_tasks_body` helper: locates task by `(hash, ordinal)`, preserves leading whitespace + checkbox marker case (`[X]` stays uppercase), swaps only the text portion. Returns `(new_body, new_hash, new_ordinal, is_completed)`.
- [x] `edit_task` Tauri command: read-migrate-backup-write shape; positional key-map re-keys sidecar + provenance across the hash change so completion timestamp + "from last week" chip survive typo fixes. Handles same-hash sibling drift.

**Slice 6c — inline row actions: delete** (commit `52f4199`)

- [x] **Trash icon** at the row's far trailing edge. Modal confirmation (`btn-ruby` Delete + `btn-marble` Cancel; task text quoted in a bordered blockquote; blockDismissal while in-flight).
- [x] `delete_task_from_tasks_body` helper + `delete_task` command. Positional key-map handles sibling ordinal drift (`old_all` and `new_all` differ in length by 1; map old position i to new position i or i-1 depending on the deleted task's position).

**Slice 6c-followup — Copy Completed → Key Accomplishments** (commit `52f4199`)

- [x] `merge_completed_tasks_into_key_accomplishments` helper: (a) dedupe candidates against every existing line in the field via `normalize_task_text` (bullets and prose both count as "already there"), (b) find an existing `#### Completed Tasks` heading and append new bullets at the end of its contiguous bullet block; if no heading, append a fresh block at the end with a blank-line separator. Repeated imports never stack duplicate headings.
- [x] `import_completed_tasks` Tauri command. `/summary` **+ Import completed tasks** button uses the backend command; flushes pending dirty edits via `saveNow()` FIRST (avoids a false external-update banner) then invokes.

**Slice 6c-followup — auto-import** (commit `52f4199`)

- [x] `AutoImportLog` sidecar (`.metadata/auto-import-log.json`) with `last_import_date` (local YYYY-MM-DD).
- [x] `check_and_apply_auto_task_import` Tauri command. Two gates: (a) `taskList.autoImportCompleted` setting toggle (default on), (b) local-date match with `last_import_date`. When both open, delegates to `import_completed_tasks_impl` and stamps the log. Stamped even on "no completed tasks" runs so we don't re-check every trigger event all day.
- [x] Landing-page triggers fire alongside `check_and_apply_rollover` on onMount + `tauri://focus` + `visibilitychange` + `captainslog:week-changed` + 60s safety interval.

**Slice 6c audit fixes** (commit `4511f44`)

- [x] `import_completed_tasks_impl`: stamp `last_updated` in the all-duplicates + was-migrated branch (was writing the migrated file with a stale timestamp).
- [x] `toggle_task` takes `AppHandle` + emits `weekly-file-changed` — aligns it with `edit_task` / `delete_task` / `import_completed_tasks`. `/summary` now reconciles a landing-page toggle if it's open on the same week.
- [x] Delete confirmation modal focuses the Delete button on open (was landing on the dialog card; keyboard users had to Tab past copy).
- [x] `.delete-confirm-quote` background switched from `color-mix(brand-maroon 6%, transparent)` to `--bg-elevated` — was near-invisible in dark theme.

**Verification:** 470 Rust tests (~60 new). Manual smoke on Chris's real journal covered delete of an incomplete + a completed task, delete of one of two duplicate tasks, delete of one of four duplicates spanning both states — sidecar + provenance stayed in perfect bijection with the file across all cases.

**Deferred to Phase 3e** — Task due dates (calendar action, overdue heading, date chip). Shipped 2026-07-10; see Phase 3e section below for the receipt.

**Deferred as post-3d follow-ups** (documented in DEVELOPMENT-JOURNAL 2026-07-10 entry):

- Focus restoration after successful edit or delete (currently focus lands on document.body).
- Edit-input `onblur = cancel` — arguable; may want save-on-blur.
- Auto-import silent failure surface (backend errors console.error only).
- Import receipt error-tone auto-clears after 5s (should persist errors).
- Orphan sidecar/provenance entries after manual file edits (Rebuild handles it; low real-world impact).

**Out of scope (intentional)**

- Task states beyond `- [ ]` / `- [x]` (BuJo has cancelled, migrated, scheduled — not yet).
- Drag-reorder within the task list.
- Gamification (streaks, counts, achievements) — BuJo anti-pattern.
- Multiple task formats (emoji format vs. dataview format). Plain `- [ ]` only, no metadata sigils *(revisited in Phase 3e for due dates)*.

### Phase 3e — Task due dates + reminders ✅ (shipped 2026-07-10)

Optional due dates on tasks with a calendar-icon row action + a landing-page "Overdue" heading, plus OS-notification task reminders fired "X days before due, at time Y." Design locked 2026-07-10 across two plans-out rounds; shipped the same day in three parts on top of Phase 3d.

Prereq (satisfied): Noot notification asset upscaled from RPG assets to 170×170 and committed at `app/src-tauri/icons/noot-prompt.png`.

**Part A — TaskDueDates backend** (commit `617225c`)

- New sidecar `.metadata/task-due-dates.json`. Keyed by `(year, week, textHash, ordinal)` — identical shape to `TaskCompletions` + `RolloverLog`. Value: `{ dueDate: "YYYY-MM-DD" }` (local date; no time-of-day).
- `TaskDueDates::load` / `save` follow the established sidecar posture: missing file → empty; corrupt file → empty + stderr warning; atomic write via staged `.tmp` + rename.
- `set_task_due_date(year, week, text_hash, ordinal, due_date: Option<String>)` Tauri command — `Some` = set/update; `None` = clear. Same read-migrate-backup-write shape as toggle/edit/delete.
- **Cross-command re-key.** `edit_task`, `delete_task`, `toggle_task`, `check_and_apply_rollover`, and the Rebuild sweep all extend the positional key-map they already run for `TaskCompletions` + provenance so a due-date entry follows the task across renames, deletes, toggles, and weekly rollovers. Rollover carries the debt forward verbatim — an overdue-in-source-week task stays overdue in the target week.
- Reconciliation: `list_tasks` joins the sidecar with parsed tasks by `(year, week, hash, ord)`; orphans are ignored on read and pruned by Rebuild.

**Part B — due-dates frontend** (commits `8ca8e46`, `00b0d79`, `bc4edf5`)

- Third inline icon between the pencil and the trash on each incomplete task row: a calendar. Click opens a `DatePickerPopover` anchored to the icon.
- Popover extended with an `onClear` action (disabled when the task has no date) and commit-on-Today so clicking Today both fills the field and applies. Tasks that already have a date open the picker seeded to that date.
- Chip variants alongside the origin + timestamp chips: "Due today" (equal to local date) / "Due Fri" (this week) / "Due Jul 15" (this year) / "Due Jul 15, 2027" (different year). Chip click reopens the picker seeded to the current date. `.task-due-chip` for on-time; `.task-due-chip.overdue` flips background + text to `--brand-maroon` tones so it reads as danger. New `--brand-maroon-text` dark-mode token added to keep AA in the dark theme without leaking maroon into the whole scale.
- **Overdue section header** on the landing page. When there are any overdue incomplete tasks the Incomplete group splits into two sub-groups: **Overdue** at the top (same visual weight as "Incomplete Tasks" / "Completed Tasks"), then **Incomplete Tasks**. Zero overdue → only "Incomplete Tasks" renders. Overdue = `due_date` strictly earlier than today's local date; a task due today shows "Due today" but sits in Incomplete (one grace day). Completed tasks never appear in Overdue regardless of date.
- **Sort within Overdue: earliest due date first** (oldest debt on top). Incomplete + Completed remain file order.
- **Chip hidden on completed rows** (per follow-up user feedback) — a completed dated task no longer shows its Due chip; unchecking it restores the chip because the underlying sidecar entry is preserved by the positional re-key.

**Part C — task reminders** (commit `dd7c7b5`)

Layered on top of due dates: OS notifications fired "X days before due, at time Y" for tasks with a due date. Reuses the journal-reminder scheduling architecture (single `tokio::spawn` loop, chunked polling against `Local::now()` for DST + system-sleep safety, mac-usernotifications in the bundled `.app` with mac-notification-sys fallback in dev). Same "must be running to fire" constraint as the journal reminder.

- **`TaskReminderSettings`** struct in `settings.rs` — `enabled: bool`, `days_before: u8`, `hour: u8`, `minute: u8`, `#[serde(default)]` for legacy settings.json compatibility.
- **`task_reminder_task` background loop** in `reminders.rs`, parallel to `restart_reminder_task`. Wakes → loads `TaskDueDates` + `TaskCompletions` + `TaskReminderSettings` → computes the pending queue (every incomplete task with a due date → fire time = `due_date - days_before` at `hour:minute` local) → filters past fire times → sleeps until the earliest fire time in ≤5 min chunks → dispatches a `UNUserNotificationCenter` notification (or fallback) on fire → loops. New `TaskReminderHandle` state in `lib.rs`, mirroring `ReminderHandle`.
- **Reschedule triggers.** `restart_task_reminder_task(app, handle, config)` is called from every relevant mutation: `update_settings`, `set_task_due_date`, `edit_task`, `delete_task`, `toggle_task`, `check_and_apply_rollover`, `import_completed_tasks`, `check_and_apply_auto_task_import`. Cancellation is implicit — a completed / deleted / date-cleared / reminder-disabled task falls out of the pending queue on the next reschedule.
- **Notification content**: title `Captain's Log — Task Reminder`; body `"{task_text}" is due {when}` where `{when}` follows the chip format; icon = Noot (`app/src-tauri/icons/noot-prompt.png`); click opens the main window to the landing page via the UN-action-button + Tauri event pattern the journal reminder uses.
- **Settings > Tasks tab UI** — three fields below the existing task-list toggles: `Enable task reminders` (default on), `Days before due` (0..30, 0 = day-of), `Time of day` (default 09:00). One global config; no per-task overrides.

**Out of scope (intentional)**

- Recurring due dates.
- Time-of-day components on due dates themselves (dates only, per BuJo). Reminders have a time of day, but that's the reminder settings, not the task.
- Reminders that survive app quit (would require `UNCalendarNotificationTrigger` — a big scope expansion; ship in-process for now, matching the journal reminder's contract).
- Per-task reminder overrides. Follow-up if the global default proves too coarse.
- Setting a due date at task-add time. The + Add Task modal stays text-only.
- Global setting to disable the due-date feature. Opt-in per task (no date = no chip).

**Verification.** New Rust sidecar coverage: load/save roundtrip, missing-file / corrupt-file posture, set-then-clear, rollover carries the date forward, edit-then-check preserves under the new hash, delete + re-key of same-hash siblings, toggle-preserves-date, Rebuild orphan sweep. Manual on Chris's real journal: added a task with due=yesterday → surfaced under Overdue with a red chip; set to today → chip flipped to "Due today" and row moved back to Incomplete; cleared the date → chip disappeared. Reminders: due=tomorrow / days_before=1 / time=09:00 with system clock at 08:00 today → notification fired within a minute. Due=today / days_before=3 → silently skipped (past). Completing a task before its reminder fired → reminder never fired. Settings change mid-wait → reschedule used the new offset. App restart mid-wait → the fresh spawn recomputed and picked up the same fire time.

**Lessons + follow-ups**

- **H2/H3 truncation bugs found mid-flight** (fixes `cf2856b` + `fbbe35d`). `extract_subsection` was treating any user-typed heading at the boundary depth as a section terminator, which meant a note body that happened to open with `### Something` could truncate the surrounding section on the next parse. Rewrote both extractors to only treat the known `SECTION_KEY_*` headings ("Weekly Summary", "Plans and Priorities", "Tasks", "Weekly Notes") as boundaries; arbitrary user headings inside a section body are now inert.
- **Pre-release cleanup, tier 1** (commit `c845116`) — deleted the stale `toggle_checkbox_in_plans` + `append_task_to_plans` helpers (both superseded by the Slice 6 `### Tasks`-section variants) plus 21 tests that only exercised the deleted code paths. No behavior change; the "task lives in Plans body" era is fully gone.
- **Pre-release cleanup, tier 2** (commit `314d27b`) — extracted `TaskRowActionButton` + `TaskMetaChip` out of the landing page (three copies each on incomplete + completed + overdue rows collapsed to one component per shape), swapped the delete confirmation modal to the shared `ConfirmDialog`, added pencil / trash / check to the `Icon` component, and collapsed `+page.svelte` by ~407 lines.
- **Toggle re-key needs a position-move algorithm (NOT zip).** Documented in commit `617225c`. When a task is toggled the file order of the incomplete + completed sub-lists both shift, so `parse_plans_tasks(old).zip(parse_plans_tasks(new))` misaligns for same-hash duplicates: e.g. toggling the second of two identical `- [ ]` tasks maps position 1 in the old file (incomplete #1) to position 0 in the new file (incomplete #0) plus position N in the new file (end of completed). Solved by computing an explicit `(old_position → new_position)` map that walks the toggled task's move + shifts every other same-hash sibling accordingly. Same pattern reused by `edit_task`, `delete_task`, and rollover.

### Phase 4 — Link Enrichment ✅ (shipped 2026-07-15)

Every markdown link in the editor renders as an inline pill chip — favicon on the left, `[text]` label on the right — backed by a generic HTML-head scraper. Storage stays plain markdown (the source line is still `[Ticket](https://…)`); the chip is a CodeMirror rendering layer on top. Enrichment is service-agnostic: no curated hostname list, no MCP connectors, no per-service branding. Anything that returns HTML with an `og:title` / `<title>` / `og:site_name` and a `<link rel="icon">` renders as a pretty chip; anything auth-gated or non-HTML falls back to a hostname/globe chip and still reads cleanly. Paste handlers upgrade bare URLs to `[title](url)` async so pasting a Jira ticket into Notes produces a chip within a second or two.

**Backend enrichment pipeline** (`src-tauri/src/link_enrich.rs`)

- New `enrich_link(url, force_refresh?)` Tauri command. Fetches the URL with `reqwest` (rustls-tls, JSON feature, 3-second timeout, ~2MB HTML body cap), parses the head with `scraper`, extracts `og:title` → `<title>` → hostname fallback, `og:site_name` when present, and the first `<link rel="icon">` (or `/favicon.ico` fallback). Follows redirects and resolves relative favicon paths against the final URL.
- Favicons fetched separately, capped by size, and cached inline as base64 data URIs on the enrichment record. Failed favicon fetches store `None` and the frontend paints a globe glyph.
- `.metadata/link-cache.json` sidecar mirrors the TaskDueDates posture exactly: missing → empty map; corrupt JSON → empty + stderr warning (no crash, no truncation of the sibling `.metadata/` files); atomic write via staged `.tmp` + rename. Keyed by URL; value carries `{ title, siteName, faviconDataUri, fetchedAt }`.
- Auth-gated hosts (Jira, private GitHub, Slack, Confluence when the session cookie isn't on the request) return an empty `EnrichmentResult` — the frontend interprets that as "render the hostname/globe fallback." No hardcoded per-service branch: whatever fails to yield a title just falls back.
- Two new Rust deps: `reqwest` with `rustls-tls` + `json` (no OpenSSL, no system tls), `scraper` for HTML head parsing.
- 27 tests in `link_enrich`: happy path, `og:title` beats `<title>`, `<title>` beats hostname, relative favicon resolution, `/favicon.ico` fallback, empty-body posture, non-HTML content-type, redirect chain, timeout branch, oversized body cap, cache hit vs miss, `force_refresh` skips cache, missing-cache-file posture, corrupt-cache-file posture, atomic write.

**Editor UX — link chip widget + paste handlers** (`app/src/lib/link-chip.ts`, `app/src/lib/link-paste.ts`)

- `linkChip` ViewPlugin walks the Lezer syntax tree for `Link` nodes across the visible viewport, per-view enrichment cache (URL → EnrichmentResult), StateEffect-driven refresh when async `enrich_link` resolves. First render paints an unenriched hostname chip immediately; the enrichment upgrade slides in on the next tick without a layout thrash.
- `LinkChipWidget` renders a `<button>` pill (`display: inline-block`) with two children: a favicon span (`background-image: url(<data-uri>)` — deliberately not an `<img>` element; see lessons) and a label span with the source `[text]`. `Decoration.replace` covers the whole markdown link range, `atomicRanges` registered so arrow-key traversal skips the chip cleanly.
- **Cursor-inside-hides rule.** When the cursor sits inside the `[text](url)` range, the widget is suppressed so the source is visible and editable — the same convention the date chip uses.
- **Plain click opens.** Left-click on the chip fires a Tauri opener call with the URL. No modifier required (matches native reader expectations; Cmd-click was the old convention on `markdown-links.ts` and is gone now that the chip is unambiguous).
- **Alt-click edits.** Alt+click selects the `[text]` portion of the source so the user can type-to-replace. Position lookup uses `view.posAtDOM(btn)` + a live syntax-tree walk from that position to find the enclosing `Link` node's `LinkMark` children — NOT positions baked into the widget at construction time. This is the load-bearing fix from the vanish saga; see lessons.
- **linkPaste extension** (registered as `EditorView.domEventHandlers({ paste })` on MarkdownEditor). URL-only paste with a selection wraps `[selected](url)` (Slack pattern). No-selection paste inserts the bare URL immediately, then kicks off `enrich_link` async and swaps the range to `[title](url)` when the title comes back. If enrichment fails or returns empty, the bare URL stays on disk.

**Chip styling** (MarkdownEditor.svelte, alongside `.cm-date-chip`)

- `inline-block` pill matching the date chip's proven shape. No `max-width`, no `overflow` clip, no flex tricks — deliberately minimal so nothing exotic interacts with WebKit line-wrap layout (see lessons).
- Favicon via `background-image` on a fixed-size span (16×16, `background-size: contain`), not `<img>` — `<img>` load events fire post-mount and were part of the wrap-boundary vanish repro on WebKit.
- Same border-radius, padding, and typography as `.cm-date-chip`.

**Import dedup fix** (`src-tauri/src/tasks.rs`)

- New `strip_markdown_links_for_dedup(text)` helper: strips `[text](url)` down to `url` before hashing/comparing in `normalize_task_text`. Consumed by `merge_completed_tasks_into_key_accomplishments` so both forms of the same task text — the raw bullet in the `### Tasks` block and the paste-upgraded `[title](url)` form in the Key-Accomplishments field — normalize to the same string.
- **Why this is needed.** The linkPaste extension runs on the CodeMirror editor surface (Notes body, Weekly Summary fields, `/journal` raw markdown). It does NOT run on the plain `<input>` in the + Add Task modal or the inline task-edit input — both are native inputs, not CM6. So a user who pastes a bare URL into + Add Task ends up with a raw-URL task on disk; when that task later gets completed and imported via the Copy-Completed flow, the Key-Accomplishments bullet under `#### Completed Tasks` gets pasted-and-upgraded to `[title](url)`. Next auto-import iteration compared `raw url` (task text) against `[title](url)` (existing bullet), decided they were different, and stacked a duplicate. Stripping links on both sides before compare closes the loop.
- 7 new tests: strip preserves plain text, strip on plain-URL is identity, strip on wrapped-URL yields the URL, dedup treats `url` and `[title](url)` as equal, dedup preserves order when the bullet is already present in either form, real-world sample from Chris's journal round-trips cleanly across a simulated auto-import.

**Out of scope (intentional)**

- MCP-connector enrichment (Jira / Confluence / GitHub / Slack via authenticated MCP calls). Deferred pending real-world need — the hostname/globe fallback already reads acceptably, and the paste-upgrade `[title](url)` behavior means the source markdown carries the human-readable label without needing a live fetch.
- Curated per-service branding (Jira icon, GitHub octocat, Slack hash, etc.). No hardcoded hostname list; every host runs through the same head-scraper.
- Generic oEmbed client. Same rationale — defer until a service actually needs it.
- Auth-gated title extraction (Jira ticket keys, private GitHub, etc.). Would require MCP or per-service auth; not worth the complexity when the fallback is fine.
- Paste-handler integration in the + Add Task modal and the inline task-edit input. Both are plain `<input>` elements, not CodeMirror surfaces, so the `EditorView.domEventHandlers` extension doesn't attach. Follow-up: either move those inputs onto a minimal single-line CM6 instance, or wire a plain DOM paste listener with the same enrich_link upgrade. Import dedup already handles the mismatch as of this phase, so the visual asymmetry is the only remaining pain.

**Verification**

- `cargo test`: **522 passing** (up from 493 pre-Phase 4; +34 = 27 link_enrich + 7 dedup).
- `svelte-check`: clean at 425/0/0.
- `vite build`: clean, no new warnings.
- Manual smoke on Chris's real journal:
  - Paste-with-selection wrap: selected `MAGE-1041`, pasted `https://prodigygame.atlassian.net/browse/MAGE-1041` → wrapped to `[MAGE-1041](https://…)` inline. Chip rendered with hostname/globe (Jira is auth-gated). Selection wrap behavior matches Slack.
  - Paste-no-selection upgrade: pasted a bare GitHub PR URL into a blank line → bare URL appeared immediately as a hostname chip, then upgraded to `[PR title · repo](url)` about 800ms later once `enrich_link` returned.
  - Alt-click edit: Alt-clicked a chip mid-line → `[text]` portion of the source selected, typed replacement text, chip re-rendered with new label. Positions computed correctly even after ~200 chars of unrelated edits earlier in the doc.
  - Wrap-boundary stability: placed a chip at position `col 78` of an 80-col wrapping line, then typed and deleted characters elsewhere in the doc — chip stayed painted through every mutation. Repeated on both `/journal` and `/summary` editors.
  - Import dedup: on a journal week that already had a paste-upgraded `[title](url)` bullet under `#### Completed Tasks` plus a matching raw-URL task in `### Tasks`, ran auto-import → no duplicate appeared. Reverted the fix locally to confirm the duplicate reproduced.

**Lessons + follow-ups**

- **The vanish saga.** WebKit-specific: chip disappeared at line-wrap boundaries when the doc changed elsewhere. Four rounds of fixes before the real one landed. Rounds 1–3 were CSS: max-width variants (`max-width: 100%` → `unset`), `overflow` values, `inline-flex` vs `inline-block`, `<img>` vs `background-image`. All plausible, none load-bearing; the chip would still vanish under the exact repro. Round 4 was structural: the widget's `eq()` method was comparing `from` / `to` positions, so any doc edit far from the link (deleting a character three lines up) shifted the link's from/to and made `eq()` return false. CodeMirror then tore down and rebuilt the widget DOM *inside an already-laid-out wrapping line box*, and WebKit sometimes lost the widget node during that reflow — probably a layout-invalidation cache miss triggered by the mid-flow node swap. The fix: content-only `eq()` (`text`, `url`, `faviconDataUri` — nothing positional), with all click handlers computing live positions via `view.posAtDOM(btn)` + syntax-tree walk. The date chip has the exact same `eq()` pattern but its labels are short (like `Jul 15`) and rarely land at a wrap boundary, so the vanish never surfaced there. Filed as a note in DEVELOPMENT-JOURNAL 2026-07-15 for future widget authors.
- **Widget eq() should be content-only, positions should be live.** Generalizes the vanish fix. Any future CM6 widget in this app should follow the same rule.
- **`background-image` beats `<img>` for widget favicons.** Fewer subresource load events, no reflow when the image resolves, no CSP surprises for data URIs.
- **Follow-up: task-input paste handlers.** The + Add Task modal input and the inline task-edit input don't get the paste-upgrade treatment because they're plain `<input>` elements. Import dedup covers the correctness gap, but the visual asymmetry (paste into a Note → chip; paste into + Add Task → raw URL that renders as a chip only *after* it lands in the file and gets read back through CM6) is a real UX papercut. Two options: (a) swap those inputs to a minimal single-line CM6 EditorView with the paste extension enabled, or (b) attach a plain-DOM `paste` listener that calls `enrich_link` and rewrites the input value. Punt until it bites in practice.
- **Follow-up: MCP-driven enrichment for auth-gated hosts.** The hostname/globe fallback is acceptable but not delightful for Jira / private GitHub / Slack. If it becomes annoying, the natural extension is an MCP branch inside `enrich_link` that dispatches to the appropriate connector based on hostname pattern, falls back to the generic head-scrape on connector miss. Deferred.
- **Follow-up: cache TTL / invalidation.** The `.metadata/link-cache.json` sidecar currently never expires entries. Fine for the current usage pattern (stable page titles), but a Confluence page title change wouldn't propagate without a manual `force_refresh` call. No UI for that today; the frontend never passes `force_refresh: true`. If titles drift becomes a real complaint, add a "Refresh link chip" affordance on Alt-click, or an age-based TTL in the load path.

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
