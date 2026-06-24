<!--
  MarkdownEditor — CodeMirror 6 wrapper for Captain's Log's prose surfaces.

  ## Why CM6 over the WYSIWYG markdown editors

  The on-disk format is markdown and Phase 5 hands those .md files to an
  LLM that's sensitive to formatting drift (heading-style flips, bullet
  normalization, smart-quote substitution, etc.). Milkdown / TipTap /
  Lexical all mutate the source on save — even when the visible content
  looks identical, the bytes shift. CodeMirror 6's buffer IS the markdown
  file, byte-for-byte. The user sees `**bold**` markers (we ship source
  mode for v1; live-preview decorations are a follow-up); the file on
  disk is what they typed.

  ## Public API

      <MarkdownEditor
        value={state}
        onChange={(v) => state = v}
        placeholder="What did you just do?"
        autofocus
        style="flex: 1;"
      />

  - `value`: ONE-WAY initial value + external reload only. NOT $bindable
    because CM6 transactions own the doc — re-setting on every keystroke
    fights the transaction model and resets the cursor. Use `onChange` to
    push changes back to the parent.
  - `onChange`: fires on every doc-changing transaction with the full
    current doc string. Plug straight into the consumer's existing $effect
    debounce — the auto-save flow stays identical to what it was with the
    prior textarea wrappers.
  - `placeholder`, `class`, `style`, `autofocus`: pass through.

  ## CSS variables consumers can set via `style`

      --md-font-family   default: var(--font-body)  (/journal sets monospace)
      --md-font-size     default: var(--text-body)
      --md-line-height   default: var(--text-body-lh)
      --md-min-height    default: 0                 (per-surface row counts)
      --md-padding       default: var(--space-3)    (/journal uses --space-4)

  ## External-value sync

  Without the `current === value` guard, the listener's `onChange` would
  trigger a parent state update, which re-flows the `value` prop, which
  re-fires this effect, which dispatches a setState, which moves the
  cursor to position 0. The guard breaks the echo loop.
-->
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    EditorView,
    keymap,
    placeholder as placeholderExt,
  } from '@codemirror/view';
  import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
  import { markdown } from '@codemirror/lang-markdown';
  import { GFM } from '@lezer/markdown';
  import { syntaxHighlighting, defaultHighlightStyle } from '@codemirror/language';
  import { markdownLinks } from './markdown-links';

  let {
    value = '',
    onChange,
    placeholder = '',
    class: className = '',
    style = '',
    autofocus = false,
    id = undefined,
  }: {
    value?: string;
    onChange: (next: string) => void;
    placeholder?: string;
    class?: string;
    style?: string;
    autofocus?: boolean;
    /** Optional DOM id. Forwarded to the inner .cm-content element so
     * <label for={id}> clicks focus the editor. */
    id?: string;
  } = $props();

  let container: HTMLDivElement;
  let view: EditorView | undefined;

  onMount(() => {
    view = new EditorView({
      doc: value,
      extensions: [
        history(),
        keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
        // GFM extension turns on the lezer rules for task lists ([ ] / [x]),
        // strikethrough (~~text~~), tables, and autolinks (bare URLs become
        // first-class link nodes). This is SOURCE-mode parsing only — task
        // list syntax gets distinct highlighting but doesn't render as a
        // clickable checkbox (that's a live-preview decoration, deferred
        // per the Phase 2.5 design). Autolink parsing pairs with the
        // markdown-links plugin in Step 2 so bare URLs are Cmd-clickable.
        markdown({ extensions: [GFM] }),
        syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
        EditorView.lineWrapping,
        // Cmd-click on Markdown links opens via Tauri's opener. Sees Link
        // (`[text](url)`), Autolink (`<url>`), and GFM bare URLs.
        markdownLinks(),
        // Native browser spell-check via WebKit. CodeMirror's editing
        // surface is a contenteditable div (not a textarea), and WebKit
        // paints squiggles + delivers right-click suggestions natively on
        // contenteditable elements even when tauri-apps/tauri#7705 hides
        // them on <textarea>. By NOT installing a custom drawSelection
        // (which renders its own cursor + masks WebKit's editor surface),
        // we let WebKit's editor + NSSpellChecker do the work end-to-end:
        // same engine that Apple Mail and Pages use, no IPC round-trip,
        // no streaming-Correction gap, right-click menu pre-populated.
        // Forwarding `id` lets <label for={id}> clicks focus the editor.
        EditorView.contentAttributes.of(
          id ? { spellcheck: 'true', id } : { spellcheck: 'true' }
        ),
        placeholder ? placeholderExt(placeholder) : [],
        // The listener calls `onChange` via the prop directly. Svelte 5's
        // destructured props read the current value at call time, so
        // there's no stale-closure hazard even though this listener is
        // constructed once at mount.
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            onChange(update.state.doc.toString());
          }
        }),
      ],
      parent: container,
    });
    if (autofocus) {
      view.focus();
    }
  });

  onDestroy(() => {
    view?.destroy();
  });

  // External-value sync. See the docstring above for the echo-loop story.
  $effect(() => {
    if (!view) return;
    const current = view.state.doc.toString();
    if (current === value) return;
    view.dispatch({
      changes: { from: 0, to: current.length, insert: value },
    });
  });
