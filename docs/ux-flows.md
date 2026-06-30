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
6. **Send weekly summary** (in the actions row) opens the [SendToManagerButton](../app/src/lib/SendToManagerButton.svelte) confirm modal — see Flow 6 below.
7. If reminders are enabled and the summary's content hash changes, that week's reminder is suppressed.

## Flow 3 — Editing a past Note / week

1. User opens the main window and navigates to `/journal`
2. Sidebar shows a collapsible year/week tree (newest first; current year auto-expanded)
3. Click a week → that week's full markdown file loads in the editor on the right
4. Toggle between **Preview** and **Source** modes via the segmented control (or ⌘⇧S) — Preview shows live-rendered markdown with hidden markers; Source shows raw text. Cursor position survives the toggle (Phase 2.5b compartment-based extension swap).
5. Edit freely — Notes appear as `###`-headed sections inside the week file. There's no per-Note "click to edit" mode; everything is raw-markdown editing of the weekly file.
6. Edits autosave (1.5s debounce). Saving emits a `weekly-file-changed` Tauri event so `/summary` (if open on the same week) reconciles or prompts about unsaved edits.

## Flow 4 — Settings

Reachable from `/settings` (via the main window's nav). Five tabs:

1. **General** — three sub-sections:
   - *Your details* — name, email, Bamboo title, Jira project keys
   - *Manager details* — name, email (feed the Send-to-Manager flow)
   - *Journal location* — folder picker via [PathPickerField](../app/src/lib/PathPickerField.svelte)
2. **Reminders** — enable toggle, multi-day pills, time picker. Notification-permission tip if macOS hasn't granted it yet.
3. **Mail** — Send-to-Manager dispatch path (Gmail / Native Mac Mail / Outlook), body delivery mode (Prefilled vs. Compose + paste), body format (Clean text vs. Markdown source), per-mode tips and sub-controls (Outlook flavor, Native HTML toggle).
4. **Theme** — Light / Dark / Custom. Custom theme editor (12 primaries → ~23 OKLCH-derived tokens, AA contrast warnings, `.captheme.json` export/import). Colorful Labels toggle.
5. **Labels** — per-label details modal (rename / color override / delete). Bulk operations land in Phase 3a.

Tray-menu escape hatches: "Preset Theme" submenu (Dark / Light) flips theme without going through Settings — used when a Custom palette makes the in-app picker unreadable.

## Flow 5 — Weekly reminder notification

1. At the configured day(s)/time, macOS posts a native notification (via `UNUserNotificationCenter`)
2. Message: *"Time to log this week's Summary, Chris."* with optional **Write** action button (visible when system Alert Style is Persistent)
3. Click the notification or **Write** → main window opens to `/summary`
4. The scheduler re-derives the next fire instant from `chrono::Local::now()` each loop iteration, so sleep / hibernation doesn't leave the next fire stuck in the past (Phase 2.9b fix)
5. Summary edits that change the content hash suppress further reminders for that week

## Flow 6 — Send weekly summary to manager

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

## Flow 7 — Search & Navigation (Phase 3b — planned)

Full-text search across all weekly files with label / date / file filters and click-to-jump. Not yet shipped; design parked until Phase 3a (Label Library viewer + bulk management) lands first.

## Flow 8 — Performance Review export (Phase 5 — planned)

The reason this whole app exists. Date-range picker, review-question templates, bundled markdown export with link-enriched metadata, one-click "draft my review" handoff to an LLM. Not yet shipped; design lives in [ROADMAP.md](../ROADMAP.md#phase-5--performance-review-module).
