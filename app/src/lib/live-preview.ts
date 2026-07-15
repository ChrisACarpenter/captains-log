/**
 * CodeMirror 6 extension: Slack/Typora-style "Live Preview" decorations.
 *
 * Hides Markdown marker tokens (`**`, `*`, `~~`, backticks, `#`, `[`/`]`/
 * `(...)`/URL-inside-Link or Image) so the user sees rendered rich text
 * while the buffer on disk stays canonical Markdown byte-for-byte.
 *
 * ## Architectural decisions documented
 *
 * **Why `Decoration.replace` + an `EditorView.atomicRanges` registration.**
 * `Decoration.replace` with no widget collapses the marker range to zero
 * visible width — the user can't see the asterisks/hashes/backticks. But
 * by itself, replace doesn't make the cursor skip those positions; arrow
 * keys would step through invisible character positions one by one, which
 * reads as "right arrow does nothing" to the user. Registering the same
 * DecorationSet via `EditorView.atomicRanges` tells CM6's cursor motion
 * commands to treat each hidden range as a single skip-step. Right arrow
 * from the end of "bold" jumps straight past the closing `**` to the
 * next visible position. Selection drag also treats hidden ranges as
 * unit hops. Backspace from just past a hidden range deletes the whole
 * marker chunk at once.
 *
 * Note that atomic-range rules ONLY apply to user-driven cursor commands.
 * A programmatic `view.dispatch({ selection })` (e.g. the toolbar's
 * Cmd+K placing the selection inside the URL placeholder) is preserved
 * verbatim — it bypasses the atomic-skip normalization. That's why the
 * reveal-on-active-Link logic below matters: even though the toolbar
 * correctly positions the selection inside a URL placeholder, the user
 * needs to SEE that selection to know they're meant to type a URL.
 *
 * **Reveal-on-active-Link.** When the cursor (or any selection endpoint)
 * lands inside a `Link` or `Image` node, that node's markers + URL stay
 * visible. This handles three concrete cases:
 *
 *   1. The user types `[label](url)` inline — once parsed as a Link, the
 *      decorations would normally hide everything except "label." But
 *      while the cursor is still on that line / inside the Link, the
 *      URL placeholder needs to remain visible so the user can edit it.
 *   2. The toolbar's Cmd+K with no selection inserts `[](url)` and lands
 *      the cursor inside the brackets. Without the reveal, the user
 *      sees ZERO visible change — the toolbar appears broken.
 *   3. The toolbar's Cmd+K with a selection wraps as `[selected](url)`
 *      and highlights "url" inside the parens. Without the reveal, the
 *      user can't see what to type or where their typing lands.
 *
 * The reveal is narrowly scoped to Link / Image specifically — not all
 * marker types — because Link is the only construct where the marker
 * itself carries content (the URL) that the user needs to author. Bold
 * and italic markers carry no content; hiding them everywhere is fine.
 *
 * **Why we don't add Decoration.mark for content classes.**
 * `defaultHighlightStyle` (registered as a PRIMARY highlighter in
 * MarkdownEditor.svelte) already styles `tags.strong` (bold), `tags.emphasis`
 * (italic), `tags.strikethrough` (line-through), `tags.heading` (bold +
 * underline), `tags.link` (underline), and `tags.monospace` (none — handled
 * by the CSS below targeting `.cm-line` parents). Our HighlightStyle
 * override adds the system-ui font-family swap for bold/heading (ABeeZee
 * has no bold weight). The content styling is already in place; this
 * extension only needs to HIDE markers, not re-style content.
 *
 * **Why heading sizing lives in this extension, not the global HighlightStyle.**
 * Per-level heading sizes (h1 larger than h2 larger than h3) are a
 * presentation choice specific to the "rich text view" experience, not a
 * core spell-check / accessibility property. Putting it here keeps the
 * source-mode pathway (when livePreview=false on /journal) free of the
 * size override — /journal stays raw markdown, sized normally.
 *
 * **Why we hide URL inside Link but not standalone URLs.** A Markdown link
 * is `[text](url)`. We hide the `[`, `]`, `(`, `)`, AND the URL token,
 * leaving just the link text visible (already styled as a link by
 * defaultHighlightStyle). For bare URLs (GFM autolinks like
 * `https://example.com` typed without brackets) we want the URL itself
 * to remain visible — it IS the link text. Lezer-markdown helpfully
 * distinguishes: a URL inside a Link parent gets hidden; a standalone URL
 * stays visible.
 *
 * ## Known v1 limitations
 *
 *   - Multi-line emphasis (`**bold\nbold**`) is unusual in CommonMark but
 *     Lezer parses it correctly; the markers still hide and the wrap still
 *     applies. Verify in QA.
 *   - Nested emphasis (`***strong-italic***`) — the outer `**` and inner
 *     `*` are separate EmphasisMark nodes; both hide independently.
 *   - Fenced code blocks: the ``` fences ARE emitted as CodeMark and would
 *     get hidden. We deliberately KEEP them visible — the body of a code
 *     block needs a visual frame, and the fence markers double as that
 *     frame. (See the matchContext guard in the iterate callback.)
 *   - Task list checkboxes (`[ ]` / `[x]`): not converted to clickable
 *     widgets in v1. The brackets stay visible as part of the line. A
 *     follow-up step adds the widget decoration.
 *   - Reveal-on-cursor: not in v1. If typing at the boundary of a hidden
 *     span feels weird (e.g. extending a bold word), revisit.
 */

import {
  Decoration,
  EditorView,
  ViewPlugin,
  WidgetType,
  keymap,
  type Command,
  type DecorationSet,
  type PluginValue,
  type ViewUpdate,
} from '@codemirror/view';
import { EditorState, Prec, RangeSetBuilder } from '@codemirror/state';
import { syntaxTree } from '@codemirror/language';
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { tags } from '@lezer/highlight';
import type { Extension } from '@codemirror/state';
import type { SyntaxNode } from '@lezer/common';
import { dateChip } from './date-chip';
import { linkChip } from './link-chip';

/**
 * Live-preview-only highlight overrides:
 *
 *   - Heading sizes (h1 > h2 > h3) — defaultHighlightStyle only gives
 *     `tags.heading` a bold+underline, no size differentiation. Without
 *     these, hiding the `#` marker would erase the visual that "this is
 *     a heading" because the text would just look like bold body copy.
 *     `textDecoration: 'none'` cancels defaultHighlightStyle's underline
 *     — large bold text doesn't need underline AND italic-style emphasis.
 *
 *   - Monospace (tags.monospace covers inline code and fenced code body)
 *     — defaultHighlightStyle has no rule for it, so without this inline
 *     code would render as plain prose after we hide the backticks.
 *     Switching the font carries the "this is code" signal once the
 *     backtick markers disappear.
 */
