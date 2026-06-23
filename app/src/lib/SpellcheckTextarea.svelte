<!--
  SpellcheckTextarea — a drop-in `<textarea>` that paints red wavy
  underlines on misspelled words.

  Why this exists: Tauri's WKWebView hides the OS-native spell-check
  underline styling (tauri-apps/tauri#7705). Native spell-check is on
  via the Edit menu + NSUserDefaults seed (right-click suggestions
  work), but visual feedback never renders. We bridge the gap by:

    1. Asking macOS's `NSSpellChecker` (via the `check_spelling` Tauri
       command) for misspelled `{start, length}` ranges every time the
       text settles for 400ms.
    2. Mirroring the textarea's content into a positioned `<div>`
       behind it. The mirror text is fully transparent EXCEPT for the
       misspelled spans, which get a wavy-red `text-decoration`
       underline. The underline is what the user sees; the text under
       it is the real textarea's text on top.

  The component preserves `spellcheck="true"` on the textarea itself,
  so the right-click "Show Spelling and Grammar" macOS menu still
  works untouched.

  Usage:
    <SpellcheckTextarea bind:value={body} placeholder="..." rows={5} />

  Per-surface styling tweaks (padding, font, border) live as CSS custom
  properties on the wrapper, so each consumer can match its own look:
    <SpellcheckTextarea
      bind:value
      style="--sq-padding: 16px; --sq-font: var(--font-body);"
    />
-->
<script lang="ts">
  import { onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  type SpellRange = { start: number; length: number };

  let {
    value = $bindable<string>(''),
    placeholder = '',
    rows = 3,
    class: className = '',
    style = '',
    spellcheck = true,
    ...rest
  }: {
    value?: string;
    placeholder?: string;
    rows?: number;
    class?: string;
    style?: string;
    spellcheck?: boolean;
    [key: string]: unknown;
  } = $props();

  const SPELLCHECK_DEBOUNCE_MS = 400;

  let ranges = $state<SpellRange[]>([]);
  let textareaEl = $state<HTMLTextAreaElement | null>(null);
  let backdropEl = $state<HTMLDivElement | null>(null);

  let debounceTimer: ReturnType<typeof setTimeout> | null = null;
  // Monotonic request id — we drop any check result whose id is stale
  // (a newer check has been kicked off since this one was sent).
  let requestSeq = 0;

  // Run the spell-check on the latest value. Debounced separately from
  // the 1.5s autosave so squiggles refresh ~4x faster than the file
  // save cadence.
  $effect(() => {
    value;
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => void doCheck(), SPELLCHECK_DEBOUNCE_MS);
  });

  async function doCheck() {
    requestSeq++;
    const id = requestSeq;
    const snapshot = value;
    try {
      const result = await invoke<SpellRange[]>('check_spelling', {
        text: snapshot
      });
      // Drop stale results: if another check was kicked off after this
      // one (user kept typing during the await), abandon this result.
      if (id !== requestSeq) return;
      ranges = result ?? [];
    } catch {
      // Backend unavailable (shouldn't happen on macOS) — leave ranges
      // alone; no squiggles is no worse than a silent no-op.
    }
  }

  onDestroy(() => {
    if (debounceTimer) clearTimeout(debounceTimer);
  });

  // ---- Mirror rendering ----

  function escapeHtml(s: string): string {
    return s
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  }

  // Build the mirror HTML: text outside misspelled ranges renders plain;
  // misspelled spans get wrapped in `<mark class="sq">` for the wavy
  // underline.
  const highlightsHtml = $derived.by(() => {
    if (!ranges.length) return escapeHtml(value);
    let html = '';
    let cursor = 0;
    for (const r of ranges) {
      // Defensive: ignore out-of-range entries (text could have shrunk
      // between check and render).
      if (r.start < cursor || r.start + r.length > value.length) continue;
      html += escapeHtml(value.slice(cursor, r.start));
      html += `<mark class="sq">${escapeHtml(
        value.slice(r.start, r.start + r.length)
      )}</mark>`;
      cursor = r.start + r.length;
    }
    html += escapeHtml(value.slice(cursor));
    // Trailing-newline drift fix: if the text ends in a newline, the
    // textarea reserves a blank trailing line but a div doesn't — so
    // squiggles on the last line shift up by line-height. Adding a
    // single trailing space character pads the div's last line.
    if (value.endsWith('\n')) html += ' ';
    return html;
  });

  // Mirror the textarea's scroll position to the backdrop so squiggles
  // stay aligned with their words during scrolling.
  function syncScroll() {
    if (textareaEl && backdropEl) {
      backdropEl.scrollTop = textareaEl.scrollTop;
      backdropEl.scrollLeft = textareaEl.scrollLeft;
    }
  }
