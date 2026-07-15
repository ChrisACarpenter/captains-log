# UX Flows

Step-by-step interaction flows for the core experiences.

Flows marked **(Phase N — planned)** describe future work and are aspirational. Everything else reflects current behavior.

## Flow 1 — Quick capture (the main loop)

Goal: 2 clicks from "I just did a thing" to "it's logged."

1. User clicks the menu bar icon (🧭)
2. Quick-capture popup appears (460×460px window, resizable)
   - Cursor focused in the body text field (live-preview MarkdownEditor)
   - Optional title field at top
   - Labels field below (chip-based autocomplete)
3. User types
4. User clicks **Submit** (or hits ⌘↩)
5. Popup closes
6. Note is appended to the current week's file with the current timestamp

**Auto-save:** while the user is typing, the in-progress note is debounced to `<root>/.metadata/capture-draft.json` every 1.5s. If they close the popup mid-edit, reopening restores the draft. **Discard** explicitly clears it (with a confirmation prompt).

**Edge cases:**

- User clicks outside the popup → hides the window silently (draft persists)
- User hits Esc → hides the window silently (draft persists)
- User clicks **Discard** → native confirm; on Yes, cancels pending saves, deletes the draft, hides the popup

## Flow 2 — Weekly Summary

Goal: ~2 minutes to write a structured summary using the 4-field template.

1. User opens the main window (Dock icon or "Show Captain's Log" from the tray menu)
2. Navigates to `/summary` (or follows a reminder notification straight there)
3. The four-field form is always editable — there's no separate "edit mode." Each field is a live-preview MarkdownEditor.
4. User fills in any or all of the four fields plus the Labels chip input
5. Edits autosave (1.5s debounce). The [SaveStatus](../app/src/lib/SaveStatus.svelte) indicator next to the Save button reports current state.
6. **Send weekly summary** (in the actions row) opens the [SendToManagerButton](../app/src/lib/SendToManagerButton.svelte) confirm modal — see Flow 7 below.
7. If reminders are enabled and the summary's content hash changes, that week's reminder is suppressed.

## Flow 2a — Task Management

Goal: capture and close out short-lived todos alongside the weekly summary — with due dates, rollover, and OS reminders — without leaving the landing page.

Surface: the landing page (`/`) displays tasks under two (sometimes three) headings, all sourced from the current week's `### Tasks` section (delimited by `<!-- captainslog:tasks:incomplete -->` / `<!-- captainslog:tasks:completed -->` anchors):

- **Overdue** — only rendered when it has entries. Sorted earliest due-date first.
- **Incomplete Tasks** — file order.
- **Completed Tasks** — file order, muted styling.

Each row's inline actions are hosted by [TaskRowActionButton](components.md#taskrowactionbutton):

