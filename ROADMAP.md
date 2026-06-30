# Captain's Log — Roadmap

## Current phase: Phase 2.8 ✅ done — next up Phase 3a (Label library viewer + bulk management)

Phase 1 MVP and Phase 2 polish are complete. Phase 2.6 ("Send weekly summary to manager") shipped 2026-06-24. Phase 2.5 (editor upgrade, Architecture B live-preview) shipped 2026-06-25 — Slack/Typora-style marker hiding on CodeMirror 6 with markdown-on-disk; live-preview engine, widgets (date chip + picker, bullets, task checkboxes), toolbar overhaul, /journal Preview/Source toggle, layout chrome polish, and an architecture doc all landed in a single session. Phase 2.7 (onboarding wizard expansion + Settings tabbed redesign + multi-day reminders) shipped 2026-06-26, plus a cross-app UX polish pass (Phase 2.7b): dark-theme contrast audit + 30+ fixes, button/UX standardization, shared component extractions, and a scrollbar-gutter fix. Phase 2.9 (HTML email body + Preview modal) landed 2026-06-26 but was dark-released — Phase 2.9b (2026-06-29) finished the job by adding a Mail tab to Settings, three send modes (Gmail default, Native Mac Mail, Outlook), a universal Preview modal with clipboard, a week-rollover fix, and a sleep-drift fix on the reminder scheduler. Phase 2.9c (2026-06-29) layered on the "Compose + paste" body-delivery mode (open empty compose + write rich HTML to clipboard = 2-click formatted send across all clients), restructured the Mail tab around a single "How should Send work?" section, and burned down a stack of editor-rendering bugs around lists, numbered-marker contrast, and task-item double-markers.

Phase 2.8 (Custom Themes) shipped 2026-06-30 — 12 user-editable primaries → ~23 OKLCH-derived tokens via culori, Theme = Light / Dark / Custom, AA contrast warnings, hex-input editor, `.captheme.json` export/import via Tauri dialogs, plus a "Colorful Labels" follow-on that gives each label a per-name hue (theme-aware, regenerates on switch — no lazy-persist, so no theme-burn). A tray-menu "Preset Theme" submenu (Dark / Light) is the escape hatch for when a Custom palette makes the in-app theme picker unreadable. Phase 2.8c (2026-06-30) layered a shared-component pass on top — `Modal`, `ConfirmDialog`, `LoadingOverlay`, `PointerFinger`, `StepHeader`, `PathPickerField` extracted out of onboarding / settings / send-to-manager — refactored the SendToManagerButton Preview popup onto the shared Modal (From: line gated on `user_email`, HTML render now shown in Compose+paste mode too, Close + Copy buttons placed side-by-side at the lower-right), and fixed a Gmail + Compose+paste clipboard-skip bug where a stale `mailNativeHtml` flag short-circuited the `writeHtml` call.

**Next up: Phase 3a — Label Library viewer + bulk management.** Browse / filter all labels, drill from a label into the Notes that use it, surface the per-label color override picker that closes the loop on Phase 2.8b's `set_label_color` plumbing, plus bulk rename / merge / delete with atomic writes. Phase 3b then layers full-text search + date/file filters + click-to-jump on top of the same plumbing.

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

## Phase 3 — Label Library + Search & Navigation

Sequenced intentionally: the Label library viewer (3a) is the natural starter because it needs label→Notes navigation, which is just a constrained search. Phase 3b's full-text search then generalizes that result-list + week-jump plumbing across all weekly content.

### Phase 3a — Label library viewer + bulk management

- [ ] **Browse + filter all labels** in use across the journal. Sits on top of the existing `.metadata/labels.json` index (already maintained by `record_note_labels`). Filter input (substring + maybe tag-cloud-by-recency).
- [ ] **Drill from a label into the matching Notes / Summaries** — first surface that needs a label→content lookup. The plumbing here becomes Phase 3b's result-list + week-jump foundation.
- [ ] **Per-label color override picker** — surfaces the `set_label_color` Tauri command that Phase 2.8b already shipped. When Colorful Labels is on, the picker shows the auto-generated hue (from `generateLabelColor`) AND a Set / Reset button. "Set" persists a user override to `labels.json`; "Reset" clears `color: None`. Closes the loop on the design Chris locked in — auto-derived for new labels, explicitly overridable here.
- [ ] **Bulk rename / merge / delete** — natural extension of the same screen. Merge: pick 2+ labels, choose one as canonical, rewrites every occurrence across weekly files. Delete: remove a label from `labels.json` AND strip it from every Note/Summary that references it. Rename: in-place across files. All three need careful atomic-write semantics — reuse the staged `.tmp` + rename pattern from 2.8b's labels.json hardening.

### Phase 3b — Search & Navigation

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
