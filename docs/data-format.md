# Data Format Spec

How Notes, Weekly Summaries, and metadata are structured on disk.

## File location

- Journal data root: user-configurable, default `~/Documents/CaptainsLog/`. Subsequent paths in this doc are relative to that root.
- Weekly files: `<root>/YYYY/YYYY-Www.md` (e.g. `~/Documents/CaptainsLog/2026/2026-W25.md`)
- Label index: `<root>/.metadata/labels.json`
- Journal-level settings: `<root>/.metadata/settings.json` (manager, mail, reminder, etc.)
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
```

- Always at the top of the file, above Weekly Notes
- All four `###` headings present even if empty (consistency)
- Bodies are free markdown; inline labels in any of them count

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
