/**
 * CodeMirror 6 extension: render `[text](url)` markdown links as
 * inline pill chips (favicon + label). Plain click opens the URL;
 * Alt-click places the cursor inside the label range so the user can
 * type-to-replace. Enrichment (favicon + og:title) is fetched from
 * the Rust `enrich_link` command and cached under
 * `.metadata/link-cache.json`.
 *
 * ## Design pillars
 *
 * **Storage stays plain.** The doc is standard markdown; the chip is
 * a render-layer treatment only. Email export, tool interop, grep —
 * they all see regular `[text](url)`.
 *
 * **The label is the markdown text.** The `[text]` part IS the chip's
 * label; enrichment only contributes the favicon + tooltip. Alt-click
 * edits the underlying markdown, not "a chip" — same source of truth
 * on both sides.
 *
 * **Skip when the cursor is inside.** Same rule as `date-chip.ts` —
 * if the cursor is strictly between the link's `[` and `)`, hide the
 * chip so the user sees the raw markdown they're editing. When the
 * cursor moves out, the chip reappears.
 *
 * ## The wrap-boundary vanish story
 *
 * An earlier iteration had `eq()` compare the widget's `from`/`to`
 * positions alongside its content. That worked fine for date-chip
 * (short labels, rarely at wrap points) but broke for link chips: any
 * doc edit far from the link shifted its `from`/`to` by the change
 * size, `eq()` returned false, and CM6 tore down + rebuilt the
 * widget's DOM inside its already-laid-out wrapping line box.
 * WebKit sometimes lost the widget during that reflow — the chip
 * vanished, only reappearing when a subsequent measurement pass
 * happened (which the user could trigger by clicking near where the
 * chip "should be").
 *
 * The fix: `eq()` compares content ONLY. When positions shift but
 * content is unchanged, CM6 reuses the existing DOM node — no
 * rebuild, no wrap-boundary reflow, no vanish. Click handlers
 * compute their target position dynamically via `view.posAtDOM(btn)`
 * + a syntax tree walk, so the widget's stale closure positions
 * never matter.
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
import { RangeSetBuilder, StateEffect } from '@codemirror/state';
import { syntaxTree } from '@codemirror/language';
import type { Extension } from '@codemirror/state';
import type { SyntaxNode } from '@lezer/common';
import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';

export type EnrichmentResult = {
  url: string;
  title: string | null;
  siteName: string | null;
  faviconDataUrl: string | null;
  fetchedAt: string;
};

/**
 * Fires when async `enrich_link` resolves and the plugin needs to
 * re-render its decorations with the new favicon. Empty
 * `view.dispatch({})` doesn't work — the plugin's `update()` gate
 * returns false on all three standard flags for a change-less
 * transaction.
 */
const linkChipRefresh = StateEffect.define<{ url: string }>();

const GLOBE_SVG = `<svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>`;

