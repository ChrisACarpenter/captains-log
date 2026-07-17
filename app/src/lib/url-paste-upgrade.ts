/**
 * URL paste-upgrade Svelte action for plain `<input>` + `<textarea>`
 * elements.
 *
 * ## Behavior
 *
 * On a paste event whose clipboard is one URL and nothing else:
 *
 *   - **Selection present** → wrap the selection as `[selected](url)`.
 *     This is the Slack pattern the app's CodeMirror `linkPaste`
 *     extension already implements for prose editors. No fetch —
 *     the user's selection is the label, always.
 *
 *   - **No selection (or selection contains characters that would
 *     break the wrap)** → insert the bare URL first (so the user
 *     immediately sees something), then async-upgrade to
 *     `[title](url)` markdown once `enrich_link` resolves. The bare
 *     URL stays if enrichment yields nothing useful (auth-gated
 *     case) or if the user typed over it before enrichment resolved.
 *
 * Everything else (multi-line clipboard, non-URL text, image data)
 * falls through to the input's default paste behavior.
 *
 * ## Why an action and not a component
 *
 * The three call sites (landing-page task-add modal, inline task-
 * edit input, and TextAreaField in the Prep Self Review wizard) all
 * host plain `<input>` / `<textarea>` elements. Wrapping them in a
 * new component would force each site to swap markup + rebind
 * events; a `use:urlPasteUpgrade` action leaves the underlying
 * markup alone and coexists cleanly with `bind:value`.
 *
 * ## bind:value compatibility
 *
 * When the action mutates `.value` directly (both the initial
 * bare-URL paste and the async upgrade), it dispatches a synthetic
 * `input` event afterwards. Svelte 5's `bind:value` on
 * `<input>` / `<textarea>` reads `.value` off the target when the
 * input event fires — so the bound state stays in lockstep with
 * the visible text.
 *
 * ## Matches `link-paste.ts` where it counts
 *
 * The URL detection regex (`/^https?:\/\/[^\s<>"']+$/`) and the
 * selection-safety rules (no `]`, no newlines) are shared with the
 * CodeMirror extension. Users get the same paste experience whether
 * they're typing in a Weekly Summary field (MarkdownEditor + CM6)
 * or a task input (plain `<input>`).
 */

import { invoke } from '@tauri-apps/api/core';
import type { EnrichmentResult } from '$lib/link-chip';