const livePreviewStyle = HighlightStyle.define([
  { tag: tags.heading1, fontSize: '1.5em', textDecoration: 'none' },
  { tag: tags.heading2, fontSize: '1.3em', textDecoration: 'none' },
  { tag: tags.heading3, fontSize: '1.15em', textDecoration: 'none' },
  { tag: tags.heading4, fontSize: '1em', textDecoration: 'none' },
  { tag: tags.heading5, fontSize: '1em', textDecoration: 'none' },
  { tag: tags.heading6, fontSize: '1em', textDecoration: 'none' },
  {
    tag: tags.monospace,
    fontFamily: 'ui-monospace, "SF Mono", SFMono-Regular, Menlo, monospace',
  },
]);

/**
 * The atomic hidden range — collapsed to zero visible width. Atomic by
 * default because Decoration.replace with no widget produces a non-pointable
 * range; cursor navigation skips it, selection treats it as one unit.
 */
const hiddenMark = Decoration.replace({});

/**
 * Decoration.mark applied to the entire InlineCode node. Together with the
 * monospace HighlightStyle on `tags.monospace`, this gives inline `` `code` ``
 * a Slack-style chip: tinted background, orange text, small padding, rounded.
 * The class wraps both the (hidden) backticks AND the code body so the
 * padding visually hugs only the visible characters.
 */
const inlineCodeChip = Decoration.mark({ class: 'cm-md-inline-code' });

/**
 * Decoration.line applied to every line inside a FencedCode block. CSS gives
 * the line a tinted background so the multi-line block reads as a Slack-
 * style code box. The fences themselves (now hidden) leave their lines
 * empty-but-styled at top and bottom — a small visual leading/trailing
 * gap that ends up reading as the box's padding.
 */
const fencedCodeLine = Decoration.line({ class: 'cm-md-fenced-line' });

/**
 * Decoration.line applied to every line inside a Blockquote. CSS gives the
 * line a left accent bar + indent so quoted prose reads as Slack-style
 * quote (instead of the raw `> ` source we shipped in v1). The `>` markers
 * themselves get hidden by LINE_START_MARKERS, so the user sees the
 * indented quoted text with no source decoration.
 */
const blockquoteLine = Decoration.line({ class: 'cm-md-blockquote-line' });

/**
 * Hanging-indent line decoration for list items. Lezer-markdown emits a
 * ListItem node whose range covers every visual line of one bullet, but
 * CodeMirror's `EditorView.lineWrapping` extension wraps each editor line
 * flush against `.cm-content`'s left padding — so a long bullet that
 * wraps visually has rows 2+ at column 0, breaking alignment with where
 * row 1's text begins (after the `- ` marker).
 *
 * We fix that with the classic CSS hang-indent: `padding-left` applies
 * to every visual row INCLUDING soft-wrapped rows; the matching negative
 * `text-indent` applies only to the first visual row, pulling the bullet
 * back to column 0. The `--md-list-depth` custom property scales the
 * padding per nesting level so nested bullets stay aligned under their
 * parent's content.
 *
 * Cached by depth so a doc with hundreds of bullet lines doesn't allocate
 * a fresh Decoration.line object for each one.
 */
const listLineDecoCache = new Map<number, Decoration>();
function listItemLineDeco(depth: number): Decoration {
  let d = listLineDecoCache.get(depth);
  if (!d) {
    d = Decoration.line({
      attributes: {
        class: 'cm-md-list-line',
        style: `--md-list-depth: ${depth};`,
      },
    });
    listLineDecoCache.set(depth, d);
  }
  return d;
}

/**
 * Inline marker tokens — hidden as-is. These are adjacent to the content
 * they wrap, with no trailing whitespace to swallow.
 *
 *   EmphasisMark     `**` of bold, `*` of italic (and `__`/`_` variants)
 *   StrikethroughMark `~~` of strikethrough (GFM)
 *   LinkMark         `[`, `]`, `(`, `)` punctuation around link text
 */
const INLINE_MARKERS = new Set([
  'EmphasisMark',
  'StrikethroughMark',
  'LinkMark',
]);

/**
 * Line-start marker tokens that get hidden — Lezer's grammar emits these
 * covering just the marker character(s), NOT the trailing space that
 * separates the marker from the line's content. Without extending the
 * hidden range over that space, `# Heading` would render as ` Heading`
 * (visible leading space). The hidden range gets bumped forward by one
 * character when it's a space.
 *
 *   HeaderMark   `#`(s) on ATX headings (1-6 hashes)
 *   QuoteMark    `>` at the start of a blockquote line (nested or not).
 *                The Blockquote node also emits a line decoration that
 *                gives the line a left accent bar — together they replace
 *                the raw `> ` source with a Slack-style quote treatment.
 *
 * ListMark (`-` / `1.`) is NOT in this set — bullets and numbered lists
 * stay visible until widget treatment lands in Day 8-10 (bullet glyph +
 * clickable task checkbox). Hiding the list marker without a widget
 * substitute would erase the visual signal that "this is a list."
 */
const LINE_START_MARKERS = new Set([
  'HeaderMark',
  'QuoteMark',
]);

/**
 * Walk the syntax tree across the visible viewport and emit
 * Decoration.replace ranges for every marker token we want to hide.
 *
 * Returns a sorted DecorationSet (RangeSetBuilder guarantees order as long
 * as we add ranges in document order, which the tree's `enter` callback
 * does naturally).
 */
/**
 * Collect the `from`-`to` ranges of all Link / Image nodes that contain
 * any selection endpoint. Markers inside these ranges stay visible so
 * the user can author / edit the URL portion.
 */
function findActiveLinkOrImageRanges(view: EditorView): { from: number; to: number }[] {
  const ranges: { from: number; to: number }[] = [];
  const selection = view.state.selection;
  // Collect selection endpoints (multi-selection handled defensively;
  // single-selection is the realistic case).
  const positions: number[] = [];
  for (const r of selection.ranges) {
    positions.push(r.from);
    if (r.to !== r.from) positions.push(r.to);
  }
  if (positions.length === 0) return ranges;

  // For each visible range, walk the tree to find Link / Image nodes
  // whose span contains any selection endpoint.
  for (const { from, to } of view.visibleRanges) {
    syntaxTree(view.state).iterate({
      from,
      to,
      enter(node) {
        if (node.name !== 'Link' && node.name !== 'Image') return;
        for (const pos of positions) {
          if (pos >= node.from && pos <= node.to) {
            ranges.push({ from: node.from, to: node.to });
            return; // matched; no need to check other positions
          }
        }
      },
    });
  }
  return ranges;
}

function isInsideActiveLink(
  nodeFrom: number,
  nodeTo: number,
  activeRanges: { from: number; to: number }[]
): boolean {
  for (const r of activeRanges) {
    if (nodeFrom >= r.from && nodeTo <= r.to) return true;
  }
  return false;
}

