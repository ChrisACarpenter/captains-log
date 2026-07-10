# Data Format Spec

How Notes, Weekly Summaries, and metadata are structured on disk.

## File location

- Journal data root: user-configurable, default `~/Documents/CaptainsLog/`. Subsequent paths in this doc are relative to that root.
- Weekly files: `<root>/YYYY/YYYY-Www.md` (e.g. `~/Documents/CaptainsLog/2026/2026-W25.md`)
- Label index: `<root>/.metadata/labels.json`
- Journal-level settings: `<root>/.metadata/settings.json` (manager, mail, reminder, etc.)
- Task + capture sidecars: `<root>/.metadata/*.json` — see [Metadata sidecars](#metadata-sidecars) below for the full list
- Pre-migration backups: `<root>/.metadata/pre-slice6-backups/YYYY-Www.md` — byte-identical copies of weekly files, taken once each just before the Slice 6a task-section migration first touches them. Written by `save_pre_migration_backup_if_needed` in `commands.rs`; presence-guarded so a second migration attempt never overwrites the original. Escape hatch for hand-recovery if migration output looks wrong.
- App-level settings: `~/Library/Application Support/com.prodigygame.captainslog/app-settings.json` (theme, journal root pointer, last-known UI state)

Week numbers use ISO 8601 (weeks start Monday; week 1 contains the year's first Thursday). This matches what most calendar apps and `date +%V` report.

## Weekly file structure

```markdown
---
period: 2026-W25
start: 2026-06-15
end: 2026-06-21
labels: [release, mage, journal-app, project]
last_modified: 2026-06-18T14:23:01-04:00
---

# Week of June 15 - June 21, 2026

## Weekly Summary
*Last updated: 2026-06-21 17:00*

### Key accomplishments
- ...

### Plans and priorities for next week
- ...

### Challenges or roadblocks
- ...

### Anything else on your mind
- ...

### Labels
#release #mage

### Tasks
<!-- captainslog:tasks:incomplete -->
- [ ] Follow up on the localization bug
<!-- captainslog:tasks:completed -->
- [x] Ship the release

## Weekly Notes

### 2026-06-18 14:23 — Working on the journal app
**Labels:** #journal-app #project

Started planning the new journal/perf-review tool. The #label syntax was tricky to nail down.

### 2026-06-18 10:15 — RPG release captain
**Labels:** #release #mage

Took the release. One small bug found in localization, fixed before ship.
```

## Frontmatter

```yaml
period: 2026-W25                  # ISO year + week
start: 2026-06-15                 # ISO date, Monday of the week
end: 2026-06-21                   # ISO date, Sunday of the week
labels: [...]                     # Union of all labels used in this file
last_modified: 2026-06-18T14:23:01-04:00  # ISO 8601 with timezone
```

The `labels` array is the aggregate of every label appearing anywhere in the file — Labels field on any Note, inline hashtags in any Note body, Weekly Summary fields. Used for fast file-level filtering during search.

## Note structure

Every Note is a `###` heading with this format:

```markdown
### YYYY-MM-DD HH:MM — Optional title
**Labels:** #label1 #label2

Free-form body text. Inline `#labels` here also get parsed.
```

- **Heading line:** `### <ISO date> <time, 24h> — <title>`. Title is optional; if absent, drop the trailing dash too.
- **Labels line:** Optional. Format is `**Labels:** ` followed by space-separated `#labelname` tokens. Parsed via regex `^\*\*Labels:\*\*\s+(.+)$`.
- **Body:** Free markdown. Inline labels follow the `#` + word-character pattern (no space between `#` and the first character). Body labels and field labels both contribute to the Note's label set.

## Weekly Summary structure

```markdown
## Weekly Summary
*Last updated: <ISO timestamp>*

### Key accomplishments
<body>

### Plans and priorities for next week
<body>

### Challenges or roadblocks
<body>

### Anything else on your mind
<body>

### Labels
#tag1 #tag2

### Tasks
<!-- captainslog:tasks:incomplete -->
- [ ] An open task
<!-- captainslog:tasks:completed -->
- [x] A checked task
```

- Always at the top of the file, above Weekly Notes
- All six `###` headings present even if empty (consistency)
- The four prose bodies (Key accomplishments, Plans and priorities, Challenges or roadblocks, Anything else) are free markdown; inline labels in any of them count
- **Plans and priorities is prose-only.** Task checkboxes (`- [ ]` / `- [x]`) used to live here inline with the prose; Slice 6a moved them into their own `### Tasks` section. Legacy files with checkboxes in Plans are migrated opportunistically on the next write (see `migrate_tasks_from_plans` in `notes.rs`); the original file is snapshotted to `.metadata/pre-slice6-backups/` before the first migrating write
- **Labels** subsection is the free-form list of week-level labels (space-separated `#tag` tokens). Parsed by `parse_weekly_summary` and rendered by `render_weekly_summary`
- **Tasks** subsection sits between Labels and `## Weekly Notes`. It always contains two HTML-comment anchors — `<!-- captainslog:tasks:incomplete -->` and `<!-- captainslog:tasks:completed -->` — that mark deterministic insertion points for the two buckets. Task-writing helpers (`append_task_to_tasks_body`, `toggle_task_in_tasks_body`, `edit_task_in_tasks_body`, `delete_task_from_tasks_body`) rely on these anchors; the parser is tamper-robust (it classifies lines by checkbox state, so a user deleting an anchor doesn't lose task state, only the canonical write position). The anchor string constants live in `notes.rs` as `TASKS_ANCHOR_INCOMPLETE` and `TASKS_ANCHOR_COMPLETED`

## Label index

`<root>/.metadata/labels.json`:

```json
{
  "version": 1,
  "labels": [
    {
      "name": "release",
      "count": 47,
      "firstUsed": "2026-01-12",
      "lastUsed": "2026-06-18",
      "color": null
    },
    {
      "name": "journal-app",
      "count": 3,
      "firstUsed": "2026-06-17",
      "lastUsed": "2026-06-18",
      "color": "#ff5c08"
    }
  ]
}
```

JSON keys are camelCase (the Rust struct uses `#[serde(rename_all = "camelCase")]`). Snake-case keys from older files are accepted as a back-compat alias on read.

The optional `color` field (Phase 2.8b) is an explicit per-label hex override; absent or `null` means the chip color is derived at render time from the label name + active theme via `generateLabelColor()`. The field is skipped on serialize when absent to keep older files clean.

Sorted by `lastUsed` desc, then `count` desc (recent + frequent surfaces first in autocomplete). See [label-system.md](label-system.md) for full update rules.

## Metadata sidecars

Small JSON files under `<root>/.metadata/` that hold state which doesn't belong in the markdown itself — completion timestamps, provenance, in-flight drafts. All follow the same load posture: missing file yields the default (empty), corrupt file yields the default plus a stderr warning, and every write goes through the storage backend's atomic `write_metadata` path.

### `task-completions.json`

Per-task completion timestamps. Composite key is `(year, week, textHash, ordinal)` — identical to every other task-scoped sidecar. `completed_at` is ISO 8601 with offset, and may be an approximation backfilled from the source file's mtime when a user checks a task in an external editor.

Rust types live in `tasks.rs` as `TaskCompletions` (top-level wrapper) and `TaskCompletion` (one entry).

```json
{
  "version": 1,
  "completions": [
    {
      "year": 2026,
      "week": 25,
      "textHash": "a4f2…",
      "ordinal": 0,
      "completedAt": "2026-06-19T14:23:00-04:00"
    }
  ]
}
```

Lifecycle: created lazily on the first task check (or first read that backfills mtime-derived stamps). Reconciled against the current week's task markdown on every load — markdown wins for state, sidecar wins for timestamps. Sidecar rows whose task no longer exists are garbage-collected.

### `task-due-dates.json`

Per-task optional due date (Phase 3e). Same composite key as `task-completions.json`. `dueDate` is a LOCAL `YYYY-MM-DD` string — no time-of-day, no timezone — because users pick calendar days; the time-of-day for reminders lives on the reminder settings, not per task.

Rust types live in `tasks.rs` as `TaskDueDates` and `TaskDueDate`. `validate_due_date_input` is the frontend-facing normalizer.

```json
{
  "version": 1,
  "dueDates": [
    {
      "year": 2026,
      "week": 25,
      "textHash": "a4f2…",
      "ordinal": 0,
      "dueDate": "2026-06-26"
    }
  ]
}
```

Lifecycle: created lazily when the user first sets a due date on any task. Entries are `upsert`ed on change and removed via `TaskDueDates::remove` when a user clears the date or deletes the task.

### `rollover-log.json`

Provenance for tasks carried forward across weeks (Phase 3c, extended in Slice 5). Serves two purposes:

1. **Idempotency.** `lastRunToWeek` records the most recent week the rollover targeted. `check_and_apply_rollover` no-ops when it matches the current week, so repeated focus/visibility events never double-copy tasks.
2. **Provenance.** One `provenance` entry per live task instance. When a task is rolled from week N to week N+1, the new entry keeps the same `originalYear`/`originalWeek`/`originalCreatedAt` — so a task carried through multiple weeks can still trace back to where it was born. Chris's ask: paper trail for a future "time to resolution" stat.

Rust types live in `tasks.rs` as `RolloverLog`, `TaskProvenance`, and `YearWeekKey`.

```json
{
  "version": 1,
  "lastRunToWeek": { "year": 2026, "week": 26 },
  "lastRunAt": "2026-06-22T09:15:00-04:00",
  "provenance": [
    {
      "year": 2026,
      "week": 26,
      "textHash": "a4f2…",
      "ordinal": 0,
      "originalYear": 2026,
      "originalWeek": 24,
      "originalCreatedAt": "2026-06-10T11:02:00-04:00"
    }
  ]
}
```

Lifecycle: created lazily on the first rollover run. `TaskProvenance::upsert` overwrites on `(year, week, textHash, ordinal)` collision so a retried rollover doesn't create ghost rows.

### `auto-import-log.json`

Slice 6c-followup — persistent record of when the automated "import completed tasks into Key accomplishments" workflow last ran. The trigger-event handlers (focus / visibility / mount) compare today's local date against `lastImportDate` and no-op if they match, so the import runs at most once per local day regardless of how many events the app sees.

Rust type lives in `tasks.rs` as `AutoImportLog`.

```json
{
  "version": 1,
  "lastImportDate": "2026-06-22",
  "lastImportAt": "2026-06-22T09:15:04-04:00"
}
```

Lifecycle: created lazily on the first successful auto-import. Missing file is treated as "never run" (import will fire on the next trigger). `lastImportAt` is diagnostic only — today-vs-yesterday gating uses `lastImportDate`.

### `capture-draft.json`

In-flight quick-capture note. When the user opens the quick-capture popup and starts typing, the frontend auto-saves the current title/body/labels here on a ~1.5s debounce so the draft survives a quit, crash, or accidental hide. On a successful Submit the file is deleted (the draft is now a real Note in the weekly file); on load, an empty draft (no title, no body, no labels) is treated as "nothing to restore" so blank fields don't repopulate.

Rust type is `CaptureDraft` in `notes.rs`; the read/write/delete commands (`load_capture_draft`, `save_capture_draft`, `clear_capture_draft`) live in `commands.rs` and gate on `CAPTURE_DRAFT_FILE`.

```json
{
  "title": "Working on the journal app",
  "body": "Started planning the new tool.",
  "labels": ["journal-app", "project"]
}
```

Lifecycle: created on first debounced save while the popup is open. `save_capture_draft` deletes the file (rather than writing empty bytes) when the draft normalizes to empty, so `.metadata/` stays clean in the no-draft case. Cleared on Submit via `clear_capture_draft` (idempotent — a missing file is fine). No `version` field: the shape is small enough that additive changes ride on `#[serde(default)]` and there's nothing to migrate on read.

## Journal-level settings

`<root>/.metadata/settings.json` — per-journal state. Schema evolves; every field uses `#[serde(default)]` on the Rust side, so older files load cleanly when new fields are added.

```json
{
  "version": 1,
  "userName": "Chris Carpenter",
  "userEmail": "chris.carpenter@prodigygame.com",
  "bambooTitle": "QA Analyst",
  "jiraProjectKeys": ["MAGE", "LIVE"],
  "managerName": "Alex",
  "managerEmail": "alex@prodigygame.com",
  "reminder": {
    "enabled": true,
    "daysOfWeek": [4],
    "hour": 16,
    "minute": 0
  },
  "mailSendMode": "Gmail",
  "mailBodyFormat": "CleanText",
  "mailBodyDelivery": "Prefilled",
  "mailNativeHtml": false,
  "mailOutlookFlavor": "Business",
  "colorfulLabels": false
}
```

Field notes:

- `daysOfWeek`: array of weekday indices (0 = Monday … 6 = Sunday, ISO convention). Phase 2.7 widened this from a single `dayOfWeek` u8 to a Vec; a serde back-compat shim accepts the legacy single-value form.
- `mailSendMode`: `"Gmail"` (default) | `"NativeMail"` | `"Outlook"` — the Send-to-manager dispatch path.
- `mailBodyFormat`: `"CleanText"` | `"MarkdownSource"` — controls plaintext flavor (ignored when `mailNativeHtml` is true and the mode is NativeMail).
- `mailBodyDelivery`: `"Prefilled"` (URL/AppleScript carries the body) | `"ClipboardPaste"` (compose opens empty, rich HTML written to clipboard for Cmd+V).
- `mailNativeHtml`: Native Mac Mail only — emits a multipart `.eml` with styled HTML.
- `mailOutlookFlavor`: `"Business"` (outlook.office.com) | `"Personal"` (outlook.live.com).
- `colorfulLabels`: Phase 2.8b toggle — when true, chips render with per-label generated/persisted color.

## App-level settings

`~/Library/Application Support/com.prodigygame.captainslog/app-settings.json` — settings that aren't tied to a specific journal (theme, journal root pointer, window state).

Key fields:

- `theme`: `"Light"` | `"Dark"` | `"Custom"` (default Dark)
- `customTheme`: when `theme` is Custom, the 12 user-edited primaries that derive the full token set (see Phase 2.8 in ROADMAP.md). Survives a Dark/Light flip so the user can come back to Custom from the tray-menu escape hatch.
- `journalRoot`: absolute path to the active journal directory.
- `firstRunComplete`: bool — gates the onboarding wizard.

## Markdown flavor

We target CommonMark plus a few extensions:

- YAML frontmatter
- Standard inline formatting (`**bold**`, `*italic*`, `` `code` ``)
- Lists, links, headings, code blocks
- Inline `#hashtag` syntax for labels (custom — parsed by us, not standard markdown)

Avoid GitHub-Flavored extensions (task lists, tables, strikethrough) unless we explicitly add support, since they complicate parsing and aren't needed for v1.
