# UX Flows

Step-by-step interaction flows for the core experiences.

## Flow 1 — Quick capture (the main loop)

Goal: 2 clicks from "I just did a thing" to "it's logged."

1. User clicks the menu bar icon (🧭)
2. Quick-capture popup appears (small window, ~400×300px)
   - Cursor focused in the body text field
   - Optional title field at top (small, easily skipped)
   - Labels field below
3. User types
4. User clicks **Submit** (or hits ⌘↩)
5. Popup closes
6. Note is written to current week's file with the current timestamp

**Edge cases:**

- User clicks outside the popup → minimize, don't lose the draft (next icon click reopens with draft)
- User hits Esc with content → confirm "Discard draft?"
- User hits Esc with empty popup → close silently

## Flow 2 — Weekly Summary

Goal: ~2 minutes to write a structured summary using the 4-field template.

1. User clicks Dock icon or "Open Captain's Log" from menu bar
2. Full journal window opens, current week selected by default
3. User clicks **Edit Weekly Summary** (or it's already in edit mode if empty)
4. The 4-field form appears with this week's Notes shown in a side pane for reference
5. User fills in any or all of the 4 fields
6. **Save** writes back to the file's `## Weekly Summary` section
7. If reminders are enabled, this week is marked "summarized" — no more reminders this week

## Flow 3 — Search (Phase 3)

Goal: find a specific Note in seconds.

1. User opens the journal window
2. Clicks search icon (or ⌘F)
3. Search bar appears at top
4. Filters: free-text, label (multi-select), date range
5. Results list shows matching Notes with snippet + week + timestamp
6. Click a result → navigate to that Note in its week

## Flow 4 — Settings

Reachable from the menu bar icon menu and from the journal window.

Tabs:

1. **Profile** — name
2. **Storage** — journal folder location, "Open in Finder" button, "Move folder" button
3. **Reminders** — on/off, day, time
4. **Labels** (Phase 2) — bulk rename / merge / delete
5. **About** — version, links to docs, "Open journal folder" shortcut

## Flow 5 — Editing a past Note

1. User opens the journal window
2. Navigates the year/week tree on the left to find the right week
3. Selects the week → markdown view appears in the main pane
4. Clicks the Note's heading to enter edit mode for that Note specifically (not the whole file)
5. Edits body text or labels
6. **Save** updates the file and the label index if labels changed

Full-file raw-markdown mode is also available for power users who want to edit the whole week's file directly.

## Flow 6 — Weekly reminder notification

1. At the configured day/time, macOS notification fires
2. Message: *"Time to log this week's Summary, Chris."*
3. Click the notification → journal window opens with the Weekly Summary edit mode active
4. Dismiss → reminder stops for this week (won't re-fire same week)

If the user dismisses, the next reminder fires at the same configured time next week.

## Flow 7 — Performance Review export (Phase 5)

The reason this whole app exists.

1. User opens the journal window → **Performance Review** tab
2. Calendar UI to pick a date range (start date + end date), with quick presets ("Last 6 months," "H1 2026," etc.)
3. User picks a review template (e.g. Prodigy mid-year, 8 questions)
4. Optional: edits the LLM instruction block
5. Clicks **Generate draft**
6. App bundles every Note + Weekly Summary in the range into one markdown file, with link-enriched metadata, and sends it to the configured LLM with the template prompts
7. First-draft answers appear, editable side-by-side with the source bundle
8. User edits, exports as text/markdown, or copies to clipboard for pasting into BambooHR / Lattice / etc.
