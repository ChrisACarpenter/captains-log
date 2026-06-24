<!--
  Formatting toolbar for MarkdownEditor. Renders a single horizontal strip
  of icon buttons above the editor. Each click dispatches a CodeMirror
  transaction via the shared command module (see markdown-formatting.ts),
  which means the toolbar and the Cmd+B/I/K/E/Shift+7/8 keymap share
  exactly one wrap/unwrap implementation per format — no drift.

  ## Why per-editor (and not floating / shared)

  Captain's Log has four MarkdownEditor instances on /summary plus one on
  /capture. A shared sticky toolbar at the page level forces a
  "which-editor-am-I-formatting?" mental model that a non-markdown user
  (the whole point of this toolbar) is least equipped to navigate.
  Per-editor toolbars cost ~32px of chrome per instance, which is well
  spent for clarity.

  ## Help button

  The trailing "?" button opens the markdown cheat sheet in the user's
  default browser via Tauri's opener plugin. Same destination as the
  inline link added to /journal — one URL, one mental model. Visually
  separated from the formatting buttons by a small gap so it doesn't read
  as a format toggle.

  ## A11y v1

  Each button gets aria-label + aria-keyshortcuts. `role="toolbar"` on the
  strip with a labelled name. `view.focus()` runs inside every command so
  the editor reclaims focus after a click — keystrokes go back to the
  user's content. The roving-tabindex pattern (one tab stop per strip)
  is deferred — v1 has plain tab order through each button. Revisit if
  Chris finds the tab-into-toolbar interruption noisy.
-->
<script lang="ts">
  import type { EditorView } from '@codemirror/view';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import Icon from './Icon.svelte';
  import {
    cycleHeading,
    toggleBold,
    toggleItalic,
    toggleStrikethrough,
    toggleBulletList,
    toggleNumberedList,
    toggleQuote,
    insertLink,
    toggleInlineCode,
  } from './markdown-formatting';

  let {
    view,
  }: {
    /** The EditorView this toolbar dispatches into. Owned by the parent
     *  MarkdownEditor; passed down once it's mounted. */
    view: EditorView | undefined;
  } = $props();

  const CHEAT_SHEET_URL = 'https://www.markdownguide.org/cheat-sheet/';

  /** Run a command if the editor is mounted. Guard for the brief window
   *  between MarkdownEditor's mount lifecycle and the parent first render
   *  — clicks that arrive in that gap are no-ops, not crashes. */
  function run(cmd: (v: EditorView) => boolean): void {
    if (view) cmd(view);
  }

  function openCheatSheet(): void {
    openUrl(CHEAT_SHEET_URL).catch((err) => {
      console.error('[markdown-toolbar] opener failed:', err);
    });
  }
</script>

<div class="md-toolbar" role="toolbar" aria-label="Markdown formatting">
  <button
    type="button"
    class="md-toolbar-btn"
    onclick={() => run(cycleHeading)}
    aria-label="Cycle heading level"
    title="Heading — cycle H1 / H2 / H3 / none"
  >
    <Icon name="heading" />
  </button>

  <span class="md-toolbar-sep" aria-hidden="true"></span>

  <button
    type="button"
    class="md-toolbar-btn"
    onclick={() => run(toggleBold)}
    aria-label="Bold"
    aria-keyshortcuts="Meta+B"
    title="Bold (⌘B)"
  >
    <Icon name="bold" />
  </button>

  <button
    type="button"
    class="md-toolbar-btn"
    onclick={() => run(toggleItalic)}
    aria-label="Italic"
    aria-keyshortcuts="Meta+I"
    title="Italic (⌘I)"
  >
    <Icon name="italic" />
  </button>

  <button
    type="button"
    class="md-toolbar-btn"
    onclick={() => run(toggleStrikethrough)}
    aria-label="Strikethrough"
    title="Strikethrough"
  >
    <Icon name="strikethrough" />
  </button>

  <span class="md-toolbar-sep" aria-hidden="true"></span>

  <button
    type="button"
    class="md-toolbar-btn"
    onclick={() => run(toggleBulletList)}
    aria-label="Bulleted list"
    aria-keyshortcuts="Meta+Shift+8"
    title="Bulleted list (⌘⇧8)"
  >
    <Icon name="list" />
  </button>

  <button
    type="button"
    class="md-toolbar-btn"
    onclick={() => run(toggleNumberedList)}
    aria-label="Numbered list"
    aria-keyshortcuts="Meta+Shift+7"
    title="Numbered list (⌘⇧7)"
  >
    <Icon name="list-ordered" />
  </button>

  <button
    type="button"
    class="md-toolbar-btn"
    onclick={() => run(toggleQuote)}
    aria-label="Block quote"
    title="Block quote"
  >
    <Icon name="quote" />
  </button>

  <span class="md-toolbar-sep" aria-hidden="true"></span>

  <button
    type="button"
    class="md-toolbar-btn"
    onclick={() => run(insertLink)}
    aria-label="Insert link"
    aria-keyshortcuts="Meta+K"
    title="Link (⌘K)"
  >
    <Icon name="link" />
  </button>

  <button
    type="button"
    class="md-toolbar-btn"
    onclick={() => run(toggleInlineCode)}
    aria-label="Code"
    aria-keyshortcuts="Meta+E"
    title="Code (⌘E)"
  >
    <Icon name="code" />
  </button>

  <span class="md-toolbar-spacer" aria-hidden="true"></span>

  <button
    type="button"
    class="md-toolbar-btn md-toolbar-help"
    onclick={openCheatSheet}
    aria-label="Open markdown cheat sheet in browser"
    title="Markdown cheat sheet"
  >
    <Icon name="help" />
  </button>
</div>

<style>
  .md-toolbar {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 4px 6px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    /* Sits flush above the editor with a small visual gap so the strip
     * reads as belonging to the editor below it, not to whatever sits
     * above. The editor wrapper has resize: vertical on /summary; we
     * stay OUTSIDE that resizable wrapper so the strip is fixed and
     * the drag-handle on the editor's bottom still works. */
    margin-bottom: 4px;
  }

  .md-toolbar-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    padding: 0;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast);
  }

  .md-toolbar-btn:hover {
    background: var(--bg-surface);
    color: var(--text-primary);
  }

  .md-toolbar-btn:active {
    background: var(--bg-surface);
    color: var(--accent-primary);
  }

  .md-toolbar-btn:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px var(--focus-glow);
    color: var(--accent-primary);
  }

  .md-toolbar-sep {
    width: 1px;
    height: 18px;
    background: var(--border-structural);
    margin: 0 4px;
  }

  /* Spacer pushes the trailing Help button to the right edge so it visually
   * separates from formatting buttons — same idea as the help icon's role
   * being category-different (navigation, not formatting). */
  .md-toolbar-spacer {
    flex: 1;
  }

  .md-toolbar-help {
    color: var(--text-muted);
  }
</style>