/**
 * Bullet glyph widget. Replaces the `-` (or `*` / `+`) ListMark on a
 * BulletList line with a `•` rendered in the same line-height. The
 * trailing space after the marker is left visible so the gap between
 * bullet and content stays natural. Numbered lists are left alone — the
 * digits are readable and replacing them would erase the ordering signal.
 *
 * Renders as a span (not a button) — bullets aren't interactive. The
 * `aria-hidden` keeps screen readers from announcing decoration when
 * they'd already announce "list item." Pure presentation.
 */
class BulletWidget extends WidgetType {
  eq(other: WidgetType): boolean {
    return other instanceof BulletWidget;
  }
  toDOM(): HTMLElement {
    const span = document.createElement('span');
    span.className = 'cm-md-bullet';
    span.textContent = '•';
    span.setAttribute('aria-hidden', 'true');
    return span;
  }
  ignoreEvent(): boolean {
    return true;
  }
}

/**
 * Numbered-list marker widget. Mirrors `BulletWidget`: replaces the
 * source `1.` / `2.` / `12.` etc. with a `<span class="cm-md-list-num">`
 * whose text content is the same marker. The replacement is functionally
 * a no-op on rendered text, but it gives us full CSS control over the
 * span (matching `.cm-md-bullet`) and bypasses CodeMirror's default
 * highlight style, which colors `tags.processingInstruction` so dimly
 * that the digits become unreadable on the dark theme. Without the
 * widget swap, any class- or attribute-based color rule we ship gets
 * out-cascaded by the highlight style's CSS.
 *
 * `eq()` compares the marker text so the widget DOM rebuilds when the
 * digit changes (e.g. an item gets inserted earlier in the list and
 * everything renumbers).
 */
class OrderedListMarkerWidget extends WidgetType {
  constructor(public marker: string) {
    super();
  }
  eq(other: WidgetType): boolean {
    return other instanceof OrderedListMarkerWidget && other.marker === this.marker;
  }
  toDOM(): HTMLElement {
    const span = document.createElement('span');
    span.className = 'cm-md-list-num';
    span.textContent = this.marker;
    return span;
  }
  ignoreEvent(): boolean {
    return true;
  }
}

/**
 * Task checkbox widget. Replaces the 3-char `[ ]` / `[x]` TaskMarker
 * with a clickable square that toggles the underlying markdown text.
 *
 * Click → dispatches a doc transaction directly on the editor view
 * (the widget receives `view` via `toDOM(view)`). Replaces the 3-char
 * range with `[ ]` or `[x]` depending on the new state. The next update
 * tick re-runs `buildLivePreviewDecorations`, which sees the changed
 * marker text and renders a fresh widget with the new state. The
 * sibling `cm-md-task-done` mark decoration applies a strikethrough
 * across the task content when checked — driven by the same marker text
 * so it stays consistent.
 *
 * `eq()` includes the position fields so the widget DOM rebuilds when
 * the task line shifts. Same lesson as the date chip: without that,
 * click handlers in reused widgets carry stale offsets.
 */
class TaskCheckboxWidget extends WidgetType {
  constructor(
    readonly checked: boolean,
    readonly from: number,
    readonly to: number
  ) {
    super();
  }
  eq(other: WidgetType): boolean {
    return (
      other instanceof TaskCheckboxWidget &&
      other.checked === this.checked &&
      other.from === this.from &&
      other.to === this.to
    );
  }
  toDOM(view: EditorView): HTMLElement {
    const btn = document.createElement('button');
    btn.type = 'button';
    // `checkbox-square` carries the visual (see app.css); `cm-md-task`
    // is the editor-local marker for margin/baseline tweaks and any
    // future editor-only behaviours.
    btn.className = 'cm-md-task checkbox-square';
    btn.setAttribute('role', 'checkbox');
    btn.setAttribute('aria-checked', this.checked ? 'true' : 'false');
    btn.setAttribute(
      'aria-label',
      this.checked ? 'Done. Click to mark not done.' : 'Click to mark done.'
    );
    // SVG check glyph; visible only when checked via CSS opacity rule.
    btn.innerHTML =
      '<svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polyline points="5 12 10 17 19 7"/></svg>';

    // Stop CodeMirror from treating the mousedown as a cursor placement.
    btn.addEventListener('mousedown', (e) => {
      e.preventDefault();
      e.stopPropagation();
    });
    btn.addEventListener('click', (e) => {
      e.preventDefault();
      e.stopPropagation();
      // Read CURRENT marker text from the doc so we never write a stale
      // toggle if something else edited the line between widget mount
      // and click. The marker is 3 chars at positions [from, to).
      if (this.to > view.state.doc.length) return;
      const currentText = view.state.doc.sliceString(this.from, this.to);
      if (!/^\[[ xX]\]$/.test(currentText)) return; // doc moved on us; bail
      const next = this.checked ? '[ ]' : '[x]';
      view.dispatch({
        changes: { from: this.from, to: this.to, insert: next },
        userEvent: 'input.toggle.task',
      });
    });

    return btn;
  }
  ignoreEvent(): boolean {
    return true;
  }
}

const taskDoneMark = Decoration.mark({ class: 'cm-md-task-done' });

/**
 * Internal: collect all decorations as { from, to, deco } tuples, then sort
 * before building the RangeSet. Sorting is required because the tree-walk
 * doesn't visit nodes in strictly-increasing-from order when we emit both
 * `Decoration.line` (at line.from positions) and `Decoration.replace` (at
 * marker positions inside the same line) AND `Decoration.mark` (chip
 * spanning the whole InlineCode node, which starts at the same `from` as
 * its first child CodeMark replace).
 *
 * Sort key: (from asc, deco.startSide asc). CodeMirror's RangeSetBuilder
 * REQUIRES this exact ordering — if two ranges share a `from` and we add
 * them in opposite startSide order, the builder THROWS
 * ("Ranges must be added sorted by `from` position and `startSide`"),
 * which CodeMirror catches at the facet boundary and renders zero
 * decorations for the entire view. The symptom: monospace font applies
 * (HighlightStyle survives) but NO marker-hiding and NO chip class.
 *
 * Reference startSide values (from @codemirror/view/dist/index.js):
 *   - LineDecoration:  -200_000_000   (Side.Line)
 *   - PointDecoration (replace, non-inclusive default):  499_999_999  (Side.NonIncStart - 1)
 *   - MarkDecoration (non-inclusive default):            500_000_000  (Side.NonIncStart)
 *
 * Sorting by the actual `deco.startSide` (instead of a hand-rolled rank
 * constant) means we can't get this wrong as we add new decoration types
 * — the value lives on the Decoration itself, exposed on the public
 * RangeValue.startSide property in @codemirror/state.
 */
type Range = { from: number; to: number; deco: Decoration };

