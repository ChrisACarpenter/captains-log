<!--
  External-update banner — rendered when the weekly markdown file we
  have open in /journal or /summary has been modified by another writer
  (a sibling route, the menu-bar /capture popup) and our in-memory copy
  is dirty. Surfaces an unmissable Reload / Dismiss prompt so the user
  doesn't accidentally clobber the external change on their next save.

  Earlier shape: ~120 lines of nearly-identical markup + CSS duplicated
  between /journal and /summary. Pulled into this component so future
  banner tweaks (border accent, action labels, a11y semantics) live in
  one place.

  ## Props

      message:   default-slot Snippet, the body copy. Differs per route
                 ("modified outside this view" vs "modified outside this
                 view (likely from /journal or a quick-capture note)").
      onReload:  callback for the "Reload (lose my edits)" button.
      onDismiss: callback for the × button.

  ## A11y

  role="status" + aria-live="polite" so screen readers announce the
  banner without interrupting whatever the user was doing. The dismiss
  button has aria-label="Dismiss warning" since the × glyph alone
  doesn't read.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  type Props = {
    onReload: () => void;
    onDismiss: () => void;
    children: Snippet;
  };

  let { onReload, onDismiss, children }: Props = $props();
</script>

<div class="external-update-banner" role="status" aria-live="polite">
  <span class="banner-text">{@render children()}</span>
  <button type="button" class="banner-action" onclick={onReload}>
    Reload (lose my edits)
  </button>
  <button
    type="button"
    class="banner-dismiss"
    onclick={onDismiss}
    aria-label="Dismiss warning"
  >×</button>
</div>

<style>
  .external-update-banner {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin-bottom: var(--space-3);
    padding: var(--space-2) var(--space-3);
    background: var(--bg-elevated);
    border: 1px solid var(--accent-primary);
    border-left-width: 3px;
    border-radius: var(--radius-sm);
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    color: var(--text-primary);
  }
  .banner-text {
    flex: 1;
  }
  .banner-action {
    appearance: none;
    background: transparent;
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    font: inherit;
    padding: 3px 10px;
    cursor: pointer;
    transition: background var(--transition-fast), border-color var(--transition-fast);
  }
  .banner-action:hover {
    background: var(--bg-surface);
    border-color: var(--accent-primary);
  }
  .banner-dismiss {
    appearance: none;
    background: transparent;
    border: none;
    color: var(--text-muted);
    font-size: 18px;
    line-height: 1;
    padding: 0 4px;
    cursor: pointer;
  }
  .banner-dismiss:hover {
    color: var(--text-primary);
  }
</style>
