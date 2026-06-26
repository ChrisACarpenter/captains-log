/**
 * Markdown formatting commands for CodeMirror 6.
 *
 * Each command takes an `EditorView`, applies a text transformation on the
 * current selection (or line), dispatches the change, and returns true to
 * stop further keymap propagation. The commands back BOTH:
 *
 *   - The toolbar buttons (MarkdownToolbar.svelte's onClick handlers)
 *   - The Mod+B/Mod+I/Mod+K/Mod+E/Mod+Shift+7/Mod+Shift+8 keyboard shortcuts
 *
 * Wrap/unwrap logic lives in ONE place per format so a future tweak to
 * "what counts as bold here?" doesn't drift between mouse and keyboard
 * paths. `Mod-` in the keymap resolves to Cmd on macOS, Ctrl elsewhere.
 *
 * ## Design notes
 *
 *   - Bold uses `**…**`, italic uses `*…*`. Asterisks (not underscores) so
 *     `snake_case_identifiers` don't trigger false toggles.
 *   - Toggle commands DETECT and UNDO an existing wrap if the selection is
 *     already wrapped. So Cmd+B on `**bold**` removes the asterisks.
 *   - Line-prefix commands (lists, quote, heading) operate on the full
 *     line(s) containing the selection, not just the selection text.
 *   - Headings cycle: none → H1 → H2 → H3 → none. One button, one icon.
 *   - Link with no selection puts the cursor INSIDE THE BRACKETS so the
 *     user types the label first, matching journaling prose flow.
 *   - Code is contextual: backticks for single-line, fenced block for
 *     multi-line. Avoids needing a separate "code block" button.
 */
import type { Command, EditorView, KeyBinding } from '@codemirror/view';
import { EditorSelection, type EditorState } from '@codemirror/state';
import { syntaxTree } from '@codemirror/language';
import type { SyntaxNode } from '@lezer/common';

// ---------- helpers ----------

/** Selection range as {from, to} in document UTF-16 units. */
function mainRange(view: EditorView): { from: number; to: number } {
  const { from, to } = view.state.selection.main;
  return { from, to };
}

/**
 * Split a line into `{ indent, content }`. Indent = leading run of spaces
 * and/or tabs (the line's visual indentation); content = everything after.
 * Used by the list commands so that toggling bullet/numbered/task on an
 * indented line preserves the indentation rather than stamping `- ` /
 * `1. ` / `- [ ] ` at column 0.
 */
function splitIndent(line: string): { indent: string; content: string } {
  const m = /^[ \t]*/.exec(line);
  const indent = m ? m[0] : '';
  return { indent, content: line.slice(indent.length) };
}

/**
 * Family of the marker we're about to insert. Used to decide whether the
 * NON-blank line above (if any) would absorb our empty marker via lazy
 * paragraph continuation (the entire `- ` line getting parsed as text
 * inside the prior list item / paragraph / setext heading).
 *
 *   - 'bullet': `- `, `* `, `+ `, `- [ ] ` (task is a kind of bullet)
 *   - 'numbered': `1. `, `2) `, etc.
 *
 * Caller passes the marker string; this returns the family that line
 * above must also be in (no blank line needed) for the parse to come
 * out as a list mark.
 */
function markerFamilyRegex(marker: string): RegExp {
  if (/^\d+[.)] $/.test(marker)) {
    // Numbered family: line above must be `1. ` / `2) ` etc.
    return /^[ \t]*\d+[.)](?:\s|$)/;
  }
  // Bullet family (including task `- [ ] `): line above must be a bullet
  // marker `-`/`*`/`+` regardless of whether it's task syntax.
  return /^[ \t]*[-*+](?:\s|$)/;
}

/**
 * Regex matching a line that contains ONLY a list marker (with optional
 * indent and trailing whitespace), no actual content. Covers all the
 * shapes we need to detect:
 *   - `- ` / `* ` / `+ ` (bullet markers)
 *   - `- [ ] ` / `- [x] ` (task markers)
 *   - `1. ` / `2) ` (numbered markers)
 *   - Plus any whitespace before / after / between
 *
 * Used by applyListMarkerToCurrentLine to detect "user has a stale marker
 * (e.g. auto-continued from Enter on a numbered list) and is now
 * switching list types" — the right move is to REPLACE the line content
 * with the new marker, not append to it.
 */
const ONLY_MARKER_RE = /^[ \t]*(?:-(?:\s+\[(?: |x|X)\])?|\*|\+|\d+[.)])\s+$/;

/**
 * Apply a list marker to the current line. Handles two cases that both
 * need a blank-line separator when the line above is a non-blank line of
 * a different list family:
 *
 *   1. **Empty current line.** User clicked a list button on a blank
 *      line below content. Insert the marker (with optional `\n` prefix
 *      to break out of the prior list / paragraph context).
 *
 *   2. **Only-marker current line.** User has a stale marker on the line
 *      (e.g. `2. ` from auto-continued numbered list, or `- ` they just
 *      typed and abandoned). Bullet/Numbered/Task button should REPLACE
 *      the existing marker with the new one — preserving indent and
 *      adding the blank-line separator when needed.
 *
 * Both cases produce a dispatch with:
 *   - changes: { from: line.from, to: line.to, insert: prefix? + indent + marker }
 *   - selection: anchor placed AFTER the marker so the user's first
 *     keystroke types correctly (without this, default mapping leaves
 *     the cursor at the LEFT and typing breaks the parse).
 *
 * Returns true when it handled the case (caller short-circuits);
 * false otherwise so the multi-line / content-bearing path takes over
 * via transformLines.
 *
 * The blank-line separator is critical. Empirically verified by parsing
 * each case with @lezer/markdown + GFM: an empty `- ` directly below an
 * OrderedList item gets absorbed as lazy paragraph continuation (no
 * ListMark node emitted); the same `- ` below a plain paragraph gets
 * re-classified as a Setext-heading underline (silently turning the
 * paragraph into an H2!). Both silently swallow the marker and the
 * bullet widget never fires.
 */