</script>

<div bind:this={container} class="md-editor {className}" {style}></div>

<style>
  /* Wrapper picks up consumer-supplied sizing (flex: 1 from /capture and
   * /journal; rows-derived heights from /summary). The inner .cm-editor
   * fills the wrapper via height: 100% so flex-grown contexts work. */
  .md-editor {
    width: 100%;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
  }

  /* CM6 emits its own DOM tree (.cm-editor > .cm-scroller > .cm-content);
   * reach across the component boundary with :global() to style its chrome
   * to match the prior <textarea> consumers were used to. The visual
   * vocabulary (background, border, focus glow, radius) is identical so
   * the swap is invisible to the rest of the design system. */
  .md-editor :global(.cm-editor) {
    flex: 1;
    background: var(--bg-surface);
    color: var(--text-primary);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    transition: border-color var(--transition-fast);
    font-family: var(--md-font-family, var(--font-body));
    font-size: var(--md-font-size, var(--text-body));
    line-height: var(--md-line-height, var(--text-body-lh));
    /* Default min-height: 0 so the editor can shrink below its content's
     * intrinsic size inside a flex-column parent — without this, .cm-editor's
     * default `min-height: auto` makes it grow with content and push the
     * surrounding popup boundaries (visible in the /capture popup, where
     * the body would slide under the Labels row). The CM6 .cm-scroller
     * inside takes over with overflow: auto. Consumers that DO want a
     * floor (e.g. /summary's per-field row counts) override via the
     * --md-min-height CSS variable. */
    min-height: var(--md-min-height, 0);
  }

  .md-editor :global(.cm-editor.cm-focused) {
    outline: none;
    border-color: var(--accent-primary);
    box-shadow: 0 0 0 2px var(--focus-glow);
  }

  /* Padding lives on .cm-scroller (not .cm-editor) so the focus ring
   * hugs the outer border without an awkward gap. Defaults to the body
   * `--space-3` but /journal's monospace surface overrides to --space-4
   * to match its previous textarea-era look. */
  .md-editor :global(.cm-scroller) {
    padding: var(--md-padding, var(--space-3));
    overflow: auto;
    font-family: inherit;
  }

  /* Selection styling — match the rest of the app's accent colors. The
   * caret is Prodigy orange in both themes so it stays visible against
   * the dark and light backgrounds (the prior --text-primary caret was
   * almost invisible against the dark surface). */
  .md-editor :global(.cm-content) {
    caret-color: var(--accent-primary);
  }
  .md-editor :global(.cm-selectionBackground) {
    background: var(--focus-glow) !important;
  }
  .md-editor :global(.cm-focused .cm-selectionBackground) {
    background: var(--focus-glow) !important;
  }

  /* CM6's default placeholder color is too pale for our themes; bump
   * to --text-muted so it matches the textareas the editor replaces. */
  .md-editor :global(.cm-placeholder) {
    color: var(--text-muted);
    font-style: normal;
  }
</style>