- **Pencil (edit)** → swaps the row text for an inline input. Enter commits, Escape cancels.
- **Calendar (due date)** → opens [DatePickerPopover](components.md#datepickerpopover) anchored to the icon. Actions: **Set** / **Clear** / **Today** (Today commits immediately without needing a second click).
- **Trash (delete)** → opens [ConfirmDialog](components.md#confirmdialog) before removing the task from the file.

Row metadata is rendered by [TaskMetaChip](components.md#taskmetachip):

- *origin* — small chip when the task was rolled over from the prior week.
- *time* — "checked N ago" chip on completed rows (opt-in via Settings > Tasks).
- *due* — accent chip when a due date is set.
- *due-overdue* — maroon, bold variant when the due date is earlier than today. Hidden on completed rows.

**Rollover.** On the first open of a fresh week (roughly Monday morning), any incomplete tasks from the prior week auto-import into the current week's `### Tasks` section. A [RolloverReceipt](components.md#rolloverreceipt) banner announces the count and auto-dismisses. Provenance (which task came from which prior week) is stored in `.metadata/rollover-log.json` so the *origin* chip survives across sessions.

**Auto-import to Key Accomplishments.** Once per local day, completed tasks from the prior week merge into that week's `### Key accomplishments` section as prose bullets. Runs are tracked in `.metadata/auto-import-log.json` to avoid duplicate merges.

**Add-task.** A **+ Add Task** button under the incomplete list opens a minimal modal (text-only). Due dates aren't collected here — set them afterwards via the row's calendar action.

**Task reminders (Phase 3e).** If enabled in Settings > Tasks, an OS notification fires *"X days before due at HH:MM"* for each task with a due date. Noot icon; body reads `"<task text>" is due <when>`. Clicking the notification opens the landing page.

**Settings > Tasks tab.** Task-list display toggles (show completed, open tasks first, "checked N ago" chip, auto-rollover, auto-import to Key Accomplishments) plus reminder controls (enable, days-before, time-of-day). See Flow 4 below.

## Flow 3 — Editing a past Note / week

1. User opens the main window and navigates to `/journal`
2. Sidebar shows a collapsible year/week tree (newest first; current year auto-expanded)
3. Click a week → that week's full markdown file loads in the editor on the right
4. Toggle between **Preview** and **Source** modes via the segmented control (or ⌘⇧S) — Preview shows live-rendered markdown with hidden markers; Source shows raw text. Cursor position survives the toggle (Phase 2.5b compartment-based extension swap).
5. Edit freely — Notes appear as `###`-headed sections inside the week file. There's no per-Note "click to edit" mode; everything is raw-markdown editing of the weekly file.
6. Edits autosave (1.5s debounce). Saving emits a `weekly-file-changed` Tauri event so `/summary` (if open on the same week) reconciles or prompts about unsaved edits.

## Flow 4 — Settings

Reachable from `/settings` (via the main window's nav). Six tabs:

1. **General** — three sub-sections:
   - *Your details* — name, email, Bamboo title, Jira project keys
   - *Manager details* — name, email (feed the Send-to-Manager flow)
   - *Journal location* — folder picker via [PathPickerField](../app/src/lib/PathPickerField.svelte)
2. **Reminders** — enable toggle, multi-day pills, time picker. Notification-permission tip if macOS hasn't granted it yet.
3. **Mail** — Send-to-Manager dispatch path (Gmail / Native Mac Mail / Outlook), body delivery mode (Prefilled vs. Compose + paste), body format (Clean text vs. Markdown source), per-mode tips and sub-controls (Outlook flavor, Native HTML toggle).
4. **Theme** — Light / Dark / Custom. Custom theme editor (12 primaries → ~23 OKLCH-derived tokens, AA contrast warnings, `.captheme.json` export/import). Colorful Labels toggle.
5. **Labels** — per-label details modal (rename / color override / delete). Bulk operations land in Phase 3a.
6. **Tasks** — task-list display toggles (show completed, open tasks first, "checked N ago" chip, auto-rollover of incomplete tasks, auto-import completed tasks into Key Accomplishments) plus task-reminder controls (enable, days-before-due, time-of-day). See Flow 2a.

Tray-menu escape hatches: "Preset Theme" submenu (Dark / Light) flips theme without going through Settings — used when a Custom palette makes the in-app picker unreadable.

## Flow 5 — Link chips (Phase 4)

Goal: paste a URL into any live-preview MarkdownEditor and get an inline pill (favicon + label) instead of a raw hyperlink, without disrupting normal markdown editing.

Surface: every live-preview MarkdownEditor — quick capture, Weekly Summary fields, `/journal` week body, task-row inline editors. Implemented as CodeMirror extensions ([app/src/lib/link-chip.ts](../app/src/lib/link-chip.ts) + [app/src/lib/link-paste.ts](../app/src/lib/link-paste.ts)); there's no matching Svelte component in [components.md](components.md) because the chip is a `Decoration.replace` widget, not a mountable component.

**Paste.**

- URL-only paste **with a selection** → wraps as `[selected](url)` (Slack pattern). No async work; commits immediately.
- URL-only paste **with no selection** → inserts the bare URL, then kicks off `enrich_link(url)` asynchronously. When enrichment resolves with a usable title, the bare URL is rewritten to `[title](url)` and a `StateEffect` triggers the widget refresh. If enrichment returns empty (auth-gated host, timeout, non-HTML), the bare URL stays put — no fake title is invented.

**Chip rendering.** Any `[text](url)` markdown in live-preview mode renders as a pill: favicon on the left, label on the right, hover tooltip carrying the resolved page title / site name. The label is always the markdown `[text]` — enrichment only contributes the favicon and the tooltip, never the visible text. This means users can rename a link freely (`[MAGE-1234](url)`) and the chip respects that.

**Interactions.**

- **Plain click** on the chip → opens the URL in the system browser via Tauri's opener plugin. No in-app webview.
- **Alt-click** on the chip → the widget computes its live position with `view.posAtDOM(btn)` and walks the Lezer syntax tree to find the enclosing Link node, then selects the `[text]` range so the user can type-to-replace the label. Escape or moving the cursor out re-renders the chip.
- **Cursor-inside-hides** rule (matches the inline date-chip behavior in `MarkdownEditor.svelte`): moving the cursor into the link range unwraps the chip and shows the raw `[text](url)` markdown, so it's directly editable without a modal or side panel. Cursor-out re-renders.

**Auth-gated fallback.** Jira, Slack, Confluence, private GitHub, and any other host that returns an auth redirect (or refuses the HEAD/GET) produce an empty `EnrichmentResult` — the fields except `url` and `fetchedAt` are null. The chip still renders, but with a generic globe icon and the URL's hostname as the label if no `[text]` was authored. Users who want the ticket key visible can Alt-click and retype (e.g. `[MAGE-1234](https://prodigygame.atlassian.net/browse/MAGE-1234)`); that edited label survives across sessions because it's stored in the markdown, not in the cache.

**Cache posture.** Both the successful and the empty enrichments are persisted to `.metadata/link-cache.json` — no retries on subsequent renders. See data-format.md for the schema. To force a refresh (e.g. a Jira ticket that later becomes public), the `enrich_link` command accepts `force_refresh: true`; there's no UI hook for this in v1.

## Flow 6 — Weekly reminder notification

1. At the configured day(s)/time, macOS posts a native notification (via `UNUserNotificationCenter`)
2. Message: *"Time to log this week's Summary, Chris."* with optional **Write** action button (visible when system Alert Style is Persistent)
3. Click the notification or **Write** → main window opens to `/summary`
4. The scheduler re-derives the next fire instant from `chrono::Local::now()` each loop iteration, so sleep / hibernation doesn't leave the next fire stuck in the past (Phase 2.9b fix)
5. Summary edits that change the content hash suppress further reminders for that week

## Flow 7 — Send weekly summary to manager

Surface: a **Send weekly summary** button on `/summary` and `/journal` (when a week is selected). Implementation: [SendToManagerButton](../app/src/lib/SendToManagerButton.svelte).

1. Button is gated on: manager email set, no unsaved edits, not already sent with matching content hash. Tooltip explains any disabled reason.
2. Click → confirm modal opens (shared [Modal](../app/src/lib/Modal.svelte)). Shows manager address, week label, mode-specific tip ([TipBubble](../app/src/lib/onboarding/TipBubble.svelte)).
3. **Preview** opens a nested Modal with the rendered email — HTML iframe for Compose+paste / Native HTML modes; plaintext for Prefilled modes. **Copy To Clipboard** writes rich HTML + plaintext fallback via the OS pasteboard.
4. **Send** dispatches by mode:
   - **Gmail** — opens `https://mail.google.com/mail/u/{address}/?view=cm&to=...&su=...&body=...` (or `body=` empty in Compose+paste mode, with the body on the clipboard for Cmd+V).
   - **Native Mac Mail** — AppleScript via `osascript` pipes a Tell-Mail block with sender + recipient + subject + content. Optional `.eml` peer-override path for Styled HTML mode.
   - **Outlook** — opens the appropriate compose URL for Business or Personal flavor.
5. On confirmed dispatch, the sent-log (`<root>/.metadata/sent-log.json`) is stamped with `sentAt`, `contentHash`, `sentTo`.
6. Resends re-derive the hash; the button surface shows `Sent {time}` when locked or `Send updated version` (stale) when the content has shifted.

## Flow 8 — Search & Navigation (Phase 3b — planned)

Full-text search across all weekly files with label / date / file filters and click-to-jump. Not yet shipped; design parked until Phase 3a (Label Library viewer + bulk management) lands first.

## Flow 9 — Performance Review export (Phase 5 — planned)

The reason this whole app exists. Date-range picker, review-question templates, bundled markdown export with link-enriched metadata, one-click "draft my review" handoff to an LLM. Not yet shipped; design lives in [ROADMAP.md](../ROADMAP.md#phase-5--performance-review-module).