function applyListMarkerToCurrentLine(
  view: EditorView,
  marker: string
): boolean {
  const sel = view.state.selection.main;
  if (!sel.empty) return false;
  const line = view.state.doc.lineAt(sel.from);

  const isEmpty = line.text === '';
  const isOnlyMarker = ONLY_MARKER_RE.test(line.text);
  if (!isEmpty && !isOnlyMarker) return false;

  // Preserve existing indent (e.g. `  - ` on an indented blank line).
  const indentMatch = /^[ \t]*/.exec(line.text);
  const indent = indentMatch ? indentMatch[0] : '';

  // Decide blank-line separator. Line 1 (top of doc) is trivially safe.
  // Blank line above is safe. Same marker family above is safe (joins
  // the existing list). Anything else needs a `\n` prefix so Lezer
  // starts a fresh list block instead of absorbing this line as
  // continuation / Setext-heading underline.
  let needsBlankLine = false;
  if (line.number > 1) {
    const prevText = view.state.doc.line(line.number - 1).text;
    if (prevText.trim() !== '' && !markerFamilyRegex(marker).test(prevText)) {
      needsBlankLine = true;
    }
  }

  const replacement = indent + marker;
  const insertText = needsBlankLine ? '\n' + replacement : replacement;
  view.dispatch({
    changes: { from: line.from, to: line.to, insert: insertText },
    selection: { anchor: line.from + insertText.length },
    userEvent: 'input.indent.list',
  });
  view.focus();
  return true;
}

/**
 * Strip whatever list-style marker (bullet / numbered / task) the content
 * starts with. Returns the content without that marker. The list-toggle
 * commands use this so that converting `- item` → `1. item` strips the
 * `- ` first instead of producing `1. - item`.
 */
function stripListMarker(content: string): string {
  return content
    .replace(/^- \[[ x]\] /, '')
    .replace(/^- /, '')
    .replace(/^\d+\. /, '');
}

/**
 * True when `lineNo` is the opening or closing fence line of a `FencedCode`
 * block. Used by `transformLines` to refuse to stamp `> ` / `- ` / `1. ` /
 * `# ` prefixes onto the ` ``` ` markers themselves — that would mangle the
 * fence into a quote/list of literal backticks and break the parse (the
 * block decays into prose, our fence cursor-skip filter stops firing, and
 * the user gets stuck with their code now wrapped as a blockquote).
 *
 * Body lines INSIDE a fenced block are not skipped — `> body` / `- body`
 * inside a fence is just literal text and stays parseable.
 */
function isFenceBoundaryLine(state: EditorState, lineNo: number): boolean {
  const line = state.doc.line(lineNo);
  let node: SyntaxNode | null = syntaxTree(state).resolveInner(line.from, 1);
  while (node) {
    if (node.name === 'CodeMark' && node.parent?.name === 'FencedCode') {
      return true;
    }
    node = node.parent;
  }
  return false;
}

/** Replace [from, to) with `insert`, then position the cursor at `cursor`. */
function replaceAndMoveCursor(
  view: EditorView,
  from: number,
  to: number,
  insert: string,
  cursor: number
) {
  view.dispatch({
    changes: { from, to, insert },
    selection: EditorSelection.cursor(cursor),
  });
  view.focus();
}

/** Replace [from, to) with `insert`, select [selectFrom, selectTo) in the new doc. */
function replaceAndSelect(
  view: EditorView,
  from: number,
  to: number,
  insert: string,
  selectFrom: number,
  selectTo: number
) {
  view.dispatch({
    changes: { from, to, insert },
    selection: EditorSelection.range(selectFrom, selectTo),
  });
  view.focus();
}

/** Toggle a paired wrap (e.g. **…** or *…*) around the current selection.
 *
 *  Empty selection: NO-OP. Live-preview hides Emphasis/Strikethrough/Code
 *  markers, so inserting empty `**…**` would leave zero visible feedback.
 *  Worse, CommonMark requires non-empty content for those nodes to parse,
 *  so the markers wouldn't even hide (no Emphasis node emitted) — and on
 *  the next cursor move, four asterisks would suddenly appear out of
 *  nowhere. Returning `true` (handled) so the Cmd+B/I/E/Shift+X keystroke
 *  doesn't fall through to another binding; consistent across all four
 *  wrap toggles. To use these formats, select content first.
 *
 *  Non-empty selection that's already wrapped: unwrap.
 *  Non-empty selection that's not wrapped: wrap. */
