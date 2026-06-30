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
- Inline body labels: `#` token preceded by a boundary character — start of line, whitespace, OR an opening bracket / brace / parenthesis / comma (`(`, `[`, `{`, `,`). The Rust parser (`labels.rs::extract_inline_labels`) uses an explicit boundary check rather than a single regex, so prose like `(see #release-notes)` correctly picks up `release-notes` as a label while URLs (`https://example.com/#section`) don't false-positive.

Allowed characters in a label name: `[\w-]+` (letters, digits, underscores, hyphens). No spaces.

## Markdown heading conflict

`#label` is not a valid CommonMark heading because headings require a space after the `#`. The editor reinforces the distinction:

- Typing `#` + space at start of a line → markdown heading mode
- Typing `#` + letter → label autocomplete

No raw-file ambiguity, no UX confusion.

## Index file

`<root>/.metadata/labels.json` is the source of truth for autocomplete. JSON keys are camelCase (the Rust struct uses `#[serde(rename_all = "camelCase")]`); snake-case keys from older files are accepted as a back-compat alias on read.

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
    }
  ]
}
```

The optional `color` field (Phase 2.8b) is an explicit per-label hex override; absent or `null` means the chip color is derived at render time from the label name + active theme via `generateLabelColor()`.

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

## Per-label management (shipped)

Settings → Labels lists every label with count + first/last used dates. Clicking a row opens the [LabelDetailsModal](../app/src/lib/LabelDetailsModal.svelte) (Phase 2.8b) with:

- **Color override** — when Colorful Labels is on, override the auto-generated hue with a hex value, or **Reset** back to auto
- **Rename** — change the label everywhere it's referenced (updates all weekly files + the index). Confirm via [ConfirmDialog](../app/src/lib/ConfirmDialog.svelte)
- **Delete** — remove the label from every Note and Weekly Summary's labels list (inline `#hashtag` text in note bodies is left alone)

## Bulk label management (Phase 3a — planned, not yet shipped)

The library viewer that lets you operate on many labels at once — browse / filter, drill into Notes that use a label, and bulk **rename / merge / delete** with atomic writes. Builds on the per-label plumbing above; Phase 3a is next on the roadmap.