function hostnameOf(url: string): string {
  try {
    return new URL(url).hostname;
  } catch {
    return url;
  }
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

/**
 * Extract text + URL from a Lezer `Link` node. Returns null when the
 * link is malformed (missing bracket / paren / URL) or the URL isn't
 * http/https — anything else stays as raw markdown.
 *
 * `textFrom` + `textTo` are the positions between `[` and `]`. Used
 * by Alt-click to select the label range for type-to-replace.
 */
function extractLinkParts(
  node: SyntaxNode,
  docText: (from: number, to: number) => string,
): { textFrom: number; textTo: number; text: string; url: string } | null {
  if (node.name !== 'Link') return null;

  let firstMark: SyntaxNode | null = null;
  let closingBracket: SyntaxNode | null = null;
  let urlNode: SyntaxNode | null = null;

  let cur = node.firstChild;
  while (cur) {
    if (cur.name === 'LinkMark') {
      if (!firstMark) firstMark = cur;
      else if (!closingBracket) closingBracket = cur;
    } else if (cur.name === 'URL') {
      urlNode = cur;
    }
    cur = cur.nextSibling;
  }
  if (!firstMark || !closingBracket || !urlNode) return null;

  const textFrom = firstMark.to;
  const textTo = closingBracket.from;
  const text = docText(textFrom, textTo);
  const url = docText(urlNode.from, urlNode.to).trim();

  if (!url) return null;
  const scheme = url.slice(0, url.indexOf(':')).toLowerCase();
  if (scheme !== 'http' && scheme !== 'https') return null;

  return { textFrom, textTo, text, url };
}

class LinkChipWidget extends WidgetType {
  constructor(
    readonly text: string,
    readonly url: string,
    /** null = fetch tried, nothing useful. undefined = never fetched. */
    readonly enrichment: EnrichmentResult | null | undefined,
  ) {
    super();
  }

  /**
   * Content-only equality. See module docstring for why positions
   * are deliberately excluded from the comparison.
   */
  eq(other: WidgetType): boolean {
    return (
      other instanceof LinkChipWidget &&
      other.text === this.text &&
      other.url === this.url &&
      (other.enrichment?.faviconDataUrl ?? null) ===
        (this.enrichment?.faviconDataUrl ?? null)
    );
  }

  toDOM(view: EditorView): HTMLElement {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'cm-link-chip';

    // Label precedence: markdown `[text]` → enrichment title →
    // hostname. Empty `[]` (a rare but valid markdown link shape)
    // falls through the chain to something readable.
    const label = this.text.trim()
      || this.enrichment?.title?.trim()
      || hostnameOf(this.url);

    // Icon: a favicon rendered as a `background-image` on a span
    // (NOT an <img>) so it doesn't participate in inline layout as
    // a replaced element with async decoding — dimensions are 100%
    // CSS-driven and CM6 can measure the widget in one pass.
    // Fallback: inline SVG globe.
    const iconHtml = this.enrichment?.faviconDataUrl
      ? `<span class="cm-link-chip-icon" style="background-image: url(${JSON.stringify(
          this.enrichment.faviconDataUrl,
        )})"></span>`
      : GLOBE_SVG;

    // Single innerHTML assignment (matches date-chip's proven
    // pattern). escapeHtml on the label so `<script>` in the
    // markdown source can never become DOM.
    btn.innerHTML = `${iconHtml}<span class="cm-link-chip-text">${escapeHtml(label)}</span>`;

    const site = this.enrichment?.siteName?.trim();
    btn.title = site ? `${site} — ${this.url}` : this.url;
    btn.setAttribute(
      'aria-label',
      `Open link: ${label}. Alt-click to edit label.`,
    );
    btn.setAttribute('data-url', this.url);

    // Stop CM6 from interpreting the mousedown as "place cursor here."
    btn.addEventListener('mousedown', (e) => {
      e.preventDefault();
      e.stopPropagation();
    });

    btn.addEventListener('click', (e) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.altKey) {
        this.enterEditMode(view, btn);
      } else {
        openUrl(this.url).catch((err) => {
          console.error('[link-chip] openUrl failed:', err);
        });
      }
    });

    return btn;
  }

  /**
   * Alt-click handler. Positions are looked up LIVE from the current
   * syntax tree via `view.posAtDOM` — the widget's closure has no
   * `from`/`to` and shouldn't (see module docstring). If the DOM
   * lookup fails or the resolved node isn't a Link (shouldn't
   * happen), the click is silently ignored.
   */
  private enterEditMode(view: EditorView, btn: HTMLElement): void {
    const pos = view.posAtDOM(btn);
    const tree = syntaxTree(view.state);
    let node: SyntaxNode | null = tree.resolveInner(pos, 1);
    while (node && node.name !== 'Link') node = node.parent;
    if (!node) return;
    const parts = extractLinkParts(node, (a, b) =>
      view.state.doc.sliceString(a, b),
    );
    if (!parts) return;
    view.dispatch({
      selection: { anchor: parts.textFrom, head: parts.textTo },
      scrollIntoView: true,
    });
    view.focus();
  }

  ignoreEvent(): boolean {
    return true;
  }
}