function toggleWrap(view: EditorView, mark: string): boolean {
  let { from, to } = mainRange(view);
  const doc = view.state.doc;
  const markLen = mark.length;

  if (from === to) {
    view.focus();
    return true;
  }

  // Smart selection: if `from` is at the start of a line that begins with
  // a line-prefix marker (heading, list, numbered, quote, task), advance
  // `from` past the marker. Otherwise the wrap goes INSIDE the prefix —
  // e.g. `**# heading**` is no longer a heading because the `#` is no
  // longer at line start, and Lezer emits HeaderMark only for line-start
  // `#`. The `# ` would suddenly become visible plain text. Same shape
  // for lists/quotes: `**- item**` strips the ListMark role.
  const startLine = doc.lineAt(from);
  if (from === startLine.from) {
    const prefixMatch = /^(?:#{1,6} |- \[[ x]\] |- |\d+\. |> )/.exec(startLine.text);
    if (prefixMatch) {
      from = Math.min(startLine.from + prefixMatch[0].length, to);
      if (from === to) {
        // Adjustment collapsed the range — nothing to wrap. No-op.
        view.focus();
        return true;
      }
    }
  }

  const before = doc.sliceString(Math.max(0, from - markLen), from);
  const after = doc.sliceString(to, Math.min(doc.length, to + markLen));
  const selected = doc.sliceString(from, to);

  // Outer wrap exists immediately around the selection → unwrap by deleting marks.
  if (before === mark && after === mark) {
    view.dispatch({
      changes: [
        { from: from - markLen, to: from, insert: '' },
        { from: to, to: to + markLen, insert: '' },
      ],
      selection: EditorSelection.range(from - markLen, to - markLen),
    });
    view.focus();
    return true;
  }

  // Selection starts AND ends with the mark (inclusive wrap) → strip both.
  if (
    selected.length >= markLen * 2 &&
    selected.startsWith(mark) &&
    selected.endsWith(mark)
  ) {
    const inner = selected.slice(markLen, selected.length - markLen);
    replaceAndSelect(view, from, to, inner, from, from + inner.length);
    return true;
  }

  // Otherwise wrap.
  const wrapped = mark + selected + mark;
  replaceAndSelect(
    view,
    from,
    to,
    wrapped,
    from + markLen,
    from + markLen + selected.length
  );
  return true;
}

/** Apply a line-prefix transformation across every line that intersects the
 *  current selection. The transform function gets each line's content (no
 *  newline) and returns the rewritten content.
 *
 *  Fence-boundary lines (opening / closing ` ``` ` of a FencedCode) are
 *  SILENTLY SKIPPED — stamping `> ` / `- ` / `# ` onto a fence marker would
 *  break Lezer's parse of the block, decaying the fenced code into a
 *  blockquote of literal backticks and stranding the user with their code
 *  now wrapped as prose. Body lines inside a fence are not skipped (literal
 *  `> body` inside a fence body is just text). */
function transformLines(
  view: EditorView,
  transform: (line: string, index: number) => string
): boolean {
  const state = view.state;
  const { from, to } = mainRange(view);
  const startLine = state.doc.lineAt(from);
  const endLine = state.doc.lineAt(to);

  const changes: { from: number; to: number; insert: string }[] = [];
  let lineIdx = 0;
  for (let lineNo = startLine.number; lineNo <= endLine.number; lineNo++) {
    if (isFenceBoundaryLine(state, lineNo)) {
      lineIdx++;
      continue;
    }
    const line = state.doc.line(lineNo);
    const next = transform(line.text, lineIdx);
    if (next !== line.text) {
      changes.push({ from: line.from, to: line.to, insert: next });
    }
    lineIdx++;
  }

  if (changes.length === 0) {
    view.focus();
    return true;
  }
  view.dispatch({ changes });
  view.focus();
  return true;
}

// ---------- active-state detection ----------

/**
 * Names of toolbar formats. Match the strings passed to `active.has(...)`
 * in MarkdownToolbar.svelte. Each entry corresponds to a toolbar button
 * (or family of buttons) whose visual "pressed" state should track
 * whether that format applies at the current cursor position.
 *
 * `bold` and `italic` are distinct because `**bold**` and `*italic*` are
 * separate Lezer nodes (StrongEmphasis vs Emphasis). Headings collapse
 * H1–H6 into one boolean — the toolbar's heading button is a cycle, not
 * level-specific.
 */
export type ActiveFormat =
  | 'heading'
  | 'bold'
  | 'italic'
  | 'strike'
  | 'bullet'
  | 'numbered'
  | 'task'
  | 'quote'
  | 'link'
  | 'code';

/**
 * Walk the syntax tree from the cursor outward and collect the set of
 * formats that apply at the current selection.head. Called on every
 * cursor-move and doc-change transaction so the toolbar's pressed-state
 * stays in sync with where the user actually is.
 *
 * Walks UP from the innermost node so inline formatting (Emphasis,
 * StrongEmphasis, InlineCode) and block context (Heading, Blockquote,
 * BulletList, OrderedList, FencedCode) both get captured — they're
 * all ancestors of the cursor's resolved position.
 *
 * Task suppresses Bullet: a Task is structurally inside a BulletList in
 * GFM, so the walk would surface both. The user thinks of it as "a task
 * line" not "a bullet line containing a task" — show only the more
 * specific format on the toolbar.
 */
export function detectActiveFormats(view: EditorView): ReadonlySet<ActiveFormat> {
  const active = new Set<ActiveFormat>();
  const state = view.state;
  const sel = state.selection.main;
  const head = sel.head;
  let node: SyntaxNode | null = syntaxTree(state).resolveInner(head, -1);

  // Boundary correction: `resolveInner(head, -1)` lands on a node ENDING at
  // the cursor. For inline wrap markers (the closing `**` of bold, etc.)
  // this means the cursor is VISUALLY OUTSIDE the wrap — it sits just
  // after the closing marker. Without correction we'd walk up to the wrap
  // parent and incorrectly light the bold/italic/strike/code/link button.
  // Same shape at the LEFT edge (cursor at the opening mark's start).
  //
  // Skip the wrap by starting the parent walk one level higher when we
  // detect a marker pinned to either edge of its wrap parent.
  if (node && node.parent) {
    const isInlineMark =
      node.name === 'EmphasisMark' ||
      node.name === 'StrikethroughMark' ||
      node.name === 'CodeMark' ||
      node.name === 'LinkMark';
    if (isInlineMark) {
      const parent = node.parent;
      const atRightEdge = parent.to === node.to && head === node.to;
      const atLeftEdge = parent.from === node.from && head === node.from;
      if (atRightEdge || atLeftEdge) {
        node = parent.parent;
      }
    }
  }

  // Track the INNERMOST list-type ancestor only. The walk goes innermost-
  // outward; if a Task lives inside a BulletList, we'd otherwise add both
  // 'task' and 'bullet'. Same for an OrderedList nested in a BulletList
  // (or vice versa). The toolbar's list buttons are mutually exclusive
  // (clicking one strips the others), so the pressed state should reflect
  // only the most specific list context the cursor sits in.
  let innermostListFormat: 'task' | 'bullet' | 'numbered' | null = null;

  while (node) {
    switch (node.name) {
      case 'StrongEmphasis':
        active.add('bold');
        break;
      case 'Emphasis':
        active.add('italic');
        break;
      case 'Strikethrough':
        active.add('strike');
        break;
      case 'InlineCode':
      case 'FencedCode':
        active.add('code');
        break;
      case 'Link':
        active.add('link');
        break;
      case 'ATXHeading1':
      case 'ATXHeading2':
      case 'ATXHeading3':
      case 'ATXHeading4':
      case 'ATXHeading5':
      case 'ATXHeading6':
        active.add('heading');
        break;
      case 'Blockquote':
        active.add('quote');
        break;
      case 'Task':
        if (!innermostListFormat) innermostListFormat = 'task';
        break;
      case 'BulletList':
        if (!innermostListFormat) innermostListFormat = 'bullet';
        break;
      case 'OrderedList':
        if (!innermostListFormat) innermostListFormat = 'numbered';
        break;
    }
    node = node.parent;
  }

  if (innermostListFormat) active.add(innermostListFormat);
  return active;
}

// ---------- commands ----------

export const toggleBold = (view: EditorView): boolean => toggleWrap(view, '**');
export const toggleItalic = (view: EditorView): boolean => toggleWrap(view, '*');
export const toggleStrikethrough = (view: EditorView): boolean =>
  toggleWrap(view, '~~');

/** Inline code (single line) → backtick wrap. Multi-line selection → fenced
 *  triple-backtick block. Cursor already inside a FencedCode → strip the
 *  enclosing fence (unwrap). */
export const toggleInlineCode = (view: EditorView): boolean => {
  const state = view.state;
  const { from, to } = mainRange(view);
  const doc = state.doc;

  // If the cursor's start position sits inside an existing FencedCode, the
  // toolbar action is "remove this fence" rather than "wrap again." Without
  // this, pressing Cmd+E (or clicking the code button) inside a fenced
  // block would insert ANOTHER fence inside it — broken nested markdown.
  // Mirrors the toggle-off behavior the inline wraps already provide.
  let fenceNode: SyntaxNode | null = syntaxTree(state).resolveInner(from, -1);
  while (fenceNode && fenceNode.name !== 'FencedCode') {
    fenceNode = fenceNode.parent;
  }
  if (fenceNode) {
    const openLine = doc.lineAt(fenceNode.from);
    const closeLine = doc.lineAt(fenceNode.to);
    // Delete in reverse order so the second change's positions don't shift.
    let closeTo = closeLine.to;
    if (closeLine.number < doc.lines) closeTo += 1; // include trailing newline
    let openTo = openLine.to;
    if (openLine.number < doc.lines) openTo += 1;
    view.dispatch({
      changes: [
        { from: openLine.from, to: openTo, insert: '' },
        { from: closeLine.from, to: closeTo, insert: '' },
      ],
      userEvent: 'delete.fence.unwrap',
    });
    view.focus();
    return true;
  }

  const selected = doc.sliceString(from, to);
  const isMultiLine = selected.includes('\n');

  if (isMultiLine) {
    // Fenced block. Matches the shape that autoExpandOnThirdBacktick and
    // expandFencedCodeOnEnter produce: opening fence + body + closing fence
    // + trailing newline. The trailing newline guarantees a navigable line
    // below the box (down-arrow exits the fence cleanly). The cursor lands
    // at the END of the wrapped body so the user can keep typing after
    // their just-wrapped content — NOT on the hidden opening fence line
    // (where live-preview would render the cursor on an empty-looking line
    // with no visible feedback, the same broken UX the fence cursor-skip
    // filter prevents elsewhere but bails on docChanged transactions).
    const insert = '```\n' + selected + '\n```\n';
    // After insert at `from`: line 1 is the opening fence (positions
    // from..from+3), then \n at from+3, then body starts at from+4 and
    // ends at from+4+selected.length. Land cursor at body end.
    const bodyEnd = from + 4 + selected.length;
    replaceAndMoveCursor(view, from, to, insert, bodyEnd);
    return true;
  }
  return toggleWrap(view, '`');
};

/** Toggle a leading `- ` prefix on each selected line.
 *
 *  Behavior:
 *    - Preserves indentation (e.g. `    foo` → `    - foo` keeps the 4
 *      spaces). The prefix sits AFTER the indent, not at column 0.
 *    - Strips any existing cross-type marker first: `1. item` → `- item`,
 *      `- [ ] task` → `- task`. Otherwise pressing the bullet button on a
 *      numbered list line would produce `- 1. item`.
 *    - Global toggle: if every non-blank, non-fence line in the selection
 *      is ALREADY a plain bullet, strip them all. Otherwise apply.
 *    - Blank lines are skipped (don't stamp `- ` onto paragraph gaps).
 *    - Fence-boundary lines are skipped by transformLines (don't corrupt
 *      the fence opener/closer).
 */
export const toggleBulletList = (view: EditorView): boolean => {
  // Fast path: single-line empty selection → direct dispatch with cursor
  // placed after the marker. Without this, transformLines' default
  // selection mapping leaves the cursor on the LEFT of the inserted
  // `- ` and the user's first keystroke breaks the parse.
  if (applyListMarkerToCurrentLine(view, '- ')) return true;

  const state = view.state;
  const { from, to } = mainRange(view);
  const startLine = state.doc.lineAt(from);
  const endLine = state.doc.lineAt(to);

  let allBullets = true;
  let sawNonBlank = false;
  for (let n = startLine.number; n <= endLine.number; n++) {
    if (isFenceBoundaryLine(state, n)) continue;
    const t = state.doc.line(n).text;
    if (t.trim() === '') continue;
    sawNonBlank = true;
    const { content } = splitIndent(t);
    // Plain bullet only — `- ` NOT followed by `[` (task syntax).
    if (!/^- (?!\[)/.test(content)) {
      allBullets = false;
      break;
    }
  }
  const addOnBlanks = !sawNonBlank;
  const stripping = sawNonBlank && allBullets;

  return transformLines(view, (line) => {
    if (line.trim() === '') {
      // Selection is on a blank line — stamp a bullet marker so the
      // user can immediately start typing. Matches the toggleTaskList
      // empty-line behavior; without this, clicking the bullet button
      // on an empty line is a no-op with no visible feedback.
      return addOnBlanks ? '- ' : line;
    }
    const { indent, content } = splitIndent(line);
    if (stripping) {
      return indent + content.replace(/^- /, '');
    }
    return indent + '- ' + stripListMarker(content);
  });
};

/** Toggle a leading `N. ` prefix on each selected line.
 *
 *  Behavior matches toggleBulletList — preserves indentation, strips cross-
 *  type markers (`- item` → `1. item`), and uses a global strip-vs-apply
 *  decision. Counter restarts at 1 per command invocation.
 */
export const toggleNumberedList = (view: EditorView): boolean => {
  // Fast path: empty single-line selection → stamp `1. ` with cursor
  // placed AFTER the marker. Same rationale as toggleBulletList's
  // fast path — keeps the user's first keystroke from breaking the parse.
  if (applyListMarkerToCurrentLine(view, '1. ')) return true;

  const state = view.state;
  const { from, to } = mainRange(view);
  const startLine = state.doc.lineAt(from);
  const endLine = state.doc.lineAt(to);

  let allNumbered = true;
  let sawNonBlank = false;
  for (let n = startLine.number; n <= endLine.number; n++) {
    if (isFenceBoundaryLine(state, n)) continue;
    const t = state.doc.line(n).text;
    if (t.trim() === '') continue;
    sawNonBlank = true;
    const { content } = splitIndent(t);
    if (!/^\d+\. /.test(content)) {
      allNumbered = false;
      break;
    }
  }
  const addOnBlanks = !sawNonBlank;
  const stripping = sawNonBlank && allNumbered;

  let counter = 1;
  return transformLines(view, (line) => {
    if (line.trim() === '') {
      // Selection on a blank line — stamp a `1. ` marker so the user
      // can immediately start typing. Same posture as toggleBulletList /
      // toggleTaskList for consistency.
      return addOnBlanks ? `${counter++}. ` : line;
    }
    const { indent, content } = splitIndent(line);
    if (stripping) {
      return indent + content.replace(/^\d+\. /, '');
    }
    return indent + `${counter++}. ` + stripListMarker(content);
  });
};

/** Toggle a leading `> ` prefix on each selected line.
 *
 *  Behavior:
 *    - Preserves indentation (so `    foo` → `    > foo`).
 *    - Does NOT strip list markers when applying (a quoted list `> - item`
 *      is a valid construct — common in prose). Strips only `> ` when
 *      toggling off.
 *    - Global toggle: strip if every non-blank, non-fence line already
 *      starts with `> ` (after indent).
 *    - Blank lines and fence boundary lines are skipped.
 */
export const toggleQuote = (view: EditorView): boolean => {
  const state = view.state;
  const { from, to } = mainRange(view);
  const startLine = state.doc.lineAt(from);
  const endLine = state.doc.lineAt(to);

  let allQuotes = true;
  let sawNonBlank = false;
  for (let n = startLine.number; n <= endLine.number; n++) {
    if (isFenceBoundaryLine(state, n)) continue;
    const t = state.doc.line(n).text;
    if (t.trim() === '') continue;
    sawNonBlank = true;
    const { content } = splitIndent(t);
    if (!content.startsWith('> ')) {
      allQuotes = false;
      break;
    }
  }
  const stripping = sawNonBlank && allQuotes;

  return transformLines(view, (line) => {
    if (line.trim() === '') return line;
    const { indent, content } = splitIndent(line);
    if (stripping) {
      return indent + content.replace(/^> /, '');
    }
    return indent + '> ' + content;
  });
};

/** Cycle heading level on the current line: none → H1 → H2 → H3 → none.
 *  When a multi-line selection spans different existing levels, the cycle
 *  is decided by the FIRST line; every line gets set to the next level.
 *
 *  Lines that are already H4/H5/H6 are LEFT ALONE — the strip regex matches
 *  only `^#{1,3} ` so we never silently destroy a heading level the toolbar
 *  itself can't produce. A user who manually authored an H4+ keeps it. */
export const cycleHeading = (view: EditorView): boolean => {
  const state = view.state;
  const { from } = mainRange(view);
  const firstLine = state.doc.lineAt(from);
  const m = /^(#{1,3}) /.exec(firstLine.text);
  const currentLevel = m ? m[1].length : 0;
  const nextLevel = (currentLevel + 1) % 4; // 0..3

  const nextPrefix = nextLevel === 0 ? '' : '#'.repeat(nextLevel) + ' ';
  return transformLines(view, (line) => {
    if (line.trim() === '') return line;
    // Leave H4-H6 untouched. Without this guard the cycle would strip
    // nothing (the narrowed /^#{1,3} / regex doesn't match 4+ hashes) but
    // would still prepend `# `, turning `###### Big` into `# ###### Big`.
    if (/^#{4,6} /.test(line)) return line;
    const stripped = line.replace(/^#{1,3} /, '');
    return nextPrefix + stripped;
  });
};

/** Insert a Markdown link. Cursor placement:
 *    - selection exists  → `[selection](https://)`, URL portion pre-selected
 *      so typing replaces the scheme placeholder.
 *    - empty selection   → `[](https://)`, cursor INSIDE THE BRACKETS (so
 *      the user types the label first, matching prose flow).
 *
 *  Placeholder is `https://` (not `url`) on purpose: if the user clicks
 *  away without filling it in, `[label](https://)` reads as obviously-
 *  incomplete rather than a deceptive working-but-broken link pointing to
 *  the literal string "url". */
const LINK_URL_PLACEHOLDER = 'https://';

export const insertLink = (view: EditorView): boolean => {
  const { from, to } = mainRange(view);
  const doc = view.state.doc;
  const selected = doc.sliceString(from, to);

  if (selected.length === 0) {
    const insert = `[](${LINK_URL_PLACEHOLDER})`;
    // Cursor at position 1 (inside the brackets, ready for the label).
    replaceAndMoveCursor(view, from, to, insert, from + 1);
    return true;
  }

  const insert = `[${selected}](${LINK_URL_PLACEHOLDER})`;
  // Cursor at the URL placeholder → select it so the user can paste or
  // type over it.
  const urlStart = from + 1 + selected.length + 2; // [SEL]( -> here
  const urlEnd = urlStart + LINK_URL_PLACEHOLDER.length;
  view.dispatch({
    changes: { from, to, insert },
    selection: EditorSelection.range(urlStart, urlEnd),
  });
  view.focus();
  return true;
};

/** Toggle GFM task list syntax on each selected line.
 *
 *  - Line starts with `- [ ] ` or `- [x] ` (task) → strip task syntax,
 *    leaving the raw content.
 *  - Anything else (plain text, bullet list, numbered list) → upgrade to
 *    `- [ ] ` task. Replaces any existing `- ` or `1. ` prefix first so we
 *    don't end up with `- - [ ] item`.
 *
 *  Blank lines are skipped. Fence-boundary lines are skipped by
 *  transformLines (don't corrupt the fence). The check vs. uncheck state
 *  toggle (`- [ ]` ↔ `- [x]`) is the job of a future clickable-widget
 *  decoration on the rendered box — this toolbar button only handles
 *  task-syntax presence. */
export const toggleTaskList = (view: EditorView): boolean => {
  // Fast path: empty single-line selection → stamp `- [ ] ` with cursor
  // placed AFTER the marker. Matches the bullet/numbered pattern.
  if (applyListMarkerToCurrentLine(view, '- [ ] ')) return true;

  const state = view.state;
  const { from, to } = mainRange(view);
  const startLine = state.doc.lineAt(from);
  const endLine = state.doc.lineAt(to);

  // Behavior matches toggleBulletList/Numbered: preserves indentation,
  // strips cross-type markers, global strip-vs-apply decision. Plus one
  // extra: clicking on an empty line stamps `- [ ] ` so the user can
  // start typing a task immediately (without this, the transform's blank-
  // line skip would silently no-op).
  let allTasks = true;
  let sawNonBlank = false;
  for (let n = startLine.number; n <= endLine.number; n++) {
    if (isFenceBoundaryLine(state, n)) continue;
    const t = state.doc.line(n).text;
    if (t.trim() === '') continue;
    sawNonBlank = true;
    const { content } = splitIndent(t);
    if (!/^- \[[ x]\] /.test(content)) {
      allTasks = false;
      break;
    }
  }
  const addOnBlanks = !sawNonBlank;
  const stripping = sawNonBlank && allTasks;

  return transformLines(view, (line) => {
    if (line.trim() === '') {
      return addOnBlanks ? '- [ ] ' : line;
    }
    const { indent, content } = splitIndent(line);
    if (stripping) {
      return indent + content.replace(/^- \[[ x]\] /, '');
    }
    return indent + '- [ ] ' + stripListMarker(content);
  });
};

/** Insert today's date (ISO `YYYY-MM-DD`) at the cursor, replacing any
 *  selection. Uses the system local timezone so a user writing late at
 *  night doesn't get tomorrow's UTC date stamped. */
export const insertCurrentDate = (view: EditorView): boolean => {
  const { from, to } = mainRange(view);
  const now = new Date();
  const yyyy = now.getFullYear();
  const mm = String(now.getMonth() + 1).padStart(2, '0');
  const dd = String(now.getDate()).padStart(2, '0');
  const dateStr = `${yyyy}-${mm}-${dd}`;
  replaceAndMoveCursor(view, from, to, dateStr, from + dateStr.length);
  return true;
};

// ---------- list-aware indent ----------

/**
 * Maximum leading whitespace (in spaces) we allow before refusing to
 * indent further. Each nesting level adds 2 spaces, so 8 spaces caps the
 * usable nesting at 5 levels (the marker on level 5 sits at column 8).
 * Past that, CommonMark's `parent content offset + 3` rule starts to
 * misclassify the line as paragraph continuation rather than a nested
 * list item, and our bullet/checkbox widgets stop rendering. Cap
 * prevents the user from blindly tabbing into the dead zone.
 */
const LIST_INDENT_MAX = 8;

/** Regex that matches a markdown list-marker line: leading whitespace,
 *  then `-`/`*`/`+` (bullet) or `N.`/`N)` (numbered), followed by space,
 *  tab, or end-of-line. Used as a fallback for Lezer's syntax tree, which
 *  sometimes doesn't classify an EMPTY bullet line (`- ` with no content
 *  after the space) as a `ListItem` — particularly mid-edit before the
 *  parse catches up. Catches the parse-lag case so Tab indent + outdent
 *  still treat the line as a list context. */
const LIST_LINE_RE = /^[ \t]*(?:-|\*|\+|\d+[.)])(?: |\t|$)/;

/**
 * True when the cursor's line is inside a Lezer ListItem (bullet,
 * numbered, or task). Walks the syntax tree from line-start outward;
 * falls back to a regex on the line text to catch cases where Lezer
 * hasn't classified the line yet (most commonly: a freshly-inserted
 * empty `- ` bullet that has no content for the parser to anchor to).
 * The Task line carries its own Task node nested in ListItem, so the
 * ListItem check catches both.
 */
function cursorInListItem(view: EditorView): boolean {
  const sel = view.state.selection.main;
  const line = view.state.doc.lineAt(sel.from);
  let node: SyntaxNode | null = syntaxTree(view.state).resolveInner(
    line.from,
    1
  );
  while (node) {
    if (node.name === 'ListItem') return true;
    node = node.parent;
  }
  return LIST_LINE_RE.test(line.text);
}

/**
 * Walk backward from `line` to determine the deepest indent column that
 * Tab can land at without breaking the parse. CommonMark's rule for sub-
 * items: a list marker at column `N` is a sub-item of the nearest list
 * line above it ONLY when `N` is between `parent_content_offset` and
 * `parent_content_offset + 3`. Past that, the line gets reclassified as
 * continuation text of the parent's content (and our bullet widget
 * silently disappears).
 *
 * Algorithm: walk backwards looking for the FIRST list line at indent
 * `<= currentIndent` — that's the potential parent (a sibling at the
 * same indent becomes the parent after the current line is indented one
 * level deeper; a true ancestor at lower indent already is the parent).
 * Return `parent_content_offset + 3` (clamped at LIST_INDENT_MAX as a
 * safety net). If no parent exists (top-of-doc lone item, blank-line
 * break, or a non-list line above), return `currentIndent` so Tab is
 * a no-op — preventing the user from tabbing a standalone item into
 * indented-code-block territory.
 */
function maxListIndentAllowed(view: EditorView, line: { number: number; text: string }): number {
  const currentIndent = /^[ \t]*/.exec(line.text)?.[0].length ?? 0;
  for (let n = line.number - 1; n >= 1; n--) {
    const prevText = view.state.doc.line(n).text;
    if (prevText.trim() === '') return currentIndent; // blank breaks list context
    const prevIndent = /^[ \t]*/.exec(prevText)?.[0].length ?? 0;
    if (!LIST_LINE_RE.test(prevText)) return currentIndent; // non-list above
    if (prevIndent <= currentIndent) {
      // Potential parent (sibling-to-be-parent or true ancestor).
      const markerMatch = /^[ \t]*(-|\*|\+|\d+[.)])\s/.exec(prevText);
      if (!markerMatch) return currentIndent;
      const markerLen = markerMatch[1].length + 1; // marker chars + the space
      const parentContentOffset = prevIndent + markerLen;
      return Math.min(parentContentOffset + 3, LIST_INDENT_MAX);
    }
    // Deeper-nested item above (e.g. last item of an inner list); keep walking.
  }
  return currentIndent; // no parent found
}

/**
 * Tab inside a list item: indent the LINE by 2 spaces. The cap comes
 * from `maxListIndentAllowed` which respects CommonMark's
 * parent_content_offset + 3 rule, not a flat LIST_INDENT_MAX. Two
 * tabs is enough to break the parse on a top-level item (col 0 → col 4,
 * which is at the indented-code-block threshold relative to root) — the
 * dynamic cap stops that.
 *
 * Returns true (handled) inside lists even at the cap so the keypress
 * doesn't fall through to default `indentMore`, which would happily
 * insert another tab or 4 spaces and break the parse.
 *
 * Outside a list, returns false so default Tab behavior (indentMore /
 * insertTab) takes over for non-list content.
 */
const indentListItem: Command = (view) => {
  if (!view.state.selection.main.empty) return false;
  if (!cursorInListItem(view)) return false;

  const line = view.state.doc.lineAt(view.state.selection.main.from);
  const leading = /^[ \t]*/.exec(line.text)?.[0] ?? '';
  const leadingCols = leading.replace(/\t/g, '    ').length;
  const maxAllowed = maxListIndentAllowed(view, line);
  if (leadingCols + 2 > maxAllowed) {
    // Would exceed the parent's content_offset + 3 (or there's no
    // parent context). Consume the keystroke so default Tab doesn't
    // step in and break the parse anyway.
    return true;
  }

  view.dispatch({
    changes: { from: line.from, insert: '  ' },
    selection: {
      anchor: view.state.selection.main.anchor + 2,
      head: view.state.selection.main.head + 2,
    },
    userEvent: 'input.indent.list',
  });
  return true;
};

/**
 * Shift+Tab inside a list item: strip 2 leading spaces (one nesting
 * level). No-op when there's no leading indent to remove. Outside a
 * list returns false to let default Shift+Tab handle other contexts.
 */
const outdentListItem: Command = (view) => {
  if (!view.state.selection.main.empty) return false;
  if (!cursorInListItem(view)) return false;

  const line = view.state.doc.lineAt(view.state.selection.main.from);
  if (!line.text.startsWith('  ')) return true; // nothing to strip, consume key

  view.dispatch({
    changes: { from: line.from, to: line.from + 2, insert: '' },
    selection: {
      anchor: Math.max(line.from, view.state.selection.main.anchor - 2),
      head: Math.max(line.from, view.state.selection.main.head - 2),
    },
    userEvent: 'delete.outdent.list',
  });
  return true;
};

// ---------- keymap ----------

/**
 * Keymap bindings. Prepend this to the rest of MarkdownEditor's keymap.of
 * call so these shortcuts win precedence over CodeMirror's defaults. None
 * of these collide with `defaultKeymap`, `historyKeymap`, or `indentWithTab`.
 *
 * Shortcut choices:
 *   - Cmd+B / Cmd+I / Cmd+E / Cmd+K: editor convention.
 *   - Cmd+Shift+7 / 8 / 9: ordered list / bulleted list / blockquote — a
 *     numerically-adjacent trio so the three line-prefix list-y formats
 *     live together on the keyboard.
 *   - Cmd+Shift+X: strikethrough (matches Slack / Linear).
 *   - Cmd+Alt+0: heading cycle (mirrors Google Docs "normal text"; stays
 *     clear of Cmd+1/2/3 which switch browser tabs).
 *   - Cmd+Shift+L: task list (GitHub / Linear convention).
 *   - Cmd+;: today's date — matches Google Sheets' insert-DATE shortcut.
 *     (Sheets uses Cmd+Shift+; for insert-TIME — we keep Cmd+; for date so
 *     muscle memory transfers correctly.) Safe on macOS: native NSTextField
 *     binds Cmd+; to "find next misspelled word," but CodeMirror runs in a
 *     webview contenteditable, not an NSTextField, so that binding doesn't
 *     reach us.
 */
export const markdownFormattingKeymap: KeyBinding[] = [
  // Tab inside a list: indent by 2 spaces (cap at LIST_INDENT_MAX). Outside
  // a list this returns false so default Tab (indentMore / insertTab via
  // indentWithTab) handles normal content. Shift+Tab outdents by 2 spaces
  // inside lists, same fall-through outside.
  { key: 'Tab', run: indentListItem, shift: outdentListItem },
  { key: 'Mod-b', run: toggleBold, preventDefault: true },
  { key: 'Mod-i', run: toggleItalic, preventDefault: true },
  { key: 'Mod-Shift-x', run: toggleStrikethrough, preventDefault: true },
  { key: 'Mod-k', run: insertLink, preventDefault: true },
  { key: 'Mod-e', run: toggleInlineCode, preventDefault: true },
  { key: 'Mod-Shift-7', run: toggleNumberedList, preventDefault: true },
  { key: 'Mod-Shift-8', run: toggleBulletList, preventDefault: true },
  { key: 'Mod-Shift-9', run: toggleQuote, preventDefault: true },
  { key: 'Mod-Shift-l', run: toggleTaskList, preventDefault: true },
  { key: 'Mod-Alt-0', run: cycleHeading, preventDefault: true },
  { key: 'Mod-;', run: insertCurrentDate, preventDefault: true },
];
