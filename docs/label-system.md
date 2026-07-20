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

- **On Note save:** parse all labels in the Note, increment counts in the index, extend `last_used` / `first_used` to cover the Note's date, create new label entries if needed. `LabelIndex::touch()` is monotonic — it only increments.
- **On explicit label delete** (Settings > Labels > Delete Selected, or the `delete_label_cascade` command): the entry is removed from `labels.json` outright, and every occurrence of `#name` in explicit-labels sites (Note `**Labels:**` lines + Weekly Summary `### Labels` subsections) across all weekly files is stripped. Inline `#hashtag` prose is left alone.
- **Decrement-on-edit, note deletion, and periodic cleanup:** not currently supported. There is no decrement path, no zero-count sweep, and note deletion isn't a wired-up operation. Entries only leave the index via explicit delete or a full rebuild.

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
- **Referenced In** — a drill-down list of every site the label appears in, backed by the [`get_notes_for_label`](../app/src-tauri/src/commands.rs) Rust command. One row per label site with a Summary vs Note kind badge, the week label (`YYYY-Wnn`), and the Note's timestamp/title; sorted newest-first and capped at 50 rows with a TipBubble explaining truncation. Clicking a row closes the modal and `goto('/journal?year=Y&week=W')`, where the sidebar auto-expands the year and selects the week

## Bulk label management (Phase 3a Slice 2 — shipped)

Settings > Labels has a multi-select toolbar layered on top of the per-label list:

- Row checkboxes plus a "select all visible" checkbox in the header
- **Delete Selected** — runs `delete_label_cascade` for each selected label
- **Merge Into** — pick a canonical label from the selection, then run `rename_label` from every other selected label into that canonical name (labels.json entries merge, weekly files are rewritten)
- Execution is sequential and continues on failure; when the pass finishes a success / failure banner appears above the list summarising files modified, occurrences touched, and any per-label errors

