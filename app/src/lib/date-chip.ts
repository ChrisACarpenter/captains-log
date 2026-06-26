/**
 * CodeMirror 6 extension: render ISO date strings (`YYYY-MM-DD`) as
 * Confluence-style inline chips. Click a chip to open a date picker; the
 * picker dispatches a transaction replacing the 10-char range with the
 * new ISO date.
 *
 * ## Architecture
 *
 * **Storage stays plain.** The doc on disk is canonical Markdown — every
 * date is just a literal `2026-06-25` string. Zero migration of Chris's
 * existing weekly files, and the markdown round-trips through any tool
 * (GitHub, Obsidian, copy-paste to Slack) as readable text. The chip is
 * purely a render-layer treatment.
 *
 * **Detection.** A ViewPlugin scans the visible viewport for matches of
 * `\b\d{4}-\d{2}-\d{2}\b`. Matches inside `InlineCode` or `FencedCode`
 * syntax nodes are SKIPPED — those ranges already get the chip/box
 * treatment from live-preview, and stacking a date chip on top would be
 * visual mess (chip-inside-chip) AND wrong semantically (those characters
 * are code, not a date the user wants to edit).
 *
 * **Chip widget.** `Decoration.replace` swaps the 10-char range for a
 * widget that renders a small pill: calendar glyph + formatted date
 * (`Jun 25, 2026`, or `Jun 25` when in-year). Atomic-range registration
 * means cursor commands treat each chip as one skip — arrow keys land at
 * chip boundaries, backspace removes the whole chip.
 *
 * **Click handling.** The widget's button element dispatches a bubbling
 * `CustomEvent('captainslog:date-chip-click', { detail: {from, to, iso,
 * anchorEl} })` on the editor's DOM tree. `MarkdownEditor.svelte`
 * listens for this event and opens the picker popover anchored to the
 * chip's bounding rect. The picker calls back via another event:
 * `CustomEvent('captainslog:date-chip-commit', { detail: {from, to, iso} })`
 * (dispatched on `window`) — the extension listens, dispatches the
 * actual transaction. Decoupling the picker UI (Svelte) from the editor
 * extension (CM6) via DOM events keeps both layers framework-agnostic.
 */

import {
  Decoration,
  EditorView,
  ViewPlugin,
  WidgetType,
  type DecorationSet,
  type PluginValue,
  type ViewUpdate,
} from '@codemirror/view';
import { RangeSetBuilder } from '@codemirror/state';
import { syntaxTree } from '@codemirror/language';
import type { Extension } from '@codemirror/state';
import type { SyntaxNode } from '@lezer/common';

/** Matches strict ISO `YYYY-MM-DD`. Word boundaries on both sides so
 *  digit runs of other lengths don't false-trigger. */
const ISO_DATE_RE = /\b(\d{4})-(\d{2})-(\d{2})\b/g;

/**
 * Format an ISO date for display in the chip. In-year dates drop the
 * year for cleaner reading (`Jun 25`); cross-year keeps it explicit
 * (`Jun 25, 2024`). Uses local-time interpretation so the chip shows
 * the date the user typed, not a timezone-shifted day.
 */
function formatChipText(iso: string): string {
  const [yearStr, monthStr, dayStr] = iso.split('-');
  const year = parseInt(yearStr, 10);
  const month = parseInt(monthStr, 10);
  const day = parseInt(dayStr, 10);
  // Bare Date construction would parse ISO as UTC; build manually so
  // 2026-06-25 stays June 25 regardless of timezone.
  const date = new Date(year, month - 1, day);
  const currentYear = new Date().getFullYear();
  const monthAbbrev = date.toLocaleDateString('en-US', { month: 'short' });
  return year === currentYear
    ? `${monthAbbrev} ${day}`
    : `${monthAbbrev} ${day}, ${year}`;
}

class DateChipWidget extends WidgetType {
  constructor(
    readonly iso: string,
    readonly from: number,
    readonly to: number
  ) {
    super();
  }

  /** Required so CM6 doesn't recreate the DOM on every transaction.
   *  Two widgets are equivalent iff their ISO string AND their cached
   *  positions match. The position check is load-bearing: if we omit it,
   *  text inserted upstream shifts every chip's source range but reuses
   *  the same DOM nodes — and those nodes hold the OLD `from`/`to` in
   *  their click handlers' closures. Picking a new date then commits to
   *  stale coordinates. Including from/to forces DOM rebuild on any
   *  shift, which is cheap (the chip is a button with one SVG + text)
   *  compared to the data-loss risk. */
  eq(other: WidgetType): boolean {
    return (
      other instanceof DateChipWidget &&
      other.iso === this.iso &&
      other.from === this.from &&
      other.to === this.to
    );
  }