function buildLivePreviewDecorations(view: EditorView): DecorationSet {
  const doc = view.state.doc;
  const activeLinks = findActiveLinkOrImageRanges(view);
  const out: Range[] = [];

  for (const { from, to } of view.visibleRanges) {
    syntaxTree(view.state).iterate({
      from,
      to,
      enter(node) {
        // Reveal-on-active-Link: skip hiding ANY marker that lives inside
        // a Link or Image whose span contains the current selection. The
        // user is editing that link; they need to see its brackets + URL
        // to author it.
        if (isInsideActiveLink(node.from, node.to, activeLinks)) {
          return;
        }

        // Inline markers — adjacent to content, hide as-is.
        if (INLINE_MARKERS.has(node.name)) {
          out.push({ from: node.from, to: node.to, deco: hiddenMark });
          return;
        }

        // Line-start markers — extend the hidden range over the trailing
        // space so `# Heading` doesn't render as ` Heading` with a stray
        // leading space. Lezer's grammar emits these without the space;
        // bumping the end by one when the next char is space hides it too.
        if (LINE_START_MARKERS.has(node.name)) {
          let endPos = node.to;
          if (endPos < doc.length && doc.sliceString(endPos, endPos + 1) === ' ') {
            endPos += 1;
          }
          out.push({ from: node.from, to: endPos, deco: hiddenMark });
          return;
        }

        // CodeMark — the backticks of inline `code` AND the triple-backtick
        // fences of fenced blocks. Hide unconditionally; CodeMark only
        // appears in those two contexts in @lezer/markdown's grammar
        // (verified in node_modules/@lezer/markdown/dist/index.js), and
        // there's no case where a CodeMark should be visible. Removing
        // the matchContext guard simplifies and avoids any edge case where
        // the cursor-context lookup misses (e.g. through buffer/tree
        // boundaries in Lezer's encoding).
        if (node.name === 'CodeMark') {
          out.push({ from: node.from, to: node.to, deco: hiddenMark });
          return;
        }

        // CodeInfo — the language hint after the opening fence (e.g. `js`
        // in ` ```js `). Hide so the user only sees the rendered code body,
        // not the hint or the fence chars.
        if (node.name === 'CodeInfo') {
          out.push({ from: node.from, to: node.to, deco: hiddenMark });
          return;
        }

        // InlineCode: apply the chip class spanning the whole node (backticks
        // + body). The backticks themselves get hidden by the CodeMark
        // handling above, so the visible chip wraps just the code body.
        if (node.name === 'InlineCode') {
          out.push({ from: node.from, to: node.to, deco: inlineCodeChip });
          return;
        }

        // FencedCode: apply the line-background class to every line within
        // the block. The opening + closing fence LINES end up empty-but-
        // styled (their `\`\`\`` chars are hidden by the CodeMark handler)
        // which gives a small visual gap that reads as the box's padding.
        if (node.name === 'FencedCode') {
          const startLine = doc.lineAt(node.from);
          const endLine = doc.lineAt(node.to);
          for (let i = startLine.number; i <= endLine.number; i++) {
            const line = doc.line(i);
            out.push({
              from: line.from,
              to: line.from,
              deco: fencedCodeLine,
            });
          }
          return;
        }

        // Blockquote: apply the left-bar class to every line within the
        // quote. The QuoteMark children (the `>` chars at line-start) are
        // hidden by LINE_START_MARKERS, so the user sees indented prose
        // with an accent bar on the left — the Slack-style quote look.
        // Nested Blockquotes emit duplicate line decorations on the same
        // lines; CodeMirror tolerates that (the CSS class applies once
        // either way) so we don't dedupe.
        if (node.name === 'Blockquote') {
          const startLine = doc.lineAt(node.from);
          const endLine = doc.lineAt(node.to);
          for (let i = startLine.number; i <= endLine.number; i++) {
            const line = doc.line(i);
            out.push({
              from: line.from,
              to: line.from,
              deco: blockquoteLine,
            });
          }
          return;
        }

        // ListItem: emit a line decoration on every line in the item so
        // wrapped lines hang-indent under the bullet's content. Depth is
        // the count of ancestor BulletList/OrderedList nodes — the CSS
        // multiplies it by the per-level gutter (currently 2ch, matching
        // the bullet glyph + trailing space). Nested ListItems are also
        // visited by `iterate` and emit their own deeper-depth decorations
        // on inner lines; CodeMirror tolerates duplicate Decoration.line
        // on the same line, and the deeper-depth decoration wins by way
        // of inline-style precedence (visited after the parent).
        //
        // Bare `return` (NOT `return false`) — we want descent to continue
        // so ListMark / TaskMarker / inline handlers still fire on the
        // subtree.
        if (node.name === 'ListItem') {
          let depth = 0;
          let p = node.node.parent;
          while (p) {
            if (p.name === 'BulletList' || p.name === 'OrderedList') depth++;
            p = p.parent;
          }
          if (depth < 1) depth = 1;
          const deco = listItemLineDeco(depth);
          const startLine = doc.lineAt(node.from);
          const endLine = doc.lineAt(node.to);
          for (let i = startLine.number; i <= endLine.number; i++) {
            const line = doc.line(i);
            out.push({ from: line.from, to: line.from, deco });
          }
          return;
        }

        // URL: hide when it's the destination of either:
        //   - `[text](url)`-style Link (parent name 'Link'), OR
        //   - `![alt](url)`-style Image (parent name 'Image')
        // Without the Image branch, `![sunset](https://example.com/img.png)`
        // would render as `sunsethttps://example.com/img.png` — alt text
        // glued to URL with no separator, because the link brackets are
        // hidden but the URL stays visible.
        //
        // Bare GFM autolinks (`https://example.com` typed naked) emit a
        // URL node too but at the document root level — we want those
        // visible because the URL text IS the link the user is reading.
        // Autolinks (`<https://example.com>`) have parent Autolink, also
        // visible by design.
        if (node.name === 'URL') {
          if (node.matchContext(['Link']) || node.matchContext(['Image'])) {
            out.push({ from: node.from, to: node.to, deco: hiddenMark });
          }
          return;
        }

        // ListMark on a BulletList line — replace the `-` (or `*`/`+`)
        // with a `•` glyph widget. For OrderedList markers (digits +
        // dot) — replace with a same-text widget so we get a custom
        // span to style; the widget's text content matches the source
        // so the visible marker doesn't change, but the wrapping span
        // gets `.cm-md-list-num` for direct CSS control. Replacing the
        // source (rather than wrapping it via Decoration.mark) is what
        // gives us the cascade authority over the digits — the same
        // pattern that makes the bullet widget work.
        //
        // Task list items (`- [ ] foo`): suppress the bullet entirely.
        // The TaskMarker handler below replaces `[ ]` with a clickable
        // checkbox, which IS the visual marker — keeping the `•` too
        // produces a confusing double-marker. Detect by walking the
        // ListItem's children for a Task node.
        //
        // Walk parent chain: ListMark → ListItem → BulletList | OrderedList.
        if (node.name === 'ListMark') {
          const listItem = node.node.parent;
          const list = listItem ? listItem.parent : null;
          if (list && list.name === 'BulletList') {
            // For task list items (`- [ ] foo`), the checkbox below is
            // the visual marker — but we still need to HIDE the source
            // `-` so the user doesn't see "- ☐ foo". Decoration.replace
            // with no widget hides the range without inserting anything.
            // For ordinary bullets, swap the `-` for a `•` glyph widget.
            const isTask = !!listItem && !!listItem.getChild('Task');
            out.push({
              from: node.from,
              to: node.to,
              deco: isTask
                ? Decoration.replace({})
                : Decoration.replace({ widget: new BulletWidget() }),
            });
          } else if (list && list.name === 'OrderedList') {
            const markerText = doc.sliceString(node.from, node.to);
            out.push({
              from: node.from,
              to: node.to,
              deco: Decoration.replace({
                widget: new OrderedListMarkerWidget(markerText),
              }),
            });
          }
          return;
        }

        // TaskMarker (`[ ]` or `[x]`): replace with a clickable checkbox
        // widget. Also emit a sibling `cm-md-task-done` Decoration.mark
        // across the Task body (from just after the marker's trailing
        // space to the end of the Task node) when checked — gives the
        // strikethrough + muted treatment to the rest of the line.
        if (node.name === 'TaskMarker') {
          const markerText = doc.sliceString(node.from, node.to);
          const checked = /\[[xX]\]/.test(markerText);
          out.push({
            from: node.from,
            to: node.to,
            deco: Decoration.replace({
              widget: new TaskCheckboxWidget(checked, node.from, node.to),
            }),
          });
          if (checked) {
            // Walk up to find the Task node so we can mark its body.
            let taskNode: SyntaxNode | null = node.node.parent;
            while (taskNode && taskNode.name !== 'Task') {
              taskNode = taskNode.parent;
            }
            if (taskNode) {
              // Skip one extra char past the marker for the space that
              // separates `[x]` from the body text. Clamp at task end.
              const bodyFrom = Math.min(node.to + 1, taskNode.to);
              if (bodyFrom < taskNode.to) {
                out.push({
                  from: bodyFrom,
                  to: taskNode.to,
                  deco: taskDoneMark,
                });
              }
            }
          }
          return;
        }
      },
    });
  }

  // Sort by (from asc, deco.startSide asc). See the type comment above
  // for why this exact order is mandatory. Within same (from, startSide)
  // — e.g. two replaces of identical sides — fall back to length
  // descending so a wider range is added before any narrower range
  // nested inside it, which RangeSetBuilder also expects when sides match.
  out.sort((a, b) => {
    if (a.from !== b.from) return a.from - b.from;
    if (a.deco.startSide !== b.deco.startSide) {
      return a.deco.startSide - b.deco.startSide;
    }
    return (b.to - b.from) - (a.to - a.from);
  });

  const builder = new RangeSetBuilder<Decoration>();
  for (const r of out) {
    builder.add(r.from, r.to, r.deco);
  }
  return builder.finish();
}

