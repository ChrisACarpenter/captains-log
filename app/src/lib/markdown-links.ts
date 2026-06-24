/**
 * CodeMirror 6 extension: Cmd-click (or Ctrl-click on non-Mac) on a Markdown
 * link follows it via Tauri's opener plugin.
 *
 * ## Why an extension and not a Svelte event handler
 *
 * The editor's DOM is owned by CM6 — Svelte's event delegation doesn't see
 * mousedowns that happen inside .cm-content. CM6 exposes
 * `EditorView.domEventHandlers` for exactly this: it runs the handler in the
 * same scope as the editor's own event listeners, with access to the position
 * the click landed at (via `view.posAtCoords`). From there we resolve the
 * syntax tree to find the surrounding Link / Autolink / URL node and extract
 * the href.
 *
 * ## What gets recognized as a link
 *
 *   - `[text](url)` — Markdown inline link. Lezer emits a `Link` node whose
 *     `URL` child carries the href.
 *   - `<https://example.com>` — CommonMark autolink. Emits an `Autolink` node
 *     whose text (minus the angle brackets) is the href.
 *   - Bare URLs like `https://example.com` — only when GFM autolinking is
 *     enabled (we turn it on in MarkdownEditor.svelte via the GFM extension).
 *     Emits an `Autolink` node too.
 *
 * ## URL safety
 *
 * No URL filtering happens here. The Tauri opener plugin enforces the
 * capability scope (`opener:allow-open-url` in
 * `src-tauri/capabilities/default.json`); anything outside the allow-list
 * (currently `http://*`, `https://*`, `mailto:*`,
 * `x-apple.systempreferences:*`) is rejected at the IPC boundary. That's
 * the right place for it — keeps the policy in one auditable file rather
 * than spread across JS validators.
 *
 * ## What's deferred
 *
 *   - Visual link styling. The CodeMirror `defaultHighlightStyle` already
 *     colors link text and URLs via tag styles; we don't add an extra
 *     `.cm-md-link` class for now to avoid fighting the default colors.
 *     Revisit if Chris wants stronger affordance.
 *   - Hover cursor change on Cmd-key. Doable via a state field tracking
 *     modifier key, but adds complexity for a small UX gain.
 *   - Hover preview / "title" tooltip showing the resolved URL. Same as
 *     above — deferred unless it pulls its weight.
 */
import { EditorView } from '@codemirror/view';
import type { Extension, EditorState } from '@codemirror/state';
import { syntaxTree } from '@codemirror/language';
import type { SyntaxNode } from '@lezer/common';
import { openUrl } from '@tauri-apps/plugin-opener';

/**
 * Given a syntax node believed to BE or CONTAIN a link, return the href
 * string, or null if extraction fails. Three node types matter:
 *
 *   - URL: the bare URL token inside a Link's parens, e.g. `https://x.com`
 *     in `[text](https://x.com)`. The node text IS the URL.
 *   - Autolink: the `<https://x.com>` form OR a bare URL captured by GFM.
 *     Strip surrounding angle brackets when present.
 *   - Link: the whole `[text](url)` span. Walk children to find the URL child.
 */
function extractUrl(node: SyntaxNode, state: EditorState): string | null {
  if (node.name === 'URL') {
    return state.doc.sliceString(node.from, node.to).trim() || null;
  }
  if (node.name === 'Autolink') {
    let text = state.doc.sliceString(node.from, node.to).trim();
    if (text.startsWith('<') && text.endsWith('>')) text = text.slice(1, -1);
    return text || null;
  }
  if (node.name === 'Link') {
    let cur: SyntaxNode | null = node.firstChild;
    while (cur) {
      if (cur.name === 'URL') {
        return state.doc.sliceString(cur.from, cur.to).trim() || null;
      }
      cur = cur.nextSibling;
    }
  }
  return null;
}

/**
 * Walk from the document position outward (innermost → outer) looking for
 * the first ancestor that is a link-shaped node, and return its href.
 * Returns null when the click isn't on or inside a link.
 *
 * We walk OUT (not in) because the user could click on a Lezer "URL" leaf
 * directly OR on the surrounding "Link" parent — both should resolve to the
 * same href. resolveInner gives us the innermost node at the position; the
 * walk handles either case symmetrically.
 */
function findLinkAt(view: EditorView, pos: number): string | null {
  const tree = syntaxTree(view.state);
  let cur: SyntaxNode | null = tree.resolveInner(pos);
  while (cur) {
    if (cur.name === 'Link' || cur.name === 'Autolink' || cur.name === 'URL') {
      const url = extractUrl(cur, view.state);
      if (url) return url;
    }
    cur = cur.parent;
  }
  return null;
}

/**
 * Returns the CodeMirror extension. Add it to the EditorView's `extensions`
 * array in MarkdownEditor.svelte.
 */
export function markdownLinks(): Extension {
  return EditorView.domEventHandlers({
    mousedown(event, view) {
      // Cmd on macOS, Ctrl elsewhere — matches the convention in VS Code,
      // Obsidian, GitHub's web editor, etc. Plain click stays as
      // "place cursor for editing" so the user can edit link text without
      // accidentally launching their browser.
      if (!event.metaKey && !event.ctrlKey) return false;
      const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
      if (pos == null) return false;
      const url = findLinkAt(view, pos);
      if (!url) return false;

      // We claim the event whether the open succeeds or not — preventing
      // the default click ensures the cursor doesn't jump on a failed
      // open (and a failure is rare: capability rejection at the IPC
      // layer is the only realistic cause).
      event.preventDefault();
      openUrl(url).catch((err) => {
        console.error('[markdown-links] openUrl failed:', err);
      });
      return true;
    },
  });
}
