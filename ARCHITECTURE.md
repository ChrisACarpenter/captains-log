# Captain's Log — Architecture

Captain's Log is a personal weekly work journal for Prodigy QA, designed to make Lattice self-reviews and performance-review prep painless by capturing what you actually did, week by week, while it's still fresh. This document describes the architecture of the v1 desktop app — the three writing surfaces, the on-disk storage model, the CodeMirror-based live-preview layer, the toolbar/command system, the event-routing model behind interactive widgets, and the limitations the design currently accepts. It's written for future contributors (and future-me) who need to understand why things are shaped the way they are before changing them.

## Table of Contents

- [Overview](#overview)
- [The Three Surfaces](#the-three-surfaces)
- [Storage Model](#storage-model)
- [Live-Preview Architecture](#live-preview-architecture)
- [Toolbar + Commands](#toolbar--commands)
- [Routing Doc Changes](#routing-doc-changes)
- [Known Limitations + Decisions Deferred](#known-limitations--decisions-deferred)

## Overview

Captain's Log is a Tauri 2.0 desktop app (Rust backend, Svelte 5 frontend) that runs locally on macOS. Everything is stored as plain markdown on disk at `journals/YYYY/YYYY-Www.md`, and the app is deliberately single-user and offline-first. There's no server, no account, no sync — your journal is a folder of `.md` files you own. The goal is to capture work in a format the user can later feed an LLM at review time, so six months of accumulated weekly entries become the raw material for a Lattice self-review.

### The shared editor

All three surfaces render their prose through one component — `app/src/lib/MarkdownEditor.svelte`, a ~300-line wrapper around CodeMirror 6. CM6 was chosen because its buffer **is** the markdown file byte-for-byte; WYSIWYG markdown editors (Milkdown, TipTap, Lexical) mutate the source on save in ways that break the future LLM-bundle workflow.

Two surface-tunable behaviors are opt-in props rather than global modes:

- **`livePreview`** — when true, the `livePreview` extension installs atomic `Decoration.replace` ranges that hide markdown markers (`**`, `~~`, `#`, `-`, `>`, `[…](…)`, etc.) so the user sees rendered rich text while the on-disk bytes stay canonical markdown. `/capture` and `/summary` opt in; `/journal` opts in only when its Preview/Source toggle is on Preview.
- **`showToolbar`** — when true, a `MarkdownToolbar` strip renders above the editor with Heading / Bold / Italic / Strikethrough / lists / quote / link / code / Help buttons backed by the same `markdown-formatting.ts` commands as the `Cmd+B/I/K/E` keymap. `/capture` and `/summary` show it; `/journal` shows it only in Preview mode and hides it in Source mode.

Beyond those two switches, every surface gets the same package for free: GFM parsing, clickable Cmd-click links (`markdown-links.ts` + Tauri's `opener`), native WebKit spell-check via `EditorView.contentAttributes`, inline date-chip widgets, and a per-instance `--md-*` CSS variable set for fonts, padding, and minimum height.

## The Three Surfaces

The app exposes three distinct routes, each tuned for a different mode of writing.

### `/capture` — the menu-bar quick-capture popup

A 460×460 popup window (window label `capture`, hidden by default) summoned from the macOS menu-bar icon. It's optimized for the in-the-moment "I just did a thing, log it before I forget" flow — two clicks from tray to submitted note. The form is a title input, a body `MarkdownEditor`, and a `LabelInput` chip strip. Submitting calls the `create_note` Tauri command, which appends a timestamped Note to the current week's `### Weekly Notes` section, creating the weekly file with an empty Summary scaffold if it doesn't exist yet. Drafts auto-save to `.metadata/capture-draft.json` on a 1.5s debounce, so closing the popup mid-thought is non-destructive — the same content reappears next open. `Cmd+Enter` submits; `Esc` hides without discarding.

### `/summary` — the structured weekly form

A long-form route holding the four-field Lattice-template summary for the current ISO week: **Key accomplishments**, **Plans and priorities for next week**, **Challenges or roadblocks**, and **Anything else on your mind**, plus a `Labels` chip field. Each field is its own `MarkdownEditor` instance with a `--md-min-height` of 112px and `resize: vertical` so the user can drag a section taller. Edits auto-save to the same `journals/YYYY/YYYY-Www.md` file on a 1.5s debounce via `update_weekly_summary`; `Cmd+S` / `Cmd+Enter` force-saves. A **Send to manager** button composes a `mailto:` URL (or falls back to an `.eml` file when the URL would exceed ~1800 bytes), opens the user's default mail app via `tauri-plugin-opener`, and stamps `.metadata/sent-log.json` with a SHA-256 content hash so the UI can detect "edited since last send" and re-enable the button with a `Send updated version` label.

### `/journal` — the past-weeks browser

A two-pane navigator for everything already on disk. The left sidebar is a collapsible year/week tree (`list_years` + `list_weeks` commands, newest-first, current year auto-expanded, current week marked with an orange dot). The right pane mounts a `MarkdownEditor` against the selected week's full raw markdown file via `read_week` / `write_week`, with the same 1.5s debounced auto-save as `/summary`. A segmented **Preview / Source** toggle in the header (and `Cmd+Shift+S`) switches the editor between live-preview rich-text and raw-markdown source; the choice persists across launches in `localStorage` under `captainslog:journalViewMode`. Switching weeks flushes any pending edit to the previously-selected week before loading the new one, so debounce gaps can't strand writes against the wrong file.

## Storage Model

### File layout

Journal data lives as plain markdown files on the local disk, one file per ISO 8601 week:

```text
{journalRoot}/
├── .metadata/
│   ├── labels.json
│   ├── settings.json
│   ├── sent-log.json
│   └── capture-draft.json
├── 2026/
│   ├── 2026-W01.md
│   ├── 2026-W24.md
│   └── ...
└── 2027/
    └── ...
```

`journalRoot` is configurable from Settings and defaults to `${HOME}/Documents/CaptainsLog`. The root itself is not created at startup — year directories are created lazily on first write via `tokio::fs::create_dir_all` inside `LocalFilesystem::write_week`. Week numbers use ISO 8601 (1–53), so a December 2025 date can legitimately land in `2026/2026-W01.md` if it falls in ISO week 1 of 2026.

### File format

Every weekly file is YAML frontmatter, an `h1` week title, a `## Weekly Summary` section with five `###` subsections in a fixed canonical order, and a `## Weekly Notes` section that timestamped `###` notes get appended into. The scaffold for a brand-new file (produced by `weekly_file_scaffold`):

```markdown
---
period: 2026-W26
start: 2026-06-22
end: 2026-06-28
labels: []
last_modified: 2026-06-22T09:00:00-04:00
---

# Week of June 22 - June 28, 2026

## Weekly Summary
*Last updated: never*

### Key accomplishments

### Plans and priorities for next week

### Challenges or roadblocks

### Anything else on your mind

### Labels

## Weekly Notes
```

Notes appended by `/capture` follow the pattern:

```markdown
### 2026-06-18 14:23 — Working on the journal app
**Labels:** #journal-app #project

Started planning the new journal tool.
```

The `**Labels:**` line is omitted when a note has no labels; the ` — Title` suffix is omitted when the title is empty after trimming.

### Why plain markdown

Captain's Log treats the file tree as the source of truth, not a database. Files are human-readable, `cat`-able, `grep`-able, can be committed to git, synced through iCloud or Dropbox, opened in any text editor, and pasted directly into Lattice or a Slack DM. The frontmatter is standard YAML, so Obsidian, Bear, and any static-site generator can ingest it without a custom adapter. If Captain's Log disappears tomorrow, the data is fine — every weekly file stands alone and reads as a normal markdown journal entry.

### Read / write commands

The backend exposes raw and structured surfaces over the same files:

- `read_week(year, week) -> Option<String>` returns the file's raw markdown, or `None` if it doesn't exist. `/journal` uses this for the raw-markdown editor.
- `write_week(year, week, content)` overwrites the entire file with the supplied text. `/journal` saves through this — whatever the user typed lands on disk verbatim.
- `update_weekly_summary(input)` is what `/summary` calls. It reads the existing file, parses out the structured `WeeklySummary` fields, mutates them from the form payload, stamps `last_updated` server-side from `Local::now()`, then splices the freshly rendered summary section back into the file via `replace_weekly_summary_in_file` — preserving frontmatter, the week heading, and every Weekly Note below.

`create_note` is the third write path: it reads the file (or scaffolds a new one), appends the rendered note, and writes it back.

### Trim behavior

Before serialization, each summary field passes through `trim_body`, a helper that calls `s.trim_end()`. This strips trailing whitespace and trailing blank lines but **preserves internal blank lines** — paragraph breaks inside `Key accomplishments` survive a round-trip through parse → mutate → render. Labels rendered into `### Labels` are normalized separately: leading `#` characters are stripped (`trim_start_matches('#')`) so a chip-input that sends `"##planning"` still renders as `#planning`.

### Parser tolerance

`parse_weekly_summary` and its helper `extract_subsection` are deliberately forgiving. A missing `## Weekly Summary` header yields a default `WeeklySummary` with empty strings — no error. A missing `### Key accomplishments` subheading returns an empty string for that field rather than throwing. Out-of-order subsections still parse correctly as long as the heading text matches exactly, because `extract_subsection` finds each heading by string search and bounds the body at the next `### ` or `## ` heading regardless of which comes first. The `*Last updated: never*` sentinel is recognized and mapped to `None` rather than literal text. This forgiveness matters because users will edit these files in external editors, and a hand-edit that drops a blank line or reorders a heading must not corrupt the file on next save.

## Live-Preview Architecture

The live-preview layer is what turns the raw-markdown buffer into a rendered rich-text experience without ever mutating the file on disk — and it is the load-bearing decision behind why CM6 was chosen over Milkdown / TipTap / Lexical.

### Rich-text on top, canonical Markdown underneath

The doc on disk is byte-for-byte CommonMark + GFM. The user sees rendered output. Markdown markers (`**`, `_`, `~~`, `` ` ``, `#`, `>`, `[ ]`, `[label](url)`) are visually hidden by `Decoration.replace` collapsing their ranges to zero visible width. The underlying text is never rewritten; future LLM hand-off receives the same canonical source the user typed, and existing weekly files round-trip through GitHub / Obsidian / Slack as plain prose. This is the Confluence / Slack / Typora model: the visible affordance is the formatted result, but the persistence layer is unambiguous markdown.

### Decoration types

Three CM6 decoration kinds, each pulling its own weight:

```
Decoration.replace  – hides a range; optionally renders a widget in its place
                      (marker hiding, bullet glyph, task checkbox, date chip)
Decoration.mark     – adds a CSS class to a range
                      (inline-code chip background, strike-through on done tasks)
Decoration.line     – adds a CSS class to a whole line
                      (blockquote left bar, fenced-code line background)
```

No `Decoration.mark` is used for content styling like bold or italic — `defaultHighlightStyle` already styles `tags.strong`, `tags.emphasis`, `tags.heading`, etc. The live-preview extension only hides markers and frames code/quote blocks; the per-tag styling layer is independent.

### Atomic ranges

Every hidden `Decoration.replace` is also published via `EditorView.atomicRanges`. Without that registration, arrow keys would step through invisible character positions one-by-one — right-arrow at the end of `**bold**` would appear to "do nothing" twice as the cursor walked the two hidden asterisks. With it, cursor commands treat each hidden span as a single skip-step: arrow keys jump past, selection drags hop in units, backspace removes the whole marker chunk. The atomic-range facet uses a function (not a static set) so it always reads the plugin's current `decorations` — atomic behavior stays in sync with reveals.

One important seam: atomic-range rules apply only to user-driven cursor commands. A programmatic `view.dispatch({ selection })` (e.g. the toolbar's Cmd+K placing the cursor inside the URL placeholder) is preserved verbatim. That asymmetry is precisely why reveal-on-active-Link exists.

### Reveal-on-active-Link

When any selection endpoint lands inside a `Link` or `Image` node, every marker inside that node stays visible. This is narrowly scoped to Link / Image — not all marker types — because Link is the only construct where the marker text itself carries content (the URL) the user has to author. Bold and italic markers are content-free; hiding them everywhere is safe. The reveal handles three concrete cases: editing the URL of an existing link, Cmd+K with no selection (`[](url)`), and Cmd+K wrapping a selection (`[selected](url)` with the cursor parked inside the parens). Each one would otherwise leave the user with zero visible affordance to type into.

### Composition order

The build function collects every decoration into a flat `Range[]`, sorts it, then feeds a `RangeSetBuilder`. The sort key is `(from asc, deco.startSide asc, length desc)`. The `startSide` ordering is mandatory — `RangeSetBuilder` throws if same-`from` ranges are added in the wrong order, and CodeMirror catches that throw at the facet boundary and silently renders zero decorations for the entire view (the symptom: monospace font still applies because HighlightStyle survives, but no marker-hiding and no chip class). Reference values from `@codemirror/view`:

| Decoration kind          | `startSide`     |
| ------------------------ | --------------: |
| `Decoration.line`        |    -200,000,000 |
| `Decoration.replace`     |     499,999,999 |
| `Decoration.mark`        |     500,000,000 |

Sorting on the actual `deco.startSide` (instead of a hand-rolled rank constant) means new decoration types can be added without re-deriving the order; the value lives on the `RangeValue` itself.

### Per-surface toggle

`MarkdownEditor` exposes a `livePreview: boolean` prop and conditionally includes `livePreviewExt()` in the extension array — `livePreview ? livePreviewExt() : []`. `/capture` and `/summary` default it on. `/journal` exposes a runtime Preview/Source toggle: Source mode drops the extension entirely and reverts to raw-markdown editing for power users. This is also why heading sizing lives inside the live-preview extension's own `HighlightStyle` rather than the global one — Source mode stays free of presentation overrides, and `/journal`'s monospace surface keeps the visual it had before the rich-text experience landed.

## Toolbar + Commands

The formatting surface is split into two layers with a strict single-source-of-truth contract.

### Two-layer split

`markdown-formatting.ts` exports pure `Command` functions of shape `(view: EditorView) => boolean`. They never touch the DOM directly — they call `view.dispatch(...)` and return `true` to consume the keystroke. `MarkdownToolbar.svelte` is a presentational button strip: its `onclick` handlers call those same commands, and `MarkdownEditor.svelte` prepends them into the CodeMirror keymap:

```ts
keymap.of([
  ...markdownFormattingKeymap,   // Cmd+B/I/E/K, Cmd+Shift+7/8/9/L/X, Cmd+Alt+0, Cmd+;
  ...defaultKeymap,
  ...historyKeymap,
  indentWithTab,
]);
```

One implementation per format backs both the button and the shortcut. A tweak to "what counts as bold here?" cannot drift between mouse and keyboard paths because there is no second copy to drift against.

### Wrap commands (bold / italic / strike / code)

All four route through `toggleWrap(view, mark)` with their marker baked in (`**`, `*`, `~~`, `` ` ``). Three behaviors fall out of one function:

- **Smart selection.** If `from` sits at `line.from` and the line begins with a block prefix — `#{1,6} `, `- `, `1. `, `> `, `- [ ] ` — `from` advances past that prefix before wrapping. Without this, wrapping a heading row produces `**# heading**`: Lezer emits `HeaderMark` only when `#` is at line start, so the `# ` becomes literal text and the heading silently downgrades. Same shape kills list and quote nodes.
- **Empty-selection no-op.** With live-preview hiding `Emphasis`/`Strikethrough`/`InlineCode` markers, an empty `**…**` would have zero visible feedback — and because CommonMark won't emit those nodes without inline content, the markers wouldn't even hide, so four asterisks would pop into view on the next cursor move. Returns `true` so the keystroke is still consumed.
- **Unwrap detection.** If the same marker brackets the selection (outer match) or starts and ends the selection itself (inclusive match), the wrap is stripped instead of doubled.

`toggleInlineCode` has one extra branch on top: a multi-line selection produces a fenced block, and a cursor already inside a `FencedCode` unwraps the surrounding fence.

### Line-prefix commands

Heading cycle, bullet / numbered / task lists, and blockquote share `transformLines(view, fn)`. It iterates every line intersecting the selection, calls `fn(text, index)`, and dispatches a single multi-change transaction. The heading cycle is `none → H1 → H2 → H3 → none` — a narrow `^#{1,3} ` strip regex deliberately leaves H4–H6 alone so a hand-authored level the toolbar can't reach isn't silently destroyed.

`isFenceBoundaryLine` skips the opening and closing ` ``` ` of any `FencedCode`. Stamping `> ` / `- ` / `# ` onto a fence marker decays the block into a blockquote of literal backticks; once that happens the cursor-skip filters elsewhere stop firing and the user is stranded with their code rendered as prose. Body lines inside a fence are left alone — literal text inside a fence is harmless.

### List commands special-case

Before falling through to `transformLines`, every list command tries `applyListMarkerToCurrentLine`. That fast path fires when the selection is empty AND the current line is either fully blank or matches `ONLY_MARKER_RE` (a stale `2. ` or `- ` from auto-continuation). It takes a direct dispatch with the cursor explicitly placed *after* the inserted marker — `transformLines`'s default mapping leaves the cursor on the left of `- `, and the user's first keystroke breaks the parse.

The blank-line separator is the load-bearing piece. If the line above is non-blank AND a different list family (`markerFamilyRegex`), a `\n` is prepended before the marker. Without it, Lezer:

- absorbs an empty `- ` directly beneath an `OrderedList` item as lazy paragraph continuation — no `ListMark` node, no bullet widget.
- reclassifies an empty `- ` beneath a plain paragraph as a Setext-heading underline, silently promoting the paragraph above to an H2.

Both failure modes swallow the marker invisibly, which is why the separator is unconditional rather than best-effort.

### Tab indent inside lists

`indentListItem` (bound to plain `Tab` ahead of `indentWithTab`) only engages when the cursor is inside a `ListItem` — detected via the Lezer tree, with `LIST_LINE_RE` as a parse-lag fallback for freshly-inserted empty bullets. It walks backward to find the would-be parent list line and caps indentation at `parent_content_offset + 3`, CommonMark's exact sub-item range. Past that boundary the line gets reclassified as continuation text and the bullet widget vanishes.

A lone top-level item has no parent context; the cap collapses to `currentIndent` and Tab becomes a no-op (returning `true` so default `indentWithTab` doesn't step in and produce a 4-space indented-code-block instead). `outdentListItem` mirrors this on `Shift+Tab`, stripping two leading spaces.

### Active-state detection

`detectActiveFormats(view)` walks the syntax tree upward from `selection.head` and collects every `ActiveFormat` whose Lezer node is an ancestor. Two corrections are baked in:

- **Right-edge skip.** `resolveInner(head, -1)` lands on a node *ending* at the cursor, so when the cursor sits just after a closing `**` of bold, the parent walk would still surface `StrongEmphasis`. If the resolved node is an inline mark (`EmphasisMark`, `StrikethroughMark`, `CodeMark`, `LinkMark`) and is pinned to the wrap's `from` or `to`, the walk starts at `parent.parent` instead. `**bold**|` correctly reads as not-bold.
- **Innermost list wins.** A `Task` lives structurally inside a `BulletList`; the toolbar shouldn't light both. Only the innermost list-family ancestor is recorded.

`MarkdownToolbar` reads the result as a `$derived` Set keyed off an `updateTick` counter. `MarkdownEditor` bumps the counter inside an `updateListener` on every `selectionSet || docChanged`:

```ts
EditorView.updateListener.of((update) => {
  if (update.docChanged) onChange(update.state.doc.toString());
  if (update.selectionSet || update.docChanged) updateTick++;
}),
```

Each button calls `btnClass(format)` which emits `is-active` plus `aria-pressed={true|false}` when the format is in the set — continuous visual feedback that mirrors Slack's pressed-state convention.

### Date chip + `insertCurrentDate`

`Cmd+;` runs `insertCurrentDate`, which writes a local-timezone ISO `YYYY-MM-DD` at the cursor (replacing any selection) and lands the cursor at the end of the inserted text. Local time, not UTC — a late-night entry doesn't get stamped with tomorrow's date.

The date *chip* is a separate concern registered as part of `livePreview()`. It scans the visible viewport for `\b\d{4}-\d{2}-\d{2}\b` outside code spans and swaps each match for a clickable pill widget. The commit path is covered in the next section.

## Routing Doc Changes

The date-chip and task-checkbox widgets are interactive — clicking a chip opens a date picker; clicking a checkbox toggles `[ ]` ↔ `[x]`. Both ultimately need to dispatch a CodeMirror transaction against the right `EditorView`. That sounds trivial until you remember `/summary` mounts **four `MarkdownEditor` instances at once** (one per field). A click on a chip in editor B has to land its commit in editor B's doc — not editor A's.

### The earlier broken approach

The first design used a global routing layer:

```ts
// Earlier (broken) approach — sketched
const activeViews = new Set<EditorView>();
window.addEventListener('captainslog:date-chip-commit', (e) => {
  const { from, to, iso } = e.detail;
  for (const view of activeViews) {
    if (isValidIsoDateRange(view, from, to)) {
      view.dispatch({ changes: { from, to, insert: iso } });
      return;
    }
  }
});
```

The fallback was "first view whose range still matches an ISO." With four editors that happen to share a layout — same field-prep template, same default date stub — multiple views can satisfy that test simultaneously. Commits silently land in whichever view iteration order picked, not the one the user clicked.

### The lesson

**Route via the originating editor's own `view` reference, not a global lookup.** The DOM event bubbles up to the `MarkdownEditor` container that owns the chip's tree. That container already holds the `view` in scope. Dispatch directly on it — no window event, no view-discovery heuristic.

### The pattern

1. Widget click → `CustomEvent` on the widget DOM:
   ```ts
   // date-chip.ts — inside DateChipWidget.toDOM()
   btn.dispatchEvent(
     new CustomEvent('captainslog:date-chip-click', {
       bubbles: true,
       composed: true,
       detail: { from: this.from, to: this.to, iso: this.iso, anchorEl: btn },
     })
   );
   ```
2. `MarkdownEditor.svelte`'s container listens for that event — only events bubbling through *this* editor's DOM tree reach it.
3. The handler opens the picker with this editor's `view` in scope.
4. The popover's commit callback dispatches directly on `view`:
   ```ts
   function handleDatePickerCommit(newIso: string): void {
     if (!view) return;
     if (!isValidIsoDateRange(view, datePickerFrom, datePickerTo)) return;
     view.dispatch({
       changes: { from: datePickerFrom, to: datePickerTo, insert: newIso },
       userEvent: 'input.type.datechip',
     });
   }
   ```

### Why direct dispatch beats window events

- **Multi-instance correctness.** The bubbling event can only reach the container that contains the originating DOM. There is no ambiguity — the right view is the only candidate.
- **No mount/listener races.** Window listeners require a module-level `activeViews` Set that views register into on mount and remove on destroy. Mount order, hot-reload, and `<svelte:component>` swaps can leave the set transiently empty or stale. Per-view dispatch has no such state.
- **Simpler mental model.** "The thing you clicked talks to the editor it lives inside" needs no diagram.

### Same pattern for the task checkbox

`TaskCheckboxWidget.toDOM(view)` receives the view as an argument (CM6 passes it to `toDOM`) and captures it in the click handler's closure:

```ts
// live-preview.ts — inside TaskCheckboxWidget.toDOM(view)
btn.addEventListener('click', (e) => {
  e.preventDefault();
  e.stopPropagation();
  const currentText = view.state.doc.sliceString(this.from, this.to);
  if (!/^\[[ xX]\]$/.test(currentText)) return;
  const next = this.checked ? '[ ]' : '[x]';
  view.dispatch({
    changes: { from: this.from, to: this.to, insert: next },
    userEvent: 'input.toggle.task',
  });
});
```

The checkbox never even needs the DOM-event hop — there's no popover layer to mediate. Same principle, shorter path.

### Sibling lesson: widget `eq()` must include positions

CM6 reuses widget DOM across transactions whenever `eq()` says "same widget." If `eq()` compares only content (the ISO string, the checked flag), an upstream insertion that shifts every chip's source range will leave the **DOM reused** — and the old `from`/`to` baked into the click handler's closure. Picking a new date then commits to stale coordinates that no longer point at the chip's range. Same trap for the checkbox.

Both widgets defend against this by including `from` and `to` in `eq()`:

```ts
// DateChipWidget
eq(other: WidgetType): boolean {
  return other instanceof DateChipWidget
    && other.iso === this.iso
    && other.from === this.from
    && other.to === this.to;
}
```

Any range shift forces a DOM rebuild, which is cheap (one button, one inline SVG) compared to the data-loss risk of dispatching to wrong offsets.

## Known Limitations + Decisions Deferred

This section catalogs limitations that exist by deliberate scope choice in v1, along with the reasoning and what a future fix would entail. Nothing here is a bug — these are tradeoffs the architecture currently makes.

### Date chip false-positives in prose

The date-chip ViewPlugin matches any `\b\d{4}-\d{2}-\d{2}\b` substring in the visible viewport and rewrites it as a clickable pill. It already skips matches inside `InlineCode` and `FencedCode` syntax nodes, so error logs and version strings pasted into a fenced block stay raw — but prose containing ISO-shaped tokens (a build tag, a serial number, the literal string "1999-12-31" written in a non-date context) still chips. There is no inline opt-out for prose dates the user wants to keep as plain text. A future fix could honor a leading backslash (`\2026-06-25` stays literal) or a configurable allowlist of contexts, but no escape syntax was added in v1.

### List/quote widgets don't reveal on the active line

Bullet, numbered, task, and quote markers are replaced with widgets at all times — including the line the cursor is on. To edit the marker character itself (turn `-` into `+`, or remove a `[ ]`), the user has to flip to Source mode. Typora and Obsidian reveal markers on the active line for exactly this case. Reveal-on-active is implemented for `Link` / `Image` (the marker carries the URL the user needs to author), but not for line-prefix markers, because doing so cleanly requires a per-line "selection touches this line" check in the decoration builder and re-running it on every selection change. Defensible to defer — toolbar buttons and keyboard shortcuts handle the common marker transforms — but a known sharp edge.

### Backspace at the start of a content-bearing body line inside a fenced block

The fence-aware backspace handler covers the "empty body line, cursor at column 0" case (it deletes the whole fence box). It does not cover the case where the body line has content and the user backspaces at column 0 — the default CodeMirror behavior merges that body line into the opening fence line, which breaks the Lezer parse: the FencedCode node decays into a paragraph and the live-preview decorations evaporate mid-edit. The user can recover by undoing, but the failure mode is jarring. A full fix would extend the handler to detect "previous line is the opening fence" regardless of current-line content and either no-op or insert a newline-preserving edit.

### Multi-cursor unsupported by most widget commands

The toolbar commands in `markdown-formatting.ts` mostly bail when `state.selection.ranges.length > 1` — they read `selection.main` and operate on a single range. Properly supporting multi-cursor for bold/italic/list/heading/quote/link would mean iterating ranges in reverse-document order (so earlier edits don't shift later range offsets) and composing one transaction with multiple changes. The math is tractable but tedious, and the target journaling workflow is single-cursor. Accepted as a v1 limitation.

### Setext-style heading parsing disabled (Phase 2.9c)

Setext headings (`===` / `---` underline) used to parse via Lezer and render with heading size. They were a footgun in list-heavy prose — typing a paragraph then a `-` on the next line retroactively re-rendered the paragraph as an H2 underline. Phase 2.9c disabled the SetextHeading extension entirely (`markdown({ extensions: [GFM, { remove: ['SetextHeading'] }] })` in MarkdownEditor.svelte). The journal is ATX-exclusive now; this was always the de-facto convention.

### Tab inside fenced code blocks bypasses the list-indent cap

The Tab indent-cap logic (`maxAllowedIndent`) only fires when the cursor resolves inside a `ListItem` node. Inside a fenced code block, Tab falls through to the default "insert two spaces" behavior with no cap. This is the intended behavior — code blocks shouldn't be constrained by CommonMark's sublist indent rule — but it means a user can type far past the visible closing fence column. They break out by typing past the closing fence line; the parse re-establishes itself naturally.

### Source mode shows everything raw, by design

In Source mode the live-preview extension is stripped, the date-chip extension is stripped, and heading sizing reverts to default. Bullets show as `-`, dates show as `YYYY-MM-DD`, links show as `[text](url)`. This is intentional: Source mode is the canonical text view, the "what would I see if I opened this file in vim?" answer. It is not a bug that widgets disappear there — it is the contract. Documented explicitly so future contributors don't try to "fix" it by leaving certain widgets active in Source.