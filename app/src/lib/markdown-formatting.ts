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
import type { EditorView, KeyBinding } from '@codemirror/view';
import { EditorSelection } from '@codemirror/state';

// ---------- helpers ----------

/** Selection range as {from, to} in document UTF-16 units. */
function mainRange(view: EditorView): { from: number; to: number } {
  const { from, to } = view.state.selection.main;
  return { from, to };
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
 *  If no selection: insert the empty wrap and place the cursor between the
 *  marks. If the selection is already wrapped: unwrap. */
function toggleWrap(view: EditorView, mark: string): boolean {
  const { from, to } = mainRange(view);
  const doc = view.state.doc;
  const markLen = mark.length;

  // Empty selection → insert pair, cursor between.
  if (from === to) {
    replaceAndMoveCursor(view, from, to, mark + mark, from + markLen);
    return true;
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
 *  newline) and returns the rewritten content. */
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

// ---------- commands ----------

export const toggleBold = (view: EditorView): boolean => toggleWrap(view, '**');
export const toggleItalic = (view: EditorView): boolean => toggleWrap(view, '*');
export const toggleStrikethrough = (view: EditorView): boolean =>
  toggleWrap(view, '~~');

/** Inline code (single line) → backtick wrap. Multi-line selection → fenced
 *  triple-backtick block with a blank language hint line so the user can
 *  type the language. */
export const toggleInlineCode = (view: EditorView): boolean => {
  const { from, to } = mainRange(view);
  const doc = view.state.doc;
  const selected = doc.sliceString(from, to);
  const isMultiLine = selected.includes('\n');

  if (isMultiLine) {
    // Wrap as fenced block.
    const insert = '```\n' + selected + '\n```';
    // Place cursor at the end of the opening fence so the user can type a
    // language hint immediately (```js).
    replaceAndMoveCursor(view, from, to, insert, from + 3);
    return true;
  }
  return toggleWrap(view, '`');
};

/** Toggle a leading `- ` prefix on each selected line. Blank lines are
 *  skipped so paragraph-style multi-line selections don't get empty
 *  `- ` stamped into the gap between paragraphs. */
export const toggleBulletList = (view: EditorView): boolean =>
  transformLines(view, (line) => {
    if (line.trim() === '') return line;
    return line.startsWith('- ') ? line.slice(2) : '- ' + line;
  });

/** Toggle a leading `N. ` prefix on each selected line. Numbers from 1 on
 *  apply; strips any existing leading digits+`. ` on unapply. */
export const toggleNumberedList = (view: EditorView): boolean => {
  // Decide apply-vs-unapply globally: if EVERY non-empty line already has
  // an "N. " prefix, treat as unapply.
  const state = view.state;
  const { from, to } = mainRange(view);
  const startLine = state.doc.lineAt(from);
  const endLine = state.doc.lineAt(to);
  const allNumbered = (() => {
    for (let n = startLine.number; n <= endLine.number; n++) {
      const t = state.doc.line(n).text;
      if (t.trim() === '') continue;
      if (!/^\d+\. /.test(t)) return false;
    }
    return true;
  })();

  let counter = 1;
  return transformLines(view, (line) => {
    if (allNumbered) {
      return line.replace(/^\d+\. /, '');
    }
    if (line.trim() === '') return line;
    return `${counter++}. ` + line.replace(/^\d+\. /, '');
  });
};

/** Toggle a leading `> ` prefix on each selected line. Blank lines are
 *  skipped so quoted-prose blocks don't get `> ` stamped onto paragraph
 *  gaps (which most renderers would treat as continuing the quote, but
 *  it reads as noise in the source). */
export const toggleQuote = (view: EditorView): boolean =>
  transformLines(view, (line) => {
    if (line.trim() === '') return line;
    return line.startsWith('> ') ? line.slice(2) : '> ' + line;
  });

/** Cycle heading level on the current line: none → H1 → H2 → H3 → none.
 *  When a multi-line selection spans different existing levels, the cycle
 *  is decided by the FIRST line; every line gets set to the next level. */
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
    const stripped = line.replace(/^#{1,6} /, '');
    return nextPrefix + stripped;
  });
};

/** Insert a Markdown link. Cursor placement:
 *    - selection exists  → `[selection](url)`, cursor between the parens
 *    - empty selection   → `[](url)`, cursor INSIDE THE BRACKETS (so the
 *                          user types the label first, matching prose flow)
 */
export const insertLink = (view: EditorView): boolean => {
  const { from, to } = mainRange(view);
  const doc = view.state.doc;
  const selected = doc.sliceString(from, to);

  if (selected.length === 0) {
    const insert = '[](url)';
    // Cursor at position 1 (inside the brackets, ready for the label).
    replaceAndMoveCursor(view, from, to, insert, from + 1);
    return true;
  }

  const insert = `[${selected}](url)`;
  // Cursor at "(url)" → select the placeholder URL so the user can paste
  // or type over it.
  const urlStart = from + 1 + selected.length + 2; // [SEL]( -> here
  const urlEnd = urlStart + 'url'.length;
  view.dispatch({
    changes: { from, to, insert },
    selection: EditorSelection.range(urlStart, urlEnd),
  });
  view.focus();
  return true;
};

// ---------- keymap ----------

/**
 * Keymap bindings. Prepend this to the rest of MarkdownEditor's keymap.of
 * call so these shortcuts win precedence over CodeMirror's defaults. None
 * of these collide with `defaultKeymap`, `historyKeymap`, or `indentWithTab`.
 */
export const markdownFormattingKeymap: KeyBinding[] = [
  { key: 'Mod-b', run: toggleBold, preventDefault: true },
  { key: 'Mod-i', run: toggleItalic, preventDefault: true },
  { key: 'Mod-k', run: insertLink, preventDefault: true },
  { key: 'Mod-e', run: toggleInlineCode, preventDefault: true },
  { key: 'Mod-Shift-7', run: toggleNumberedList, preventDefault: true },
  { key: 'Mod-Shift-8', run: toggleBulletList, preventDefault: true },
];