/**
 * ViewPlugin that maintains a DecorationSet for the visible viewport.
 * Rebuilds on doc change, viewport change (scrolling reveals new ranges),
 * AND selection change (the reveal-on-active-Link logic depends on where
 * the cursor is). The selection-change cost is one syntax-tree walk per
 * cursor move — bounded by the viewport — which is fine for editor-scale
 * inputs.
 */
class LivePreviewPlugin implements PluginValue {
  decorations: DecorationSet;

  constructor(view: EditorView) {
    this.decorations = buildLivePreviewDecorations(view);
  }

  update(update: ViewUpdate) {
    if (update.docChanged || update.viewportChanged || update.selectionSet) {
      this.decorations = buildLivePreviewDecorations(update.view);
    }
  }
}

const livePreviewPlugin = ViewPlugin.fromClass(LivePreviewPlugin, {
  decorations: (v) => v.decorations,
});

/**
 * Auto-expand fenced code blocks on Enter.
 *
 * **The problem.** When the user types ` ``` ` on a line and starts typing,
 * @lezer/markdown parses the trailing text as `CodeInfo` (the language hint,
 * e.g. `js` in ` ```js `). Our `CodeInfo` decoration hides it — so to the
 * user it looks like nothing they type appears. Worse, without a closing
 * ` ``` ` below the opening fence, the `FencedCode` node extends to EOF, so
 * there's no line below the visible box for Down-Arrow to land on. The user
 * feels stuck in the fence.
 *
 * **The fix (matches Slack / Typora / Obsidian).** On Enter at the end of
 * a line that is exactly ` ``` ` or ` ```<lang> `, we auto-insert:
 *
 *     <newline>            <- terminates the opening fence line
 *     <empty body line>    <- cursor lands here
 *     <newline>
 *     ```                  <- closing fence
 *     <newline>            <- trailing line below the box; lets Down-Arrow exit
 *
 * The block parses correctly as `FencedCode { CodeMark | CodeMark }` with
 * an empty body line in the middle. Hidden markers + chip/line decorations
 * already in this extension take over from there.
 *
 * **Why a keymap command rather than an `inputHandler`.** Enter is the only
 * trigger we want; intercepting at the keymap layer is the cleanest way to
 * compose with CodeMirror's default Enter behavior — returning `false` from
 * the command falls through to the next binding.
 *
 * **Bail conditions** (return false; default Enter runs):
 *   1. Selection is non-empty (multi-char selection + Enter is a different
 *      operation — let default handle the replace).
 *   2. Cursor isn't at end of line.
 *   3. Line text doesn't match `^```([A-Za-z0-9_-]*)\s*$`. This is
 *      backticks-only (no tildes) — a narrower trigger means fewer surprises.
 *      Blockquote-prefixed (`> ```) and list-prefixed (`-   ```) lines have
 *      their prefix in `line.text` and naturally fail the regex.
 *   4. We're inside a FencedCode block that ALREADY has a closing fence
 *      (i.e. ≥2 `CodeMark` children) — the user is editing an existing
 *      block, not opening a new one.
 */
