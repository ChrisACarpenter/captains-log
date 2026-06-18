# Label System

How labels work end-to-end.

## Two input paths

Labels can be added in two places. Both feed the same index.

### 1. Dedicated Labels field

Per-Note, JIRA-style:

- Type to filter existing labels
- Dropdown shows matches, sorted by recent + frequent
- Press Enter or click to pick
- Type a new label and Enter to create
- Click `x` on a chip to remove
- The user never types `#` — the chip renders with `#` prefix for visual style

### 2. Inline `#hashtags` in body text

While writing the Note body:

- Type `#` followed by a letter → autocomplete dropdown appears
- Same source as the field's autocomplete
- Picking inserts `#labelname` into the prose
- New labels just typed inline are created on save

## Parsing rules

### Labels field input → markdown

The chips in the field write out to a `**Labels:**` line directly under the Note heading:

```markdown
### 2026-06-18 14:23 — Working on the journal app
**Labels:** #journal-app #project

Body text here...
```

### Body inline labels

Labels in the body stay where they were typed:

```markdown
### 2026-06-18 14:23 — Working on the journal app
**Labels:** #journal-app

Started planning. The #label syntax was tricky.
```

Total label set for this Note: `{journal-app, label}`.

### Regex patterns

- Labels line: `/^\*\*Labels:\*\*\s+(.+)$/m`
- Tokens within the labels line: `/#([\w-]+)/g`
- Inline body labels: `/(?:^|\s)#([\w-]+)\b/g` — only match `#` after whitespace or at start of line, so URLs and other contexts don't false-positive

Allowed characters in a label name: `[\w-]+` (letters, digits, underscores, hyphens). No spaces.

## Markdown heading conflict

`#label` is not a valid CommonMark heading because headings require a space after the `#`. The editor reinforces the distinction:

- Typing `#` + space at start of a line → markdown heading mode
- Typing `#` + letter → label autocomplete

No raw-file ambiguity, no UX confusion.

## Index file

`journals/.metadata/labels.json` is the source of truth for autocomplete.

```json
{
  "version": 1,
  "labels": [
    {
      "name": "release",
      "count": 47,
      "first_used": "2026-01-12",
      "last_used": "2026-06-18"
    }
  ]
}
```

### Update rules

- **On Note save:** parse all labels in the Note, increment counts in the index, update `last_used`, create new label entries if needed.
- **On Note edit (label removed):** decrement counts; if count reaches zero, leave the entry but flag for cleanup.
- **On Note delete:** same as edit (decrement).
- **Periodic cleanup:** entries with `count = 0` and `last_used` older than a configurable threshold (default: 90 days) get removed.

### Rebuild

If `labels.json` is missing or corrupted, the app rebuilds it by scanning every weekly file. This happens automatically on first run after a fresh install, and can be triggered manually from Settings.

## Autocomplete behavior

When the user types in either input location:

1. Filter the index to labels matching the typed prefix (case-insensitive)
2. Sort by `last_used` desc, then `count` desc, then alphabetical
3. Show top 10 in the dropdown
4. If the typed string doesn't match any existing label, show a "Create new label: `<name>`" option at the bottom

## Bulk label management (Phase 2 or later)

A "Manage Labels" view in Settings shows:

- All labels with count + first/last used dates
- **Rename:** change a label everywhere it's referenced (updates all weekly files + the index)
- **Merge:** combine two labels into one (e.g., `release` + `releases` → `release`)
- **Delete:** remove from all files + the index

Deferred because the JIRA-style autocomplete keeps the list manageable without explicit cleanup most of the time.
