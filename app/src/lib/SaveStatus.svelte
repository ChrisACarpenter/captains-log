<!--
  SaveStatus — small italic status indicator that surfaces the
  autosave state next to a Save / Done button.

  States:
    idle    — nothing rendered (route is settled, no recent save to advertise)
    dirty   — "Unsaved changes"
    saving  — "Saving…" (overridable)
    saved   — "Saved 2:34 PM" (prefix overridable; time formatted from lastSavedAt)
    error   — "Couldn't save — retry?" (overridable). When `onRetry` is passed,
              the error state renders as a clickable <button>; otherwise it
              degrades to a non-interactive <span> so capture's draft-status
              spot (no retry affordance) can use the same component.

  ## Props

      status        — the current save state (one of the five above).
      lastSavedAt   — Date the user's last save committed. Used to format
                      the "saved" text; pass null when there's no recent save.
      onRetry       — optional click handler. When set, the error state is a
                      button that calls this on click; otherwise the status
                      renders as plain text in every state.
      savingText    — default "Saving…". /capture overrides to "Saving draft…".
      savedPrefix   — default "Saved". /capture overrides to "Draft saved".
      errorText     — default "Couldn't save — retry?". /capture overrides
                      to "Couldn't save draft" (its status isn't clickable).

  Earlier shape: three near-identical local `.save-status` blocks across
  /journal, /summary, /capture (capture renamed to `.draft-status` with an
  in-file comment apologizing for the divergence). Pulled into one place.
-->
<script lang="ts">
  import type { AutoSaveStatus } from '$lib/save-status';

  type Props = {
    status: AutoSaveStatus;
    lastSavedAt: Date | null;
    onRetry?: () => void;
    savingText?: string;
    savedPrefix?: string;
    errorText?: string;
  };

  let {
    status,
    lastSavedAt,
    onRetry,
    savingText = 'Saving…',
    savedPrefix = 'Saved',
    errorText = "Couldn't save — retry?",
  }: Props = $props();

  function formatTime(d: Date): string {
    return d.toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit' });
  }

  const text = $derived.by(() => {
    switch (status) {
      case 'saving':
        return savingText;
      case 'dirty':
        return 'Unsaved changes';
      case 'saved':
        return lastSavedAt ? `${savedPrefix} ${formatTime(lastSavedAt)}` : savedPrefix;
      case 'error':
        return errorText;
      case 'idle':
      default:
        return '';
    }
  });
</script>

{#if text}
  {#if status === 'error' && onRetry}
    <button
      type="button"
      class="save-status is-error"
      onclick={onRetry}
    >
      {text}
    </button>
  {:else}
    <span class="save-status is-{status}">{text}</span>
  {/if}
{/if}

<style>
  .save-status {
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    font-style: italic;
    color: var(--text-muted);
    background: none;
    border: none;
    padding: 0;
    font-family: var(--font-body);
  }

  .save-status.is-saving,
  .save-status.is-dirty {
    color: var(--text-secondary);
  }

  /* Lifted pink — raw accent-pink at 13px on bg-base only hits 3.57:1
     (fails WCAG AA). --accent-pink-text passes per the Phase 2.7
     contrast audit. */
  .save-status.is-error {
    color: var(--accent-pink-text);
  }
  button.save-status.is-error {
    cursor: pointer;
    text-decoration: underline;
  }
  button.save-status.is-error:hover {
    filter: brightness(1.1);
  }
</style>