const expandFencedCodeOnEnter: Command = (view) => {
  // IME composition: WebKit fires keydown with key='Enter' to commit the
  // current candidate (e.g. choosing a kanji from a popup). CM6's keymap
  // facet runs on keydown and does NOT auto-suppress during composition
  // (only `inputHandler` is gated). If we ran the expansion here we'd
  // both swallow the IME commit AND insert a spurious code block. Bail.
  if (view.composing) return false;

  const state = view.state;
  // Single-cursor only. With multiple cursors, dispatching a single-range
  // change at `selection.main.from` displaces the secondary cursors but
  // doesn't expand at THEIR positions — leaving half-expanded fences
  // across the doc. Default Enter handles multi-cursor coherently; defer.
  if (state.selection.ranges.length > 1) return false;

  const sel = state.selection.main;
  if (!sel.empty) return false;

  const line = state.doc.lineAt(sel.from);
  if (sel.from !== line.to) return false;

  const match = line.text.match(/^```([A-Za-z0-9_-]*)\s*$/);
  if (!match) return false;

  // Climb the syntax tree to find a FencedCode ancestor. If one exists AND
  // contains ≥2 CodeMark children, the block is already complete — bail.
  // Use line.from (not sel.from) for resolveInner so we land at the start
  // of the line where the FencedCode node opens; side=1 prefers the node
  // that STARTS at this position rather than one that just ends here.
  //
  // The CodeMark-count walk is safe to do over all FencedCode descendants
  // because @lezer/markdown does NOT recurse inline parsing inside a
  // fenced body — FencedCode's only child types are CodeMark, CodeInfo,
  // CodeText. So nested InlineCode (which would also emit CodeMark
  // children) cannot appear inside a FencedCode block. Verified against
  // node_modules/@lezer/markdown/dist/index.js lines 450-489 (FencedCode
  // builder) vs lines 1416-1418 (InlineCode emission — inline scope only).
  let node: SyntaxNode | null = syntaxTree(state).resolveInner(line.from, 1);
  while (node && node.name !== 'FencedCode') {
    node = node.parent;
  }
  if (node) {
    let codeMarkCount = 0;
    node.cursor().iterate((n) => {
      if (n.name === 'CodeMark') codeMarkCount++;
    });
    if (codeMarkCount >= 2) return false;
  }

  // Expansion payload. Cursor lands at sel.from + 1 = the start of the new
  // empty body line (one position past the first inserted newline).
  //
  // `userEvent: 'input.type.fence'` — deliberately NOT 'input.complete.*'
  // because that prefix is CM6's documented namespace for autocompletion
  // (snippet plugins filter on it). This is a structural template
  // expansion, not an autocomplete acceptance; using the 'input.type.*'
  // namespace keeps it out of those filters while still grouping under
  // the broad 'input' user-event hierarchy that the history extension
  // recognizes for undo coalescing.
  const insert = '\n\n```\n';
  view.dispatch({
    changes: { from: sel.from, insert },
    selection: { anchor: sel.from + 1 },
    userEvent: 'input.type.fence',
  });
  return true;
};

/**
 * Wrap in `Prec.high()` so this Enter binding runs BEFORE @codemirror/commands'
 * `defaultKeymap` Enter handler (`insertNewlineAndIndent`, which always
 * returns true and would otherwise swallow Enter before our command even
 * sees it). The MarkdownEditor.svelte extension array adds defaultKeymap
 * EARLIER in declaration order, so at equal precedence defaultKeymap would
 * win. Prec.high lifts us above default-precedence extensions; we still
 * return `false` for non-matching lines so defaultKeymap's Enter takes
 * over for every other use case (normal newlines, indented newlines, etc).
 */
const fencedCodeAutoExpand: Extension = Prec.high(
  keymap.of([{ key: 'Enter', run: expandFencedCodeOnEnter }])
);

/**
 * Handle Backspace at the start of the first body line of a fenced block.
 *
 * The default backspace at column 0 merges the current line into the line
 * above. For the first body line, that line above is the opening fence —
 * merging breaks the fence and strands the cursor on the now-malformed
 * opening line. This command intercepts that exact case and does the
 * right thing instead:
 *
 *   - **Empty fence** (single empty body line): delete the entire fence —
 *     opening + body + closing + their newlines — and leave the cursor
 *     where the fence used to start. "Get me out of here, this was a
 *     false start."
 *   - **Non-empty fence** (any body line has content, or multiple body
 *     lines exist): treat like Up Arrow — move the cursor to the end of
 *     the line above the box. If the box is flush against the top of the
 *     doc, insert a blank line above and land there (mirrors the
 *     fenceLineCursorSkip behavior).
 *
 * Bail conditions match the other fence commands: IME composition,
 * multi-cursor, non-empty selection, cursor not at start of line, no
 * FencedCode ancestor, or incomplete block. Critically also bails if the
 * cursor isn't on the FIRST body line — backspace on later body lines
 * merges with the previous body line (normal text editing) and shouldn't
 * be intercepted.
 */
const handleBackspaceInFenceBody: Command = (view) => {
  if (view.composing) return false;

  const state = view.state;
  if (state.selection.ranges.length > 1) return false;
  const sel = state.selection.main;
  if (!sel.empty) return false;

  const line = state.doc.lineAt(sel.from);
  if (sel.from !== line.from) return false;

  let node: SyntaxNode | null = syntaxTree(state).resolveInner(line.from, 1);
  while (node && node.name !== 'FencedCode') {
    node = node.parent;
  }
  if (!node) return false;

  let codeMarkCount = 0;
  node.cursor().iterate((n) => {
    if (n.name === 'CodeMark') codeMarkCount++;
  });
  if (codeMarkCount < 2) return false;

  const openLine = state.doc.lineAt(node.from);
  const closeLine = state.doc.lineAt(node.to);

  if (line.number !== openLine.number + 1) return false;

  // "Empty fence" = exactly one body line AND that line has no content.
  // Multiple empty body lines or any content → treat as non-empty so we
  // exit upward rather than nuking a structured block.
  const bodyLineCount = closeLine.number - openLine.number - 1;
  const fenceIsEmpty = bodyLineCount === 1 && line.text === '';

  if (fenceIsEmpty) {
    // Delete the whole fence. Range spans opening fence line through the
    // newline terminator of the closing fence (when one exists — fence at
    // EOF without trailing newline skips that +1).
    const deleteFrom = openLine.from;
    let deleteTo = closeLine.to;
    if (closeLine.number < state.doc.lines) {
      deleteTo += 1;
    }
    view.dispatch({
      changes: { from: deleteFrom, to: deleteTo, insert: '' },
      selection: { anchor: deleteFrom },
      userEvent: 'delete.backward.fence',
    });
    return true;
  }

  // Non-empty fence: exit upward.
  if (openLine.number === 1) {
    view.dispatch({
      changes: { from: 0, insert: '\n' },
      selection: { anchor: 0 },
      userEvent: 'input.type.fenceexit',
    });
  } else {
    const lineAbove = state.doc.line(openLine.number - 1);
    view.dispatch({
      selection: { anchor: lineAbove.to },
      userEvent: 'select.fenceexit',
    });
  }
  return true;
};

const fencedCodeBackspaceHandler: Extension = Prec.high(
  keymap.of([{ key: 'Backspace', run: handleBackspaceInFenceBody }])
);

/**
 * Smart trailing-space escape for inline wraps.
 *
 * **The problem.** CommonMark forbids whitespace adjacent to the inner
 * delimiter of an Emphasis / Strikethrough run. So typing a space at the
 * boundary `**bold|**` produces `**bold **`, which Lezer no longer parses
 * as Emphasis — the asterisks become literal text. With live-preview
 * hiding markers on a parsed Emphasis, the user's experience is:
 *
 *   1. They have **bold** rendered as `bold`.
 *   2. They place the cursor at end of `bold` (just before the hidden
 *      closing `**`).
 *   3. They type a space.
 *   4. The Emphasis dies; `**bold **` parses as literal text and the
 *      asterisks suddenly become visible. "I typed space and four
 *      asterisks appeared."
 *
 * **The fix.** Intercept the space keystroke when the cursor is at the
 * left edge of a closing EmphasisMark or StrikethroughMark, and insert
 * the space AFTER the closing mark instead of inside the wrap. The wrap
 * stays intact; the space lands outside as a normal word boundary.
 *
 * Only applies to Emphasis and Strikethrough — InlineCode doesn't have
 * the trailing-space-breaks-parse issue (whitespace inside backticks is
 * fine in CommonMark), and Link/Image are handled by their own
 * reveal-on-active logic.
 *
 * Bails on non-empty selections (typing-with-selection is a replace
 * semantic, different intent).
 */
const escapeWrapSpace: Extension = EditorView.inputHandler.of(
  (view, from, to, text) => {
    if (text !== ' ') return false;
    if (from !== to) return false;

    // Look for an Emphasis/Strikethrough mark that STARTS at the cursor.
    // resolveInner(from, 1) biases toward a node starting at the position,
    // so this catches the boundary "cursor just before closing marker."
    const node = syntaxTree(view.state).resolveInner(from, 1);
    if (
      (node.name !== 'EmphasisMark' && node.name !== 'StrikethroughMark') ||
      node.from !== from
    ) {
      return false;
    }
    // Distinguish closing mark from opening: parent.from < cursor means
    // we've passed the opening (we're at the closing position).
    const parent = node.parent;
    if (!parent || parent.from >= from) return false;

    view.dispatch({
      changes: { from: node.to, to: node.to, insert: ' ' },
      selection: { anchor: node.to + 1 },
      userEvent: 'input.type.escapewrap',
    });
    return true;
  }
);

/**
 * Auto-expand fenced code blocks the moment the user types the THIRD
 * backtick.
 *
 * Without this, the user types ` ``` ` and sees the box appear, but typing
 * more text becomes hidden CodeInfo (the language hint) — feels broken.
 * With this, the third backtick keystroke completes the block in one shot:
 *
 *   Before (line: "``", cursor at end):
 *     ``[cursor]
 *   User types "`":
 *     ```
 *     [cursor on empty body line]
 *     ```
 *     [trailing blank]
 *
 * **Why a transactionFilter and not an inputHandler.** inputHandler runs
 * BEFORE the change is applied to the doc and would force us to compose
 * "insert the backtick AND the expansion" in one transaction by hand —
 * fragile when other extensions also touch the input. transactionFilter
 * sees the change AFTER it's been resolved into a Transaction, can verify
 * "yes the line is now exactly ``` and that change was a typed backtick",
 * and returns a chained spec that adds the expansion. The merge engine
 * composes both into a single transaction so Cmd+Z reverts both in one
 * step.
 *
 * **Why `sequential: true`.** Without it, the additional spec's `changes`
 * positions are interpreted in the BASE state (pre-tr) and get mapped
 * through tr's changes. With it, positions are interpreted in the POST-tr
 * state where `sel.from` is meaningful — much easier to reason about and
 * matches our mental model ("insert AFTER the third backtick has landed").
 *
 * Bail conditions:
 *   - Not a typing transaction (`input.type.*`) — paste, drop, programmatic
 *     dispatch, etc. don't auto-expand; the Enter handler still works as
 *     fallback for those.
 *   - Multi-cursor or non-empty selection — same reasoning as the Enter
 *     handler.
 *   - The inserted character wasn't a single backtick (could be a paste
 *     that happens to end in backticks; we want only the typed-one-by-one
 *     case).
 *   - The current line's text isn't exactly ` ``` ` post-tr — e.g. user
 *     typed a backtick in the middle of "hello" making "hello`" or
 *     somewhere that doesn't form a clean fence opener.
 *   - The line is already inside a COMPLETE FencedCode block (≥2 CodeMarks)
 *     — the typed backtick is doing something else (e.g. inline code body
 *     character), don't redirect.
 */
const autoExpandOnThirdBacktick: Extension = EditorState.transactionFilter.of(
  (tr) => {
    if (!tr.docChanged) return tr;
    if (!tr.isUserEvent('input.type')) return tr;

    const newState = tr.state;
    if (newState.selection.ranges.length > 1) return tr;
    const sel = newState.selection.main;
    if (!sel.empty) return tr;

    const line = newState.doc.lineAt(sel.from);
    if (line.text !== '```' || sel.from !== line.to) return tr;

    // Confirm the change was a single-backtick insertion ending at sel.from.
    // Guards against pastes that happen to produce "```" as their final
    // form (those should not auto-expand silently — let the user decide).
    let typedBacktickAtCursor = false;
    tr.changes.iterChanges((_fromA, _toA, _fromB, toB, ins) => {
      if (ins.toString() === '`' && toB === sel.from) {
        typedBacktickAtCursor = true;
      }
    });
    if (!typedBacktickAtCursor) return tr;

    // Don't auto-expand if this line is already inside a complete FencedCode.
    let node: SyntaxNode | null = syntaxTree(newState).resolveInner(
      line.from,
      1
    );
    while (node && node.name !== 'FencedCode') {
      node = node.parent;
    }
    if (node) {
      let codeMarkCount = 0;
      node.cursor().iterate((n) => {
        if (n.name === 'CodeMark') codeMarkCount++;
      });
      if (codeMarkCount >= 2) return tr;
    }

    return [
      tr,
      {
        sequential: true,
        changes: { from: sel.from, insert: '\n\n```\n' },
        selection: { anchor: sel.from + 1 },
        userEvent: 'input.type.fence',
      },
    ];
  }
);

/**
 * Cursor-skip filter for fenced code block fence lines.
 *
 * After live-preview hides the opening + closing ` ``` ` markers, the fence
 * lines look like empty body lines to the user — but they're not editable.
 * Typing on the opening line creates a `CodeInfo` (language hint) that we
 * hide → "nothing happens." Typing on the closing line breaks the closing
 * fence → the box extends indefinitely with no escape route.
 *
 * This filter intercepts cursor-only transactions and redirects any
 * selection landing on a fence line of a COMPLETE FencedCode block (≥2
 * CodeMark children). The direction-aware logic:
 *
 *   - **Click on a fence line** (`select.pointer`): land on the nearest body
 *     line. The user assumed the empty-looking line was editable; honor that.
 *   - **Arrow key crossing onto opening fence from body**: continue upward
 *     past the box (the user is trying to exit).
 *   - **Arrow key crossing onto opening fence from above**: land on the
 *     first body line (the user is entering the box).
 *   - **Arrow key crossing onto closing fence from body**: continue downward
 *     past the box.
 *   - **Arrow key crossing onto closing fence from below**: land on the last
 *     body line.
 *   - **Can't exit because the doc has no line outside the box**: keep the
 *     cursor on the body line where it was (no-op the arrow press, same as
 *     pressing Up at the top of any editor).
 *
 * Bails on:
 *   - `tr.docChanged` — typing/paste advances the cursor as a side effect of
 *     change mapping, not a navigation. If typing pushes the cursor onto a
 *     fence line (e.g. Backspace at start of body line), the parse is
 *     already broken; redirecting the cursor would obscure that and the
 *     user can Cmd+Z. (V1 limitation; a future iteration could intercept
 *     Backspace-at-body-start specifically.)
 *   - Multi-cursor (`ranges.length > 1`) — we'd need to redirect every
 *     range or none; bailing matches the auto-expand command's posture and
 *     avoids leaving secondary cursors parked on fence lines.
 *   - **Large jumps** (line delta > 1, non-pointer) — Cmd+Home, Cmd+End,
 *     Cmd+F next-match, Cmd+G goto-line, Page Up/Down etc. All explicit
 *     "take me to position X" actions where the user has chosen the
 *     destination. Overriding them would hide search results, break
 *     goto-line, and undermine boundary navigation.
 *   - Incomplete FencedCode (< 2 CodeMarks) — only completed blocks have a
 *     fence-vs-body distinction worth enforcing.
 *
 * The chained `[tr, {selection: ...}]` return composes into a single merged
 * transaction (verified via @codemirror/state mergeTransaction at lines
 * 2377-2462) — Cmd+Z undoes the whole move as one step, and the filter
 * doesn't recurse on its own selection-override.
 */
const fenceLineCursorSkip: Extension = EditorState.transactionFilter.of((tr) => {
  if (!tr.selection) return tr;
  if (tr.docChanged) return tr;

  if (tr.newSelection.ranges.length > 1) return tr;
  const newSel = tr.newSelection.main;
  if (!newSel.empty) return tr;

  const newState = tr.state;
  const newLine = newState.doc.lineAt(newSel.from);

  let node: SyntaxNode | null = syntaxTree(newState).resolveInner(newLine.from, 1);
  while (node && node.name !== 'FencedCode') {
    node = node.parent;
  }
  if (!node) return tr;

  let codeMarkCount = 0;
  node.cursor().iterate((n) => {
    if (n.name === 'CodeMark') codeMarkCount++;
  });
  if (codeMarkCount < 2) return tr;

  const openLine = newState.doc.lineAt(node.from);
  const closeLine = newState.doc.lineAt(node.to);

  const onOpen = newLine.number === openLine.number;
  const onClose = newLine.number === closeLine.number;
  if (!onOpen && !onClose) return tr;

  const isPointer = tr.isUserEvent('select.pointer');
  const oldLineNum = tr.startState.doc.lineAt(
    tr.startState.selection.main.from
  ).number;

  // Large-jump guard: arrow keys + left/right line-boundary moves produce
  // delta=1; pointer clicks can produce any delta and are handled separately.
  // Anything else (Cmd+Home/End, search results, goto-line, Page Up/Down) is
  // an explicit destination move — leave it alone.
  const lineDelta = Math.abs(newLine.number - oldLineNum);
  if (!isPointer && lineDelta > 1) return tr;

  let targetLineNum: number;
  if (onOpen) {
    if (isPointer) {
      targetLineNum = openLine.number + 1;
    } else if (oldLineNum > openLine.number) {
      targetLineNum = openLine.number - 1;
    } else {
      targetLineNum = openLine.number + 1;
    }
  } else {
    if (isPointer) {
      targetLineNum = closeLine.number - 1;
    } else if (oldLineNum < closeLine.number) {
      targetLineNum = closeLine.number + 1;
    } else {
      targetLineNum = closeLine.number - 1;
    }
  }

  // Bounds: target falls outside the doc — the user is trying to escape a
  // box that's flush against an edge of the document. INSERT a blank line
  // outside the box at the appropriate edge and land there. This matches
  // the auto-expand's "always leave a trailing line below" posture but
  // applied lazily: only when the user actively needs the line. The
  // alternative (forcing an above/below buffer line on every expand) would
  // be wasteful for blocks that the user never needs to escape from.
  // `sequential: true` makes the inserted change's position resolve in the
  // post-tr state (which is just the selection move; doc is unchanged) —
  // pos 0 / doc.length stay meaningful.
  if (targetLineNum < 1) {
    return [
      tr,
      {
        sequential: true,
        changes: { from: 0, insert: '\n' },
        selection: { anchor: 0 },
        userEvent: 'input.type.fenceexit',
      },
    ];
  }
  if (targetLineNum > newState.doc.lines) {
    const endPos = newState.doc.length;
    return [
      tr,
      {
        sequential: true,
        changes: { from: endPos, insert: '\n' },
        selection: { anchor: endPos + 1 },
        userEvent: 'input.type.fenceexit',
      },
    ];
  }

  const targetLine = newState.doc.line(targetLineNum);
  return [tr, { selection: { anchor: targetLine.from } }];
});

/**
 * The exported extension factory. Bundles:
 *
 *   1. The marker-hiding ViewPlugin.
 *   2. The live-preview HighlightStyle (heading sizes + monospace).
 *   3. The Enter-key auto-expand for fenced code blocks.
 *   4. An `EditorView.atomicRanges` registration that points back at the
 *      same DecorationSet — making cursor commands (arrow keys, backspace,
 *      Home/End, selection drag) treat each hidden range as a single
 *      skip-step instead of stepping through invisible character
 *      positions one-by-one.
 *
 * The atomic-range facet uses a function (not a static set) so it always
 * reads the current plugin's decorations from the view — keeping atomic
 * behavior in sync with what's visually hidden across edits, scrolls, and
 * selection-driven reveals.
 */
export function livePreview(): Extension {
  return [
    livePreviewPlugin,
    syntaxHighlighting(livePreviewStyle),
    fencedCodeAutoExpand,
    autoExpandOnThirdBacktick,
    fencedCodeBackspaceHandler,
    fenceLineCursorSkip,
    escapeWrapSpace,
    EditorView.atomicRanges.of((view) => {
      return view.plugin(livePreviewPlugin)?.decorations ?? Decoration.none;
    }),
    // Inline ISO-date chips. Scans for `YYYY-MM-DD` strings in prose
    // (skipping code spans), renders each as a clickable pill that opens
    // a date picker. Listens for the picker's commit event on `window`
    // to dispatch the actual doc edit.
    dateChip(),
    // Phase 4 — inline link chips. Renders `[text](url)` markdown links
    // as favicon+label pills. Enrichment (favicon + og:title) fetched
    // async in the background via the `enrich_link` Tauri command;
    // cache under `.metadata/link-cache.json`. Plain click opens the
    // URL; Alt-click enters an edit mode (LinkChipEditWidget).
    linkChip(),
  ];
}
