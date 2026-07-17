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
    type KeyBinding,
  } from '@codemirror/view';
  import { Compartment, type EditorState } from '@codemirror/state';
  import { defaultKeymap, history, historyKeymap, indentMore, indentLess } from '@codemirror/commands';
  import { markdown, markdownKeymap } from '@codemirror/lang-markdown';
  import { syntaxTree } from '@codemirror/language';
  import { GFM } from '@lezer/markdown';
  import {
    syntaxHighlighting,
    defaultHighlightStyle,
    HighlightStyle,
  } from '@codemirror/language';
  import { tags } from '@lezer/highlight';
  import { markdownLinks } from './markdown-links';
  import { linkPaste } from './link-paste';
  import { markdownFormattingKeymap } from './markdown-formatting';
  import { livePreview as livePreviewExt } from './live-preview';
  import { isValidIsoDateRange } from './date-chip';
  import MarkdownToolbar from './MarkdownToolbar.svelte';
  import DatePickerPopover from './DatePickerPopover.svelte';

  // ## Bold + heading font-family swap
  //
  // ABeeZee (the brand body face) only ships at weight 400 — no real
  // bold glyphs. The default highlight style's `font-weight: bold` on
  // `strong` and `heading` therefore renders invisibly with ABeeZee;
  // WebKit's faux-bold synthesis isn't aggressive enough to be seen.
  // Override these two tags to switch font-family to system-ui (SF Pro
  // on macOS), which has a real native bold. The mid-paragraph font
  // change is subtle in look but unmistakable in weight — clicking
  // Bold visibly does something. Emphasis (italic) stays default
  // because ABeeZee's @import includes the italic axis (`ital@0;1`),
  // so `font-style: italic` resolves to a real face.
  //
  // ## Why this is the ONLY custom style right now
  //
  // The Phase 2.5 marker-color + code + task + quote treatments were
  // tried and pulled back. Chris's pivot: lean into a rich-text-style
  // experience on /summary + /capture (live-preview decorations or a
  // WYSIWYG editor), keep /journal as the canonical raw-source view.
  // That redesign is what the next-up workflow is researching; until
  // it lands, this stays minimal — just the load-bearing bold/heading
  // fix and nothing else cosmetic.
  //
  // Registered as PRIMARY (not fallback) so defaultHighlightStyle's
  // rules for italic/strikethrough/link-underline/etc. keep applying;
  // CM6's `getHighlighters` ignores the fallback once any primary is
  // registered.
  const markdownMarkerStyle = HighlightStyle.define([
    {
      tag: tags.strong,
      fontWeight: '700',
      fontFamily: 'system-ui, -apple-system, sans-serif',
    },
    {
      tag: tags.heading,
      fontWeight: '700',
      fontFamily: 'system-ui, -apple-system, sans-serif',
    },
  ]);
  // Numbered list markers ("1.", "2.", …) are tagged via Decoration.mark
  // with class `.cm-md-list-num` from live-preview.ts and styled directly
  // (see the `.cm-md-list-num` rule in the component CSS block below).
  // We don't use a HighlightStyle rule for them because CM6's default-
  // highlight style appears to out-cascade ours for
  // `tags.processingInstruction`, leaving the digits illegibly faint on
  // the dark surface.

  let {
    value = '',
    onChange,
    placeholder = '',
    class: className = '',
    style = '',
    autofocus = false,
    id = undefined,
    showToolbar = true,
    livePreview = false,
    scrollTargetOffset = null,
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
    /** Show the formatting toolbar above the editor (Bold / Italic /
     *  lists / link / etc.). Defaults to true; set to false on /journal
     *  (raw-markdown surface where the toolbar would be off-message). */
    showToolbar?: boolean;
    /** Slack/Typora-style live preview — hides markdown markers (**, *,
     *  ~~, #, -, >, etc.) so the user sees rendered rich text. The buffer
     *  on disk stays canonical markdown. Defaults to false (source mode).
     *  Phase 2.5 Architecture B opts /capture and /summary into this;
     *  /journal stays raw-source for power-user editing. */
    livePreview?: boolean;
    /** Phase 3b Slice 2 — byte offset to scroll into view once the
     *  view is mounted and the current value is loaded. Set by /journal
     *  when the URL carries `?scrollTo=N` (search-result deep link).
     *  Clamped to the doc range; null / undefined / non-finite = no-op. */
    scrollTargetOffset?: number | null;
  } = $props();

  let container: HTMLDivElement;
  // Reactive ($state) so the MarkdownToolbar child sees `view` flip from
  // undefined to the live EditorView after mount. Without $state the
  // toolbar would render with view=undefined and never update.
  let view = $state<EditorView | undefined>(undefined);

  // Per-editor compartment wrapping the livePreview extension. Lets us
  // swap modes (Preview ↔ Source) on /journal WITHOUT remounting the
  // whole editor — `view.dispatch({effects: livePreviewCompartment.reconfigure(...)})`
  // installs the new extension set on the existing EditorView. Cursor
  // position, undo history, scroll position, and selection all survive.
  //
  // Earlier shape used `{#key viewMode}` in /journal to force remount;
  // that lost the cursor on every toggle. CodeMirror 6 doesn't let you
  // add/remove extensions reactively without a Compartment, so this is
  // the canonical way to get prop-driven extension changes.
  const livePreviewCompartment = new Compartment();
  // Date-chip picker state. Driven by `captainslog:date-chip-click`
  // events bubbling up from the chip widget's DOM. The popover renders
  // when `datePickerOpen` is true and dispatches its commit via a
  // window-level event the date-chip extension listens for.
  let datePickerOpen = $state(false);
  let datePickerIso = $state('');
  let datePickerFrom = $state(0);
  let datePickerTo = $state(0);
  let datePickerAnchor = $state<HTMLElement | undefined>(undefined);

  function handleDateChipClick(e: Event): void {
    const detail = (e as CustomEvent).detail as {
      from: number;
      to: number;
      iso: string;
      anchorEl: HTMLElement;
    };
    datePickerIso = detail.iso;
    datePickerFrom = detail.from;
    datePickerTo = detail.to;
    datePickerAnchor = detail.anchorEl;
    datePickerOpen = true;
  }

  function handleDatePickerCommit(newIso: string): void {
    // Dispatch the doc edit DIRECTLY on this MarkdownEditor's own view
    // — this is what guarantees the commit lands in the editor whose
    // chip was clicked, even when multiple MarkdownEditor instances are
    // mounted on the same page (e.g. /summary's 4 fields). Routing
    // through a window event with view-lookup-by-content would silently
    // misroute when two editors hold a matching ISO at the same offset.
    //
    // Sanity-validate the range still parses as ISO before committing
    // — guards against the doc having been edited between when the
    // picker opened and when the user picked (autosave reload, multi-
    // cursor edits, programmatic dispatches).
    if (!view) return;
    if (!isValidIsoDateRange(view, datePickerFrom, datePickerTo)) return;
    view.dispatch({
      changes: {
        from: datePickerFrom,
        to: datePickerTo,
        insert: newIso,
      },
      userEvent: 'input.type.datechip',
    });
  }

  function handleDatePickerClose(): void {
    datePickerOpen = false;
    datePickerAnchor = undefined;
  }

  // Reactive counter that bumps on every cursor-move or doc-change. The
  // MarkdownToolbar reads it as a dependency so its $derived
  // `activeFormats` re-evaluates when the cursor moves into / out of a
  // formatted node. Without this dep, $derived would only re-run when
  // `view` itself changed (never, post-mount) and the toolbar's pressed-
  // state indicators would stay stuck on whatever was at mount.
  let updateTick = $state(0);

  // Walks up the lezer-markdown tree from the primary selection head and
  // returns true if any ancestor is a list construct. Used to scope Tab
  // to list-indent inside lists, and let it fall through to the browser's
  // native focus-traversal everywhere else.
  function cursorInList(state: EditorState): boolean {
    const pos = state.selection.main.head;
    let node = syntaxTree(state).resolveInner(pos, -1);
    while (node) {
      const n = node.name;
      if (n === 'ListItem' || n === 'BulletList' || n === 'OrderedList') return true;
      if (!node.parent) break;
      node = node.parent;
    }
    // Fallback for the cursor-on-blank-line-inside-list case where the
    // tree resolves to the doc root before any ListItem.
    const line = state.doc.lineAt(pos).text;
    return /^\s*([-*+]|\d+\.)\s/.test(line);
  }

  // Context-aware Tab: indent inside markdown lists (nest a bullet),
  // otherwise return false so the browser performs native focus traversal
  // to the next form field. Returning false from a CM6 KeyBinding lets
  // the event fall through to the browser default — which is the lever
  // we want here. Replaces the bare `indentWithTab` import that used to
  // unconditionally insert a literal \t character.
  const listAwareTab: KeyBinding = {
    key: 'Tab',
    run: (view) => {
      if (!cursorInList(view.state)) return false;
      return indentMore(view);
    },
    shift: (view) => {
      if (!cursorInList(view.state)) return false;
      return indentLess(view);
    },
  };

  onMount(() => {
    view = new EditorView({
      doc: value,
      extensions: [
        history(),
        // Formatting shortcuts (Cmd+B/I/K/E + Cmd+Shift+7/8) PREPEND so
        // they win precedence over the catch-all defaultKeymap. The same
        // command functions back the toolbar buttons, so wrap/unwrap
        // logic only lives in one place — `markdown-formatting.ts`.
        // Keymap precedence is explicit: our toolbar shortcuts win first
        // (Cmd+B / Cmd+I / etc.), then Tab routes through listAwareTab,
        // then the markdown keymap handles Enter (auto-continues bullet
        // and numbered lists, increments the next number) and Backspace
        // (deletes one level of list/blockquote markup), and finally
        // defaultKeymap catches everything else. markdownKeymap MUST land
        // before defaultKeymap or the latter's plain `insertNewline` Enter
        // binding swallows the event and the auto-continue never fires.
        keymap.of([
          ...markdownFormattingKeymap,
          listAwareTab,
          ...markdownKeymap,
          ...defaultKeymap,
          ...historyKeymap,
        ]),
        // GFM extension turns on the lezer rules for task lists ([ ] / [x]),
        // strikethrough (~~text~~), tables, and autolinks (bare URLs become
        // first-class link nodes). This is SOURCE-mode parsing only — task
        // list syntax gets distinct highlighting but doesn't render as a
        // clickable checkbox (that's a live-preview decoration, deferred
        // per the Phase 2.5 design). Autolink parsing pairs with the
        // markdown-links plugin in Step 2 so bare URLs are Cmd-clickable.
        // Disable Setext (underline) headings — they conflict with starting
        // a bullet list under a paragraph. Without this, typing a paragraph
        // then a `-` on the next line parses the dash as an H2 underline
        // and re-renders the paragraph above as a heading instead of
        // starting a list. Captain's Log only emits ATX (`#`) headings.
        //
        // `addKeymap: false` disables the package's auto-installed Enter
        // and Backspace bindings. The Enter binding misfires on empty
        // list items (typing `- ` then Enter deletes the marker instead
        // of moving the cursor down), and the auto-continue behavior
        // surprised users mid-type. Default Enter (insert newline) is
        // the predictable behavior every other markdown editor uses;
        // Tab still works correctly via our own `listAwareTab` binding.
        markdown({
          extensions: [GFM, { remove: ['SetextHeading'] }],
          addKeymap: false
        }),
        // Register defaultHighlightStyle as a PRIMARY highlighter (no
        // `fallback: true`). Once another primary is registered below,
        // the fallback path is ignored — which would silently disable
        // italic, strikethrough, link underlines, and every code color.
        // As primaries, both styles' classes apply additively per CM6
        // docs ("the styling applied is the union of the classes they
        // emit"). The order below means our override wins for tags it
        // declares; everything else keeps the default.
        syntaxHighlighting(defaultHighlightStyle),
        syntaxHighlighting(markdownMarkerStyle),
        EditorView.lineWrapping,
        // Cmd-click on Markdown links opens via Tauri's opener. Sees Link
        // (`[text](url)`), Autolink (`<url>`), and GFM bare URLs.
        markdownLinks(),
        // Phase 4 — URL paste handling. Selection + URL paste wraps the
        // selection as `[sel](url)` (Slack pattern); no-selection URL
        // paste inserts the bare URL and asynchronously upgrades it to
        // `[title](url)` markdown once enrich_link resolves. Active in
        // both live-preview and source modes.
        linkPaste(),
        // Slack/Typora-style live preview — hide markdown markers via
        // atomic Decoration.replace ranges so the user sees rendered rich
        // text. Opt-in per surface (default false); when off the editor
        // stays source-mode for /journal's raw-markdown view.
        livePreviewCompartment.of(livePreview ? livePreviewExt() : []),
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
          // Trigger toolbar pressed-state recomputation on every cursor
          // move and every doc change. `selectionSet` covers arrow keys,
          // clicks, programmatic dispatches; `docChanged` covers typing
          // (selection moves with text) and any toolbar-driven edit.
          if (update.selectionSet || update.docChanged) {
            updateTick++;
          }
        }),
      ],
      parent: container,
    });
    if (autofocus) {
      view.focus();
    }
    // Date-chip click events bubble from the widget's button DOM through
    // CodeMirror's container. Listen here so the popover can be opened
    // by any chip in any editor instance independently.
    container.addEventListener(
      'captainslog:date-chip-click',
      handleDateChipClick as EventListener
    );
  });

  onDestroy(() => {
    container?.removeEventListener(
      'captainslog:date-chip-click',
      handleDateChipClick as EventListener
    );
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

  // Phase 3b Slice 2 — scroll-to-position for search-result deep links.
  //
  // Depends on both `view` (mounted after onMount fires) AND
  // `scrollTargetOffset` (set by parent after content loads). When both
  // are ready, dispatch `EditorView.scrollIntoView(pos, { y: 'center' })`
  // to bring the target byte offset into the middle of the viewport.
  //
  // Clamped to the current doc length so a stale deep-link pointing at a
  // trimmed / rewritten file doesn't blow up. Runs whenever the offset
  // changes (not just first mount), so a search → back → search cycle
  // scrolls to fresh positions correctly.
  $effect(() => {
    const v = view;
    const target = scrollTargetOffset;
    if (!v) return;
    if (target === null || target === undefined) return;
    if (!Number.isFinite(target)) return;
    const clamped = Math.max(0, Math.min(Math.floor(target), v.state.doc.length));
    v.dispatch({
      effects: EditorView.scrollIntoView(clamped, { y: 'center' }),
    });
  });

  // Reactive live-preview swap via Compartment reconfigure. When /journal
  // toggles Preview ↔ Source (livePreview prop flips), we dispatch a
  // reconfigure on the existing EditorView instead of remounting it.
  // Cursor, selection, scroll position, and undo history all survive.
  // First run after mount is a no-op (compartment already holds the
  // matching extension from construction), but harmless.
  $effect(() => {
    const lp = livePreview;
    if (!view) return;
    view.dispatch({
      effects: livePreviewCompartment.reconfigure(lp ? livePreviewExt() : []),
    });
  });
</script>

{#if showToolbar}
  <!-- Toolbar lives OUTSIDE the .md-editor wrapper so the strip stays
     fixed when the editor's `resize: vertical` handle (set by consumers
     on /summary) is dragged. The toolbar dispatches into `view` once
     the EditorView has mounted; clicks arriving before mount are
     no-ops, not crashes. -->
  <MarkdownToolbar {view} {updateTick} />
{/if}
<div bind:this={container} class="md-editor {className}" {style}></div>

{#if datePickerOpen && datePickerAnchor}
  <DatePickerPopover
    iso={datePickerIso}
    from={datePickerFrom}
    to={datePickerTo}
    anchorEl={datePickerAnchor}
    onCommit={handleDatePickerCommit}
    onClose={handleDatePickerClose}
  />
{/if}

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
    transition: border-color var(--duration-fast) var(--ease-standard);
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
   * almost invisible against the dark surface).
   *
   * font-synthesis forces the browser to faux-render bold and italic
   * even when the active font lacks those variants. The brand body face
   * (ABeeZee) only ships at weight 400, so CodeMirror's default `strong`
   * tag style (font-weight: bold) has no real glyphs to swap to — WebKit
   * is conservative about synthesizing bold for webfonts and was leaving
   * **bold** rendering as plain weight. Explicit `weight style` here
   * tells WebKit to always synthesize, which keeps the markdown toolbar
   * promise: clicking Bold actually produces visibly bold text. */
  .md-editor :global(.cm-content) {
    caret-color: var(--accent-primary);
    font-synthesis: weight style;
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

  /* Inline code chip (Slack-style). Decoration.mark from live-preview.ts
   * applies `.cm-md-inline-code` to the entire InlineCode node. The
   * backticks are hidden separately so visually the chip wraps only the
   * code body. `box-decoration-break: clone` keeps the chip's background
   * + padding + radius rendering correctly when the inline code wraps
   * across multiple lines (otherwise the wrap would cut the chip in half
   * with a stretched background). */
  .md-editor :global(.cm-md-inline-code) {
    /* inline-block (rather than inline) makes the chip an atomic inline
     * element. Critical when a checked task wraps the chip: parent
     * .cm-md-task-done's `text-decoration: line-through` propagates
     * through inline descendants but NOT through atomic inline-blocks,
     * so the strike line doesn't draw across the chip. Trade-off: the
     * chip can no longer split across lines mid-content; box-decoration-
     * break: clone is moot for inline-block so it's been removed. For
     * typical inline code (function name, short snippet), atomic is the
     * right behavior anyway. */
    display: inline-block;
    background: var(--bg-elevated);
    /* Lifted variant — raw accent-primary on bg-elevated at 0.92em (~14.7px)
       is only 3.69:1, failing WCAG AA. */
    color: var(--accent-primary-text);
    padding: 0 4px;
    border-radius: 3px;
    border: 1px solid var(--border-structural);
    font-size: 0.92em;
    vertical-align: baseline;
  }

  /* Fenced code block lines. Decoration.line from live-preview.ts applies
   * `.cm-md-fenced-line` to every line within a FencedCode block (including
   * the lines that used to show the ``` fences — those fences are now
   * hidden, so the top/bottom lines are empty-but-styled, giving the box
   * a natural padding gap). `padding-left/right` overrides .cm-line's
   * default to inset the box slightly from the editor's chrome. */
  .md-editor :global(.cm-md-fenced-line) {
    background: var(--bg-elevated);
    border-left: 3px solid var(--border-structural);
    padding-left: var(--space-3) !important;
  }

  /* Bullet glyph. Decoration.replace from live-preview.ts swaps the `-`
   * ListMark on a BulletList line for a `•` rendered in muted color so it
   * reads as a list marker, not a content character. Inline-block with a
   * fixed width keeps nested bullets aligned. Numbered lists are untouched —
   * the digits are meaningful. */
  .md-editor :global(.cm-md-bullet) {
    display: inline-block;
    width: 1ch;
    color: var(--text-secondary);
    opacity: 0.75;
    font-weight: 700;
  }

  /* Numbered-list markers ("1.", "2.", …). Rendered by an
   * OrderedListMarkerWidget that REPLACES the source digits with a span
   * carrying this class. Same shape as `.cm-md-bullet` so digits and
   * bullets read at the same visual weight. Replacing (not just wrapping)
   * was load-bearing — wrapping-via-Decoration.mark let CodeMirror's
   * default-highlight color win the cascade and the digits stayed
   * unreadable on the dark surface. */
  .md-editor :global(.cm-md-list-num) {
    color: var(--text-secondary);
    opacity: 0.75;
    font-weight: 500;
  }

  /* Hanging indent for list items. Different technique from v1 (which
   * used `text-indent: -2ch` on the line and ended up clipping the
   * BulletWidget in WebKit). Instead:
   *
   *  - The line gets `padding-left: <depth>*2ch` so the CONTENT area
   *    starts indented by the marker + space width. Wrapped visual rows
   *    naturally start at this indent — that IS the hang.
   *  - The marker widget (bullet glyph or numbered-list span) gets
   *    `margin-left: -2ch` so it pulls itself back out of the padding
   *    and sits at the line's left edge, where the user expects it.
   *
   * Result: marker at column 0 visually, row 1 text at column 2, rows
   * 2+ at column 2. The widget keeps its full inline-block width and
   * doesn't fight the cascade.
   *
   * Caveat: tuned for single-digit ordered lists (`1.` through `9.`
   * fit in 2ch). Double-digit items (`10.`, `11.`, …) visually overlap
   * the content by 1ch — acceptable for now; weekly summaries rarely
   * exceed 9 items per list. */
  .md-editor :global(.cm-md-list-line) {
    padding-left: calc(var(--md-list-depth, 1) * 2ch);
  }
  .md-editor :global(.cm-md-list-line .cm-md-bullet),
  .md-editor :global(.cm-md-list-line .cm-md-list-num) {
    margin-left: -2ch;
  }
  /* Same combo gotcha as before: lists inside blockquotes or fenced
   * code blocks need the gutter padding to stack with the container's
   * own padding (which uses !important). Without this, hang-indent
   * disappears inside quotes. */
  .md-editor :global(.cm-md-list-line.cm-md-blockquote-line),
  .md-editor :global(.cm-md-list-line.cm-md-fenced-line) {
    padding-left: calc(var(--space-3) + var(--md-list-depth, 1) * 2ch) !important;
  }

  /* Task checkbox. Decoration.replace from live-preview.ts swaps the
   * 3-char `[ ]` / `[x]` TaskMarker for this clickable square. The
   * base visual (size, border, hover/focus/checked states) comes from
   * the shared `.checkbox-square` class in app.css — the widget's
   * `className` sets both `cm-md-task` and `checkbox-square`. Only
   * editor-local layout tweaks (margin, baseline nudge) live here so
   * the widget sits inline with body text without disrupting line
   * height. */
  .md-editor :global(.cm-md-task) {
    margin: 0 4px 0 0;
    vertical-align: -3px;
  }

  /* Strikethrough + muted color over the body of a checked task. The
   * Decoration.mark from live-preview.ts targets the range AFTER the
   * TaskMarker (which is replaced by the checkbox widget above). The
   * widget itself isn't text so the strikethrough wouldn't render on
   * it anyway — the rule applies cleanly to the inline content. */
  .md-editor :global(.cm-md-task-done) {
    text-decoration: line-through;
    color: var(--text-secondary);
  }

  /* Inline date chip (Confluence-style). Decoration.replace from
   * date-chip.ts swaps any ISO date in prose for a button widget. The
   * styling: small pill, accent-soft background, accent-primary text +
   * calendar icon, snug padding so it sits inline with body text without
   * disrupting line height. Hover lifts the background a notch; focus
   * uses the standard accent ring. The button's own focus outline is
   * suppressed in favor of the box-shadow ring (matches the toolbar's
   * focus pattern). */
  .md-editor :global(.cm-date-chip) {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 1px 8px 1px 6px;
    margin: 0 1px;
    background: var(--bg-elevated);
    /* Contrast-safe orange — same reasoning as inline-code chip above. */
    color: var(--accent-primary-text);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-pill);
    font: inherit;
    font-size: 0.92em;
    font-weight: 500;
    line-height: 1.4;
    cursor: pointer;
    vertical-align: baseline;
    white-space: nowrap;
    transition: background var(--duration-fast) var(--ease-standard),
      border-color var(--duration-fast) var(--ease-standard);
    /* Prevent the user-agent button outline; we draw our own ring below. */
    outline: none;
  }
  .md-editor :global(.cm-date-chip:hover) {
    background: var(--bg-surface);
    border-color: var(--accent-primary);
  }
  .md-editor :global(.cm-date-chip:focus-visible) {
    box-shadow: 0 0 0 2px var(--focus-glow);
    border-color: var(--accent-primary);
  }
  .md-editor :global(.cm-date-chip svg) {
    flex-shrink: 0;
    opacity: 0.85;
  }
  .md-editor :global(.cm-date-chip-text) {
    line-height: 1.4;
  }

  /* Link chips (Phase 4). Same pill shape as .cm-date-chip so the
   * editor's chip vocabulary stays consistent. The favicon lives in
   * place of the calendar glyph. Chips can be clicked (open URL) or
   * Alt-clicked (edit label) — the pointer cursor + the box-shadow
   * focus ring both signal "actionable." */
  /* Link chips are `display: inline-block` (NOT inline-flex) with
   * inline children — icon + text lay out through the browser's
   * regular inline flow. inline-flex in CodeMirror's wrapping line
   * boxes had a WebKit-specific failure mode where the chip vanished
   * mid-layout on any doc change that re-measured the viewport;
   * inline-block sidesteps it entirely (matches how a native
   * link/word wraps to a new line when it hits the edge).
   *
   * The favicon is a background-image span (not an <img>) for the
   * same reason: <img> is a replaced element whose intrinsic size
   * comes from the (async-decoded) image bytes, which CM6's layout
   * pipeline treats differently from an inline block with fixed CSS
   * dimensions. */
  .md-editor :global(.cm-link-chip) {
    display: inline-block;
    padding: 1px 8px 1px 6px;
    margin: 0 1px;
    background: var(--bg-elevated);
    color: var(--accent-primary-text);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-pill);
    font: inherit;
    font-size: 0.92em;
    font-weight: 500;
    line-height: 1.4;
    cursor: pointer;
    vertical-align: baseline;
    white-space: nowrap;
    transition: background var(--duration-fast) var(--ease-standard),
      border-color var(--duration-fast) var(--ease-standard);
    outline: none;
  }
  .md-editor :global(.cm-link-chip:hover) {
    background: var(--bg-surface);
    border-color: var(--accent-primary);
  }
  .md-editor :global(.cm-link-chip:focus-visible) {
    box-shadow: 0 0 0 2px var(--focus-glow);
    border-color: var(--accent-primary);
  }
  .md-editor :global(.cm-link-chip-icon) {
    /* Favicon rendered via `background-image` — dimensions are entirely
     * CSS-driven, no async <img> decoding to trip up CM6's layout pass. */
    display: inline-block;
    width: 12px;
    height: 12px;
    background-size: contain;
    background-repeat: no-repeat;
    background-position: center;
    vertical-align: text-bottom;
    margin-right: 4px;
    opacity: 0.9;
  }
  .md-editor :global(.cm-link-chip svg) {
    /* Fallback globe icon when no favicon was fetched. Inline-block
     * so it participates in the same flow as .cm-link-chip-icon. */
    display: inline-block;
    width: 12px;
    height: 12px;
    vertical-align: text-bottom;
    margin-right: 4px;
    opacity: 0.9;
  }
  .md-editor :global(.cm-link-chip-text) {
    /* Baseline-aligned inline span — no flex quirks. */
    vertical-align: baseline;
    line-height: 1.4;
  }

  /* Blockquote lines. Decoration.line from live-preview.ts applies
   * `.cm-md-blockquote-line` to every line within a Blockquote. The `>`
   * markers themselves are hidden (LINE_START_MARKERS), so the result is
   * Slack-style quoted prose: indented content with an accent-colored
   * left bar standing in for the raw `> ` source.
   *
   * Structural styling (left border + indent) applies to ALL quoted lines.
   * Content styling (italic + muted color) is scoped via `:not(.cm-md-
   * fenced-line)` so a fenced code block nested inside a quote keeps its
   * monospace rendering — without the guard, the quote's italic + muted
   * color leak onto the code body and produce italic-muted monospace,
   * which reads as "deemphasized prose" rather than "code in a quote." */
  .md-editor :global(.cm-md-blockquote-line) {
    border-left: 3px solid var(--accent-primary);
    padding-left: var(--space-3) !important;
  }
  .md-editor :global(.cm-md-blockquote-line:not(.cm-md-fenced-line)) {
    color: var(--text-secondary);
    font-style: italic;
  }

</style>
