<!--
  RolloverReceipt — transient status card announcing that the app just
  copied N incomplete tasks from last week into this one.

  Renders as a small pill with a role="status" + aria-live="polite"
  region so screen readers hear the announcement without interrupting
  whatever the user is doing. Auto-dismisses after DISMISS_MS (5s by
  default); a manual close button lets keyboard/mouse users dismiss
  early.

  ## Props

    tasksCopied — how many tasks moved. Rendered as a pluralized
                  count. Zero means "don't render at all" — the
                  parent should conditional-mount rather than pass 0.
    sourceLabel — human-readable name for where the tasks came from,
                  e.g. "last week" or "Week 27, 2026". Kept as a plain
                  string so the parent can localize / vary format.
    onDismiss?  — called on manual close OR auto-timeout. Parent
                  clears its state so the receipt doesn't re-mount
                  if the trigger fires again in the same session.
-->
<script lang="ts">
  import { onDestroy } from 'svelte';

  type Props = {
    tasksCopied: number;
    sourceLabel: string;
    onDismiss?: () => void;
  };

  let { tasksCopied, sourceLabel, onDismiss }: Props = $props();

  /** Auto-dismiss delay. 5s gives a screen reader time to announce
   *  and a sighted user time to read the count, without lingering
   *  long enough to become clutter. */
  const DISMISS_MS = 5000;

  let dismissTimer: ReturnType<typeof setTimeout> | null = null;

  // Start the auto-dismiss on mount. Effect runs once because the
  // props that would retrigger it (tasksCopied, sourceLabel) don't
  // change during the receipt's lifetime — parent remounts on new
  // rollovers.
  $effect(() => {
    dismissTimer = setTimeout(() => {
      dismiss();
    }, DISMISS_MS);
    return () => {
      if (dismissTimer) clearTimeout(dismissTimer);
    };
  });

  onDestroy(() => {
    if (dismissTimer) clearTimeout(dismissTimer);
  });

  function dismiss(): void {
    if (dismissTimer) {
      clearTimeout(dismissTimer);
      dismissTimer = null;
    }
    onDismiss?.();
  }
</script>

<!--
  role="status" + aria-live="polite" so screen readers announce the
  message when it appears without interrupting the user's current
  reading. aria-atomic groups the count + tail into one utterance.
-->
<div class="receipt" role="status" aria-live="polite" aria-atomic="true">
  <span class="text">
    Rolled over
    <strong>{tasksCopied}</strong>
    {tasksCopied === 1 ? 'task' : 'tasks'}
    from {sourceLabel}.
  </span>
  <button
    type="button"
    class="dismiss"
    onclick={dismiss}
    aria-label="Dismiss rollover notice"
  >×</button>
</div>

<style>
  .receipt {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-3);
    background: var(--bg-elevated);
    /* --border-structural (warmer neutral) reads better against
       --bg-elevated in both themes than --border-decorative, which
       is an orange 18%-alpha tint that nearly disappears on the
       light theme's cream elevated background. */
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    color: var(--text-primary);
    line-height: 1.4;
  }
  .text {
    flex: 1;
  }
  /* Numeric emphasis matches the "checked N ago" chip elsewhere — a
     subtle way to say "this is quantitative info" without adding
     color. */
  .text :global(strong) {
    color: var(--accent-primary-text);
  }
  .dismiss {
    /* Button reset */
    appearance: none;
    background: none;
    border: none;
    padding: 0 var(--space-2);
    margin: 0;
    cursor: pointer;
    font-size: 1.2em;
    line-height: 1;
    color: var(--text-secondary);
    border-radius: var(--radius-sm);
  }
  .dismiss:hover {
    color: var(--text-primary);
  }
  .dismiss:focus-visible {
    outline: 2px solid var(--focus-glow);
    outline-offset: 2px;
  }
</style>