  toDOM(): HTMLElement {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'cm-date-chip';
    btn.setAttribute('data-iso', this.iso);
    btn.setAttribute('data-from', String(this.from));
    btn.setAttribute('data-to', String(this.to));
    btn.setAttribute('aria-label', `Date: ${this.iso}. Click to change.`);
    btn.title = this.iso;

    // Inline SVG calendar icon — 12x12 to sit comfortably on the chip's
    // baseline next to the formatted-date text. Stroke uses currentColor
    // so the chip's color rule cascades.
    btn.innerHTML = `<svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="4" width="18" height="18" rx="2"/><line x1="16" y1="2" x2="16" y2="6"/><line x1="8" y1="2" x2="8" y2="6"/><line x1="3" y1="10" x2="21" y2="10"/></svg><span class="cm-date-chip-text">${formatChipText(this.iso)}</span>`;

    btn.addEventListener('mousedown', (e) => {
      // Stop CodeMirror from interpreting this as a click-in-doc that
      // moves the selection — the chip's role is "open the picker," not
      // "place a cursor at this position."
      e.preventDefault();
      e.stopPropagation();
    });
    btn.addEventListener('click', (e) => {
      e.preventDefault();
      e.stopPropagation();
      btn.dispatchEvent(
        new CustomEvent('captainslog:date-chip-click', {
          bubbles: true,
          composed: true,
          detail: {
            from: this.from,
            to: this.to,
            iso: this.iso,
            anchorEl: btn,
          },
        })
      );
    });

    return btn;
  }

  /** Widgets default to non-ignored events, which CM6 then tries to
   *  interpret as cursor moves. Returning true makes CM6 ignore all
   *  events on the widget DOM — our manual handlers above are the only
   *  ones that fire. */
  ignoreEvent(): boolean {
    return true;
  }
}

/**
 * True when the position lies inside a syntax node we don't want to chip:
 * inline code, fenced code, or a code mark. Walks up from the resolved
 * inner node — the date might be nested deeper than the first-level
 * match.
 */
function isInsideCodeContext(view: EditorView, pos: number): boolean {
  let node: SyntaxNode | null = syntaxTree(view.state).resolveInner(pos, 1);
  while (node) {
    const name = node.name;
    if (name === 'InlineCode' || name === 'FencedCode' || name === 'CodeMark') {
      return true;
    }
    node = node.parent;
  }
  return false;
}

function buildDateChipDecorations(view: EditorView): DecorationSet {
  const builder = new RangeSetBuilder<Decoration>();
  for (const { from, to } of view.visibleRanges) {
    const text = view.state.doc.sliceString(from, to);
    let match: RegExpExecArray | null;
    ISO_DATE_RE.lastIndex = 0;
    while ((match = ISO_DATE_RE.exec(text)) !== null) {
      const matchStart = from + match.index;
      const matchEnd = matchStart + match[0].length;
      if (isInsideCodeContext(view, matchStart)) continue;
      // Skip ONLY when the cursor is STRICTLY between the date's
      // first and last character — i.e. the user is editing inside the
      // string. Strict bounds (`> matchStart && < matchEnd`) means a
      // cursor sitting exactly at matchStart or matchEnd still chips
      // immediately. That fixes the fresh-insert case: when
      // `insertCurrentDate` lands the cursor at matchEnd of the just-
      // inserted ISO, the chip pops in immediately instead of waiting
      // for the cursor to move.
      const cursorPos = view.state.selection.main.head;
      if (cursorPos > matchStart && cursorPos < matchEnd) continue;

      const widget = new DateChipWidget(match[0], matchStart, matchEnd);
      builder.add(
        matchStart,
        matchEnd,
        Decoration.replace({ widget, inclusive: false })
      );
    }
  }
  return builder.finish();
}

class DateChipPlugin implements PluginValue {
  decorations: DecorationSet;

  constructor(view: EditorView) {
    this.decorations = buildDateChipDecorations(view);
  }

  update(update: ViewUpdate) {
    if (update.docChanged || update.viewportChanged || update.selectionSet) {
      this.decorations = buildDateChipDecorations(update.view);
    }
  }
}

const dateChipPlugin = ViewPlugin.fromClass(DateChipPlugin, {
  decorations: (v) => v.decorations,
});

/**
 * Validate that the text at [from, to) in `view` still parses as an ISO
 * date. Used by MarkdownEditor's commit handler as a sanity check before
 * dispatching — defensive against the doc having been edited since the
 * picker opened (autosave reload, multi-cursor edits, etc.).
 *
 * Uses a fresh non-global RegExp to avoid the `/g` lastIndex stateful
 * trap that would otherwise make `test()` return alternating results
 * across calls.
 */
export function isValidIsoDateRange(
  view: EditorView,
  from: number,
  to: number
): boolean {
  if (from < 0 || to > view.state.doc.length || from >= to) return false;
  const text = view.state.doc.sliceString(from, to);
  return /^\d{4}-\d{2}-\d{2}$/.test(text);
}

/**
 * The exported factory. Bundles the chip ViewPlugin + an atomic-range
 * registration so cursor commands treat each chip as one skip step.
 *
 * **Routing the commit.** Earlier iterations used a window-level
 * `captainslog:date-chip-commit` event with a global `activeViews` Set,
 * and the listener would pick the first view whose range happened to
 * still hold an ISO. That misroutes commits when /summary's 4 editor
 * instances coexist — the wrong editor receives the change. The current
 * design instead has MarkdownEditor.svelte handle the chip-click event
 * bubbled through its OWN container, open the picker with its OWN view
 * reference stashed, and on commit dispatch the transaction DIRECTLY
 * on that view. No window event, no view-lookup ambiguity.
 */
export function dateChip(): Extension {
  return [
    dateChipPlugin,
    EditorView.atomicRanges.of((view) => {
      return view.plugin(dateChipPlugin)?.decorations ?? Decoration.none;
    }),
  ];
}