</script>

<div class="sq-wrap" style={style}>
  <div class="sq-backdrop" bind:this={backdropEl} aria-hidden="true">
    <div class="sq-highlights">{@html highlightsHtml}</div>
  </div>
  <textarea
    bind:this={textareaEl}
    bind:value
    class="sq-textarea {className}"
    {spellcheck}
    {placeholder}
    {rows}
    onscroll={syncScroll}
    {...rest}
  ></textarea>
</div>

<style>
  /* Wrapper provides positioning context. Layout-sized by its consumer's
   * parent — we don't dictate width/height ourselves so this drops into
   * grid/flex layouts without surprises.
   *
   * Flex-column so consumers can grow the inner textarea to fill the
   * wrapper (e.g. /capture body, /journal editor) by setting `flex: 1`
   * on the textarea via :global(). For rows-based static sizing
   * (/summary), the textarea's natural rows-derived height drives the
   * wrapper's height — flex-column doesn't change that. */
  .sq-wrap {
    position: relative;
    display: flex;
    flex-direction: column;
    width: 100%;
  }

  /* Backdrop sits behind the textarea, exactly the same size + shape.
   * Caps scroll so it can move in lockstep with the textarea via
   * syncScroll. pointer-events: none means clicks pass through to the
   * textarea — caret behavior is unaffected. */
  .sq-backdrop {
    position: absolute;
    inset: 0;
    pointer-events: none;
    overflow: hidden;
    z-index: 0;
    /* Inherit the textarea's visual chrome — consumer sets these via
     * CSS variables (defaults below) so the backdrop and textarea
     * share a frame. */
    border-radius: var(--sq-radius, var(--radius-md, 10px));
    border: 1px solid transparent;
    box-sizing: border-box;
  }

  /* Mirror text. Color is transparent so the actual textarea's text on
   * top renders on its own — this layer is purely the squiggle anchor.
   * white-space: pre-wrap and word-wrap: break-word match the textarea's
   * line-wrapping. */
  .sq-highlights {
    color: transparent;
    background: transparent;
    white-space: pre-wrap;
    word-wrap: break-word;
    overflow-wrap: break-word;
    margin: 0;
    padding: var(--sq-padding, var(--space-3, 12px));
    box-sizing: border-box;
    width: 100%;
    min-height: 100%;
    /* Font is critical — must match the textarea exactly or squiggle
     * positions drift. Consumer can override via --sq-font. */
    font-family: var(--sq-font-family, inherit);
    font-size: var(--sq-font-size, inherit);
    font-weight: var(--sq-font-weight, inherit);
    line-height: var(--sq-line-height, inherit);
    letter-spacing: var(--sq-letter-spacing, inherit);
  }

  /* The squiggle itself — transparent fill + transparent text + wavy
   * underline = visible underline only. text-decoration-skip-ink: none
   * keeps the squiggle continuous under descenders. */
  .sq-highlights :global(mark.sq) {
    background: transparent;
    color: transparent;
    text-decoration: underline wavy #e5484d;
    text-decoration-skip-ink: none;
    text-underline-offset: 2px;
    /* Some browsers ignore wavy without an explicit thickness — set
     * thickness so the squiggle renders consistently. */
    text-decoration-thickness: 1px;
  }

  /* Textarea sits on top, with a transparent background so the backdrop's
   * squiggles show through. The actual text content is fully opaque
   * because we don't touch the textarea's color. */
  .sq-textarea {
    position: relative;
    z-index: 1;
    background: transparent;
    width: 100%;
    box-sizing: border-box;
  }
</style>
