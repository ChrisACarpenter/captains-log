/**
 * CodeMirror 6 extension: paste-a-URL behaviors for the link-chip flow.
 *
 * Two shapes:
 *
 *   1. **Paste URL with a text selection** (Slack pattern). Selection
 *      becomes the label; URL becomes the href. `sel` + paste `url` →
 *      buffer becomes `[sel](url)`. No enrichment fetch — the user's
 *      selection is the label, always.
 *
 *   2. **Paste URL with no (or empty) selection**. Insert the URL
 *      verbatim first — that renders as a GFM autolink so the user
 *      immediately sees "this became a link." Kick off `enrich_link` in
 *      the background; when it resolves, upgrade the bare URL to
 *      `[title](url)` markdown. If the fetch fails or the user has
 *      already typed over the URL, the upgrade is silently skipped and
 *      the URL stays as an autolink.
 *
 * Everything else (multi-line clipboard, non-URL text, image data, or
 * a URL with the selection spanning a newline) falls through to CM6's
 * default paste behavior.
 *
 * Regex intentionally loose — see the module docstring in
 * `link-chip.ts` for why we don't RFC-validate here.
 */

import { EditorView } from '@codemirror/view';
import type { Extension } from '@codemirror/state';
import { invoke } from '@tauri-apps/api/core';
import type { EnrichmentResult } from './link-chip';

/**
 * Trimmed clipboard content is treated as a URL paste when it's ONE
 * http/https URL and nothing else. Matches `https://x.com/...` and
 * rejects `see https://x.com`, multi-line pastes, or a URL followed
 * by other text.
 */
const URL_ONLY_RE = /^https?:\/\/[^\s<>"']+$/;

function isSingleUrlPaste(text: string): boolean {
  return URL_ONLY_RE.test(text);
}

/**
 * Return the URL's hostname for the fallback label, or the URL itself
 * if `new URL()` refuses to parse it. `new URL()` here mirrors the
 * widget's fallback — same failure mode, same result.
 */
function hostnameOf(url: string): string {
  try {
    return new URL(url).hostname;
  } catch {
    return url;
  }
}

/**
 * A markdown-link inner-text is safe when it doesn't need to be
 * escaped to survive `[text](url)` parsing. In practice that means no
 * unescaped `]`. We keep this narrow (don't strip anything the user
 * SELECTED); if the selection has `]`, we punt and fall through to
 * default paste. Better to preserve their selection semantics than to
 * mangle a paste with silent escaping.
 */
function selectionSafeForWrap(text: string): boolean {
  if (!text) return false;
  if (/[\r\n]/.test(text)) return false;
  if (text.includes(']')) return false;
  return true;
}

/**
 * Given the URL just pasted at `expectedPos`, search the doc for the
 * URL string at (or near) that position and upgrade it to
 * `[label](url)` markdown. Bails out silently if the URL isn't there
 * anymore (user typed over it) — the paste flow trusts the user's
 * subsequent edits over the stale enrichment intent.
 *
 * We look at the exact expected position first (fast path); if
 * nothing matches, we search a small window either side to tolerate
 * one or two chars of drift from adjacent typing. Beyond that window
 * we assume the URL has been intentionally modified/removed.
 */
function upgradeUrlToLink(
  view: EditorView,
  expectedPos: number,
  url: string,
  label: string,
): void {
  const docLen = view.state.doc.length;
  const search = view.state.doc.sliceString(
    Math.max(0, expectedPos - 8),
    Math.min(docLen, expectedPos + url.length + 8),
  );
  const localIndex = search.indexOf(url);
  if (localIndex < 0) return;
  const from = Math.max(0, expectedPos - 8) + localIndex;
  const to = from + url.length;

  // Defensive: only upgrade if the doc really does have the raw URL at
  // this range — the slice search above could match false positives if
  // the URL string happens to appear elsewhere in a small drift window.
  const actual = view.state.doc.sliceString(from, to);
  if (actual !== url) return;

  const insert = `[${label}](${url})`;
  view.dispatch({
    changes: { from, to, insert },
    // Preserve the cursor's relative position after the upgrade. If the
    // cursor was just after the URL, keep it just after the closing `)`.
    userEvent: 'input.link-upgrade',
  });
}

export function linkPaste(): Extension {
  return EditorView.domEventHandlers({
    paste(event, view) {
      const raw = event.clipboardData?.getData('text/plain') ?? '';
      const trimmed = raw.trim();

      // Only intercept URL-only pastes. Everything else — including a
      // URL with trailing text, a snippet with a URL in it, or a
      // multi-line clipboard — falls through to default paste.
      if (!isSingleUrlPaste(trimmed)) return false;

      const sel = view.state.selection.main;
      const selText = view.state.doc.sliceString(sel.from, sel.to);

      if (selText && selectionSafeForWrap(selText)) {
        // Slack pattern: wrap selection as `[selText](url)`. No fetch —
        // the label is authoritative. The chip widget will render the
        // favicon once enrichment lands (kicked off by the linkChip
        // ViewPlugin on next re-decoration).
        event.preventDefault();
        const insert = `[${selText}](${trimmed})`;
        view.dispatch({
          changes: { from: sel.from, to: sel.to, insert },
          selection: { anchor: sel.from + insert.length },
          userEvent: 'input.paste',
        });
        return true;
      }

      // No selection (or selection unsafe): insert bare URL, then
      // upgrade async. Bare URL is what CommonMark + GFM autolink
      // renders as a link too — the user is never left staring at
      // plain text.
      event.preventDefault();
      const insertPos = sel.from;
      view.dispatch({
        changes: { from: sel.from, to: sel.to, insert: trimmed },
        selection: { anchor: sel.from + trimmed.length },
        userEvent: 'input.paste',
      });

      invoke<EnrichmentResult>('enrich_link', { url: trimmed })
        .then((result) => {
          const label =
            result.title?.trim()
            || result.siteName?.trim()
            || hostnameOf(trimmed);
          upgradeUrlToLink(view, insertPos, trimmed, label);
        })
        .catch((err) => {
          // Fetch layer error — leave the bare URL alone. Autolink is a
          // fine fallback; the ViewPlugin's cache will mark this URL as
          // "tried, no data" and stop kicking off further enrich calls.
          console.error('[link-paste] enrich_link failed:', err);
        });

      return true;
    },
  });
}