/**
 * Per-view enrichment cache. Values:
 *   - `undefined` (absent): never seen. First sight fires invoke.
 *   - `'pending'`: invoke is in flight. Don't dispatch again.
 *   - `EnrichmentResult`: resolved. Fields may all be null for auth-
 *     gated URLs; the widget's label chain handles that.
 *   - `null`: invoke rejected. Skip subsequent invokes for this URL.
 */
type EnrichmentCache = Map<string, EnrichmentResult | 'pending' | null>;

class LinkChipPlugin implements PluginValue {
  decorations: DecorationSet;
  private cache: EnrichmentCache = new Map();

  constructor(view: EditorView) {
    this.decorations = this.build(view);
  }

  update(update: ViewUpdate) {
    // Any of: doc changed, viewport changed, cursor moved, OR our
    // custom enrichment-refresh effect fired. The effect path is
    // load-bearing — an empty dispatch (which is what an "async
    // completion, please re-render" resolution would naturally
    // produce) doesn't fire any of the standard flags.
    let refreshed = false;
    for (const tr of update.transactions) {
      for (const eff of tr.effects) {
        if (eff.is(linkChipRefresh)) {
          refreshed = true;
        }
      }
    }
    if (
      update.docChanged ||
      update.viewportChanged ||
      update.selectionSet ||
      refreshed
    ) {
      this.decorations = this.build(update.view);
    }
  }

  private build(view: EditorView): DecorationSet {
    const builder = new RangeSetBuilder<Decoration>();
    const tree = syntaxTree(view.state);
    const cursorPos = view.state.selection.main.head;
    // A Link node can straddle two visibleRanges; without dedup we'd
    // call builder.add with the same range twice, which throws.
    const seen = new Set<number>();

    for (const { from, to } of view.visibleRanges) {
      tree.iterate({
        from,
        to,
        enter: (node) => {
          if (node.name !== 'Link') return;
          const linkFrom = node.from;
          if (seen.has(linkFrom)) return;
          seen.add(linkFrom);

          const parts = extractLinkParts(node.node, (a, b) =>
            view.state.doc.sliceString(a, b),
          );
          if (!parts) return;

          const linkTo = node.to;
          // Hide chip while the cursor edits inside the range.
          // Strict interior: cursor at either boundary keeps the chip.
          if (cursorPos > linkFrom && cursorPos < linkTo) return;

          this.ensureEnrichment(view, parts.url);
          const cached = this.cache.get(parts.url);
          const enrichment =
            cached && cached !== 'pending' ? cached : undefined;

          builder.add(
            linkFrom,
            linkTo,
            Decoration.replace({
              widget: new LinkChipWidget(parts.text, parts.url, enrichment),
              inclusive: false,
            }),
          );
        },
      });
    }
    return builder.finish();
  }

  private ensureEnrichment(view: EditorView, url: string): void {
    if (this.cache.has(url)) return;
    this.cache.set(url, 'pending');
    invoke<EnrichmentResult>('enrich_link', { url })
      .then((result) => {
        this.cache.set(url, result);
        // Fire the refresh effect so update() rebuilds decorations
        // and the widget re-renders with its favicon.
        view.dispatch({ effects: linkChipRefresh.of({ url }) });
      })
      .catch((err) => {
        console.error('[link-chip] enrich_link failed:', err);
        this.cache.set(url, null);
      });
  }
}

const linkChipPlugin = ViewPlugin.fromClass(LinkChipPlugin, {
  decorations: (v) => v.decorations,
});

/**
 * The exported factory. Bundles the chip ViewPlugin + an atomic-range
 * registration so cursor commands treat each chip as one skip step
 * (arrow keys land at chip boundaries, backspace removes the whole
 * chip). Matches `dateChip()`'s shape.
 */
export function linkChip(): Extension {
  return [
    linkChipPlugin,
    EditorView.atomicRanges.of((view) => {
      return view.plugin(linkChipPlugin)?.decorations ?? Decoration.none;
    }),
  ];
}
