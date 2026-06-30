<!--
  Step 5 — All set.

  Confirmation that everything has persisted. Reached only after
  complete_first_run resolves successfully — no risk of the user seeing
  this and then discovering their settings didn't save.

  Parent Wizard renders the "Take me in" button so this component stays
  presentational.
-->
<script lang="ts">
  import StepHeader from './StepHeader.svelte';

  type Props = {
    /// User's display name, if provided. Personalizes the greeting.
    name: string;
  };

  let { name }: Props = $props();

  const greeting = $derived(
    name.trim() ? `You're all set, ${name.trim()}.` : "You're all set."
  );
</script>

<section class="step">
  <StepHeader
    level="h1"
    title={greeting}
    lead="Captain's Log is ready when you are."
  />

  <p>
    A few places to start: <strong>capture a note</strong> from the menu
    bar icon, <strong>write your weekly summary</strong> on
    <code>/summary</code>, or <strong>browse past weeks</strong> from
    the journal sidebar.
  </p>
  <p class="quiet">
    Everything you set today can be changed later from Settings.
  </p>
</section>

<style>
  .step p {
    color: var(--text-secondary);
    margin-bottom: var(--space-4);
  }
  .step p:last-of-type {
    margin-bottom: 0;
  }

  .quiet {
    color: var(--text-muted);
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
  }

  /* Same display-font-on-strong trick as the other steps for consistency. */
  .step :global(strong) {
    font-family: var(--font-display);
    color: var(--text-primary);
    font-weight: normal;
  }

  code {
    font-family: ui-monospace, 'SF Mono', SFMono-Regular, Menlo, monospace;
    font-size: 0.92em;
    background: var(--bg-elevated);
    padding: 1px 5px;
    border-radius: 3px;
    color: var(--text-primary);
  }
</style>
