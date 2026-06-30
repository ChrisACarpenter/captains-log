<!--
  TipBubble — the canonical in-context guidance callout.

  Every tip in the app reads as a tip because every tip has the SAME
  shape: info icon + bold display-font heading on the first row, then a
  caption-sized body underneath. The unifying signal is the heading bar,
  not just the left-stripe shell.

  Heading defaults to "Heads up" — the most common form. Pass an explicit
  `heading` to override ("Tip", "How delete works", etc.). The trailing
  colon is added by the component; callers pass the bare word.

  ## Usage

      <TipBubble>
        Switch <em>Body delivery</em> above to
        <strong>Compose + paste</strong> for one-step paste-in.
      </TipBubble>

      <TipBubble heading="How delete works">
        Removes this label from every Note and Weekly Summary's labels
        list. Inline <code>#hashtag</code> text in note bodies is left
        alone — clean those up by hand if you want to.
      </TipBubble>

  Body content rendering rules:
    - <strong> → display-font emphasis, no bold weight, primary color.
    - <a>, <button.link-button> → accent-primary-text, underlined.
    - <code> → mono font on a subtle pill background.
    - <em> → italic emphasis at the body's caption size.

  All tips share these rules so guidance reads consistently regardless of
  where it appears. If a one-off needs to deviate (different colors,
  hard-error severity, action buttons), that's a different component —
  see ExternalUpdateBanner for the action-bearing variant.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from '$lib/Icon.svelte';

  type Props = {
    heading?: string;
    children: Snippet;
  };

  let { heading = 'Heads up', children }: Props = $props();
</script>

<aside class="tip-bubble" role="note">
  <div class="tip-heading">
    <span class="tip-icon" aria-hidden="true">
      <Icon name="info" size={16} />
    </span>
    <strong>{heading}:</strong>
  </div>
  <div class="tip-body">
    {@render children()}
  </div>
</aside>

<style>
  .tip-bubble {
    margin-top: var(--space-4);
    margin-bottom: 0;
    padding: var(--space-3);
    background: var(--bg-elevated);
    border-radius: var(--radius-md);
    border-left: 3px solid var(--accent-primary);
  }

  /* Heading row — icon + bold display-font label at body size. The icon
     anchors the left edge of the line so the tip reads as a labeled
     callout, not just a bordered paragraph. */
  .tip-heading {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-bottom: var(--space-2);
  }
  .tip-icon {
    display: inline-flex;
    align-items: center;
    color: var(--accent-primary-text);
    flex-shrink: 0;
  }
  .tip-heading strong {
    font-family: var(--font-display);
    font-weight: normal;
    color: var(--text-primary);
    font-size: var(--text-body);
    line-height: var(--text-body-lh);
  }

  /* Body — caption-sized, secondary color. Same rhythm every tip in the
     app uses; consumers can't override font-size from outside the slot. */
  .tip-body {
    color: var(--text-secondary);
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
  }
  .tip-body :global(p) {
    margin: 0 0 var(--space-2);
  }
  .tip-body :global(p:last-child) {
    margin-bottom: 0;
  }

  /* Display-font emphasis without bold weight — the designed-callout
     treatment shared with the Settings persistent-hint origin. */
  .tip-body :global(strong) {
    font-family: var(--font-display);
    color: var(--text-primary);
    font-weight: normal;
  }

  /* Inline anchors (e.g. "Bamboo" linking out to BambooHR). Use the
     contrast-safe orange variant — raw accent-primary at 13px on
     bg-elevated only hits 3.69:1, failing WCAG AA. */
  .tip-body :global(a) {
    color: var(--accent-primary-text);
    text-decoration: underline;
  }
  .tip-body :global(a:hover) {
    filter: brightness(1.1);
  }

  /* Button styled as an inline text link — used when an action needs to
     open a system pref pane or fire a Tauri command from within tip copy. */
  .tip-body :global(button.link-button) {
    display: inline;
    background: none;
    border: none;
    padding: 0;
    margin: 0;
    color: var(--accent-primary-text);
    font: inherit;
    text-decoration: underline;
    cursor: pointer;
  }
  .tip-body :global(button.link-button:hover) {
    filter: brightness(1.1);
  }
</style>
