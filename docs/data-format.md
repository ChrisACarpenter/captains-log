# Data Format Spec

How Notes, Weekly Summaries, and metadata are structured on disk.

## File location

- Journal data root: user-configurable, default `~/Documents/CaptainsLog/`
- Weekly files: `journals/YYYY/YYYY-Www.md` (e.g. `journals/2026/2026-W25.md`)
- Label index: `journals/.metadata/labels.json`
- Settings: `journals/.metadata/settings.json`

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

`journals/.metadata/labels.json`:

```json
{
  "version": 1,
  "labels": [
    {
      "name": "release",
      "count": 47,
      "first_used": "2026-01-12",
      "last_used": "2026-06-18"
    },
    {
      "name": "journal-app",
      "count": 3,
      "first_used": "2026-06-17",
      "last_used": "2026-06-18"
    }
  ]
}
```

Sorted by `last_used` desc, then `count` desc (recent + frequent surfaces first in autocomplete). See [label-system.md](label-system.md) for full update rules.

## Settings

`journals/.metadata/settings.json`:

```json
{
  "version": 1,
  "user": {
    "name": "Chris Carpenter"
  },
  "storage": {
    "backend": "local",
    "path": "/Users/chris.carpenter/Documents/CaptainsLog"
  },
  "reminder": {
    "enabled": true,
    "day_of_week": 4,
    "hour": 16,
    "minute": 0
  }
}
```

`day_of_week`: 0 = Monday … 6 = Sunday (ISO convention).

## Markdown flavor

We target CommonMark plus a few extensions:

- YAML frontmatter
- Standard inline formatting (`**bold**`, `*italic*`, `` `code` ``)
- Lists, links, headings, code blocks
- Inline `#hashtag` syntax for labels (custom — parsed by us, not standard markdown)

Avoid GitHub-Flavored extensions (task lists, tables, strikethrough) unless we explicitly add support, since they complicate parsing and aren't needed for v1.
