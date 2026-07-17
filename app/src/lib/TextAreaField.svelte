<!--
  TextAreaField — the multi-line sibling of InputField. Same label +
  hint/warning trio, but wraps a <textarea> instead of an <input>.
  Built for Phase 5's "prose or a URL, no distinction" fields (review
  questions, OKRs); reusable anywhere a labeled multi-line text input
  is needed and MarkdownEditor is overkill.

  ## Props

      id           — DOM id, forwarded to the <textarea> AND used by
                     the <label for=…> linkage.
      label        — text or Snippet above the textarea.
      value        — $bindable, two-way via bind:value.
      placeholder  — optional.
      hint         — optional helper microcopy below.
      hintSnippet  — snippet form for inline markup (links, <strong>).
      warning      — optional error text; when present replaces hint,
                     wires role=alert + aria-invalid + aria-describedby.
      rows         — initial textarea height in rows. Default 6.
      spellcheck   — default true (multi-line inputs are usually prose).
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import { urlPasteUpgrade } from '$lib/url-paste-upgrade';

  type Props = {
    id: string;
    label?: string;
    labelSnippet?: Snippet;
    placeholder?: string;
    value: string;
    hint?: string;
    hintSnippet?: Snippet;
    warning?: string;
    rows?: number;
    spellcheck?: boolean;
    /** Opt in to URL paste-upgrade — paste-a-URL-with-selection wraps
     *  the selection as `[selected](url)`; paste-a-URL-without-
     *  selection inserts the bare URL then async-upgrades to
     *  `[title](url)` when `enrich_link` resolves. Matches the
     *  behavior of the CodeMirror `linkPaste` extension used by
     *  MarkdownEditor. Off by default — opt in per field. */
    urlPaste?: boolean;
  };

  let {
    id,
    label,
    labelSnippet,
    placeholder = '',
    value = $bindable(),
    hint,
    hintSnippet,
    warning,
    rows = 6,
    spellcheck = true,
    urlPaste = false,
  }: Props = $props();
</script>

<div class="field">
  <label for={id}>
    {#if labelSnippet}{@render labelSnippet()}{:else}{label}{/if}
  </label>
  {#if urlPaste}
    <!-- URL paste-upgrade opt-in. `use:` attaches the action that
         intercepts paste events, mirroring MarkdownEditor's
         behavior in a non-CodeMirror context. -->
    <textarea
      {id}
      {placeholder}
      {rows}
      class="text-input text-area-input"
      spellcheck={spellcheck ? 'true' : 'false'}
      aria-describedby={warning ? `${id}-warning` : undefined}
      aria-invalid={warning ? 'true' : undefined}
      bind:value
      use:urlPasteUpgrade
    ></textarea>
  {:else}
    <textarea
      {id}
      {placeholder}
      {rows}
      class="text-input text-area-input"
      spellcheck={spellcheck ? 'true' : 'false'}
      aria-describedby={warning ? `${id}-warning` : undefined}
      aria-invalid={warning ? 'true' : undefined}
      bind:value
    ></textarea>
  {/if}
  {#if warning}
    <p
      id="{id}-warning"
      class="field-hint is-warning"
      role="alert"
      aria-live="assertive"
    >
      {warning}
    </p>
  {:else if hintSnippet}
    <p class="field-hint">{@render hintSnippet()}</p>
  {:else if hint}
    <p class="field-hint">{hint}</p>
  {/if}
</div>

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  label {
    font-family: var(--font-display);
    font-size: var(--text-button);
    color: var(--text-primary);
  }
  label :global(a) {
    color: var(--accent-primary-text);
    text-decoration: underline;
  }
  label :global(a:hover) {
    filter: brightness(1.1);
  }

  /* Extends the shared .text-input utility. Vertical-only resize keeps
     the field from breaking the wizard's column layout on horizontal
     drag; min-height gives a comfortable prose-ish starting size. */
  .text-area-input {
    resize: vertical;
    min-height: 96px;
    font-family: var(--font-body);
    line-height: 1.5;
  }

  .field-hint {
    margin: 0;
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    color: var(--text-secondary);
  }
  .field-hint.is-warning {
    color: var(--accent-pink-text);
  }
  .field-hint :global(strong) {
    font-family: var(--font-display);
    color: var(--text-primary);
    font-weight: normal;
  }
</style>