/** One URL, nothing else, with optional surrounding whitespace. */
const URL_ONLY_RE = /^https?:\/\/[^\s<>"']+$/;

function isSingleUrlPaste(text: string): boolean {
  return URL_ONLY_RE.test(text);
}

/**
 * A selection is safe to wrap as `[sel](url)` when it doesn't
 * contain a `]` (which would prematurely close the markdown text)
 * or a newline (which would break the inline link across lines —
 * CommonMark treats that as text, not a link). Empty selection
 * is NOT safe here — the caller uses that branch to fall through
 * to the "bare URL + async upgrade" path.
 */
function selectionSafeForWrap(text: string): boolean {
  if (!text) return false;
  if (/[\r\n]/.test(text)) return false;
  if (text.includes(']')) return false;
  return true;
}

/**
 * Return the URL's hostname for the fallback label, or the URL
 * itself if `new URL()` refuses to parse it. Matches how
 * `link-chip.ts` computes its fallback label.
 */
function hostnameOf(url: string): string {
  try {
    return new URL(url).hostname;
  } catch {
    return url;
  }
}

/**
 * Set an input's value AND notify Svelte's `bind:value`. Uses the
 * standard `input` event because that's what Svelte 5 listens for
 * on `<input>` and `<textarea>` two-way bindings.
 */
function setValueAndNotify(
  node: HTMLInputElement | HTMLTextAreaElement,
  value: string,
): void {
  node.value = value;
  node.dispatchEvent(new Event('input', { bubbles: true }));
}

export function urlPasteUpgrade(node: HTMLInputElement | HTMLTextAreaElement) {
  async function onPaste(event: ClipboardEvent): Promise<void> {
    const raw = event.clipboardData?.getData('text/plain') ?? '';
    const trimmed = raw.trim();
    if (!isSingleUrlPaste(trimmed)) return;

    event.preventDefault();

    const start = node.selectionStart ?? node.value.length;
    const end = node.selectionEnd ?? node.value.length;
    const currentValue = node.value;
    const selectedText = currentValue.slice(start, end);
    const hadFocus = document.activeElement === node;

    if (selectedText && selectionSafeForWrap(selectedText)) {
      // Slack pattern: wrap the selection. No enrichment fetch —
      // the label is the user's selection, authoritative.
      const insert = `[${selectedText}](${trimmed})`;
      const nextValue = currentValue.slice(0, start) + insert + currentValue.slice(end);
      setValueAndNotify(node, nextValue);
      if (hadFocus) {
        const caret = start + insert.length;
        node.setSelectionRange(caret, caret);
      }
      return;
    }

    // No usable selection: insert the bare URL first so the user
    // immediately sees something in the field, then async-upgrade
    // to `[title](url)` when enrich_link resolves. If enrichment
    // yields nothing (auth-gated) or the user has already typed
    // over the URL by the time it lands, we leave the field alone.
    const insertPos = start;
    const nextValue = currentValue.slice(0, start) + trimmed + currentValue.slice(end);
    setValueAndNotify(node, nextValue);
    if (hadFocus) {
      const caret = insertPos + trimmed.length;
      node.setSelectionRange(caret, caret);
    }

    try {
      const result = await invoke<EnrichmentResult>('enrich_link', { url: trimmed });
      const label =
        result.title?.trim()
        || result.siteName?.trim()
        || hostnameOf(trimmed);
      // Verify the URL is still in the input at the expected position.
      // A small slack window either side tolerates one or two chars
      // of drift from adjacent typing; beyond that we treat the URL
      // as "intentionally modified" and leave it alone.
      const live = node.value;
      const search = live.slice(
        Math.max(0, insertPos - 8),
        Math.min(live.length, insertPos + trimmed.length + 8),
      );
      const localIndex = search.indexOf(trimmed);
      if (localIndex < 0) return;
      const from = Math.max(0, insertPos - 8) + localIndex;
      const to = from + trimmed.length;
      if (live.slice(from, to) !== trimmed) return; // defensive re-check
      const upgraded = `[${label}](${trimmed})`;
      const upgradedValue = live.slice(0, from) + upgraded + live.slice(to);
      const stillFocused = document.activeElement === node;
      const preCaret = node.selectionStart ?? upgradedValue.length;
      setValueAndNotify(node, upgradedValue);
      if (stillFocused) {
        // If the caret was at the end of the URL when enrichment
        // landed (the common case: user pasted then waited), park
        // it at the end of the upgraded link so a follow-up keystroke
        // continues where it should. Otherwise leave the caret at
        // its live position (clamped to the new length).
        const wasAtEndOfUrl = preCaret === to;
        const nextCaret = wasAtEndOfUrl ? from + upgraded.length : Math.min(preCaret, upgradedValue.length);
        node.setSelectionRange(nextCaret, nextCaret);
      }
    } catch (err) {
      // Bad URL, IPC error, backend panic — leave the bare URL in
      // place. Same posture as the CodeMirror link-paste extension.
      console.error('[url-paste-upgrade] enrich_link failed:', err);
    }
  }

  // Cast to EventListener so TypeScript's DOM types accept the
  // async signature — addEventListener('paste', …) infers a strict
  // EventListener that doesn't include the ClipboardEvent-shaped
  // callback, even though the runtime dispatches ClipboardEvent
  // objects for the 'paste' name.
  const handler = onPaste as unknown as EventListener;
  node.addEventListener('paste', handler);

  return {
    destroy(): void {
      node.removeEventListener('paste', handler);
    },
  };
}
