<!--
  Step 1 — Intro / welcome.

  Sets expectations for the next four stops AND lands the privacy
  framing in one go. The "files live on your machine" reassurance is
  load-bearing — Captain's Log has no cloud account and never will,
  and the wizard is the right place to say so out loud rather than
  leaving the user to infer it.

  No form fields here. The parent Wizard owns the "Get started" button.
-->
<script lang="ts">
  import StepHeader from './StepHeader.svelte';
</script>

<section class="step">
  <StepHeader
    level="h1"
    title="Welcome to Captain's Log."
    lead="A weekly work journal with tools to help you write self reviews."
  />
  <p>Capture what you do as you do it — Captain's Log handles the rest.</p>
  <p class="reassure">
    Everything you write here lives on <strong>your machine</strong>, as plain
    markdown files. No cloud, no account, no telemetry — your journal is
    yours.
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
  /* The reassurance paragraph gets a tiny lift — same display-font-on-
     <strong> trick as the tip bubble so "your machine" looks designed,
     not just bolded. */
  .reassure :global(strong) {
    font-family: var(--font-display);
    color: var(--text-primary);
    font-weight: normal;
  }
</style>
