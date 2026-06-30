<!--
  LoadingOverlay — reusable spinner+message overlay for long-running ops.

  Drops itself absolutely inside the nearest positioned ancestor (give that
  ancestor `position: relative`). The host stays interactive everywhere else
  on the page — this overlay only covers the panel it's mounted in.

  Spawned by Phase 3a's label-index rebuild but designed for any future
  blocking operation: "Saving…", "Loading…", "Importing…", etc. Pass a
  custom `message` prop; default is the generic "Loading…".

  Styling matches the modal-overlay conventions used elsewhere in the app:
  85% bg-elevated with a small backdrop-blur for the scrim, accent-orange
  spinner ring, decorative border on the card. Tuned to feel native to
  Captain's Log without bringing its own theme.

  Accessibility: role="status" + aria-live="polite" so screen readers
  announce the message as it appears; `aria-hidden` on the spinner glyph
  so the visual flourish doesn't get read out.

  Props:
    message — string shown under the spinner. Default "Loading…".
-->
<script lang="ts">
  type Props = {
    message?: string;
  };

  let { message = 'Loading…' }: Props = $props();
</script>

<div class="loading-overlay" role="status" aria-live="polite">
  <div class="loading-card">
    <div class="loading-spinner" aria-hidden="true"></div>
    <p>{message}</p>
  </div>
</div>

<style>
  .loading-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--bg-elevated) 85%, transparent);
    backdrop-filter: blur(4px);
    border-radius: var(--radius-md);
    /* Above whatever sits inside the panel — z-index 1 is enough for our
     * usual flat panel layouts. Hosts can override on the wrapping element
     * if they need a different stacking context. */
    z-index: 1;
  }

  .loading-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-4) var(--space-6);
    background: var(--bg-surface);
    border: 1px solid var(--border-decorative);
    border-radius: var(--radius-md);
    color: var(--text-primary);
    font-family: var(--font-display);
    font-size: var(--text-button);
  }

  .loading-card p {
    margin: 0;
  }

  .loading-spinner {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    border: 3px solid var(--border-structural);
    border-top-color: var(--accent-primary);
    animation: loading-spin 0.9s linear infinite;
  }

  @keyframes loading-spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
