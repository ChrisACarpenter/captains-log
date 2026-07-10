<!--
  Small pill-shaped metadata chip that sits on the trailing edge of a
  task row. Three variants collapse the previously-inline .task-origin
  / .task-time / .task-due-chip spans into one component:

    variant='origin'       — Slice 5 provenance ("from W26"). Italic
                             accent-primary-text with no chip background.
    variant='time'         — completed-at timestamp ("checked 2h ago").
                             Muted, tabular-nums.
    variant='due'          — Phase 3e due date on incomplete rows
                             ("Due Jul 15"). Accent-primary text on a
                             faint accent-tinted pill.
    variant='due-overdue'  — Due date on incomplete rows whose date is
                             strictly earlier than today's local date.
                             Maroon text on a maroon-tinted pill; bold.
                             Reinforces the same signal the "Overdue"
                             group header already renders.

  All variants share flex-shrink=0 + text-caption size + white-space:
  nowrap so multi-chip rows stay coherent. `title` maps to the native
  hover tooltip.
-->
<script lang="ts">
  type Variant = 'origin' | 'time' | 'due' | 'due-overdue';

  let {
    variant,
    label,
    title = '',
  }: {
    variant: Variant;
    label: string;
    title?: string;
  } = $props();
</script>

<span class="task-meta-chip {variant}" {title}>{label}</span>

<style>
  .task-meta-chip {
    flex-shrink: 0;
    font-size: var(--text-caption);
    white-space: nowrap;
  }

  /* Origin — Slice 5 provenance chip. Border-less; the italic + color
     is the "chip" cue. */
  .task-meta-chip.origin {
    color: var(--accent-primary-text);
    font-style: italic;
  }

  /* Time — muted, monospace-ish digits so "checked 4h ago" vs
     "checked 11h ago" widths don't jitter side-by-side. */
  .task-meta-chip.time {
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  /* Due — accent-tinted pill for on-time due dates. */
  .task-meta-chip.due {
    color: var(--accent-primary-text);
    padding: 1px 6px;
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--accent-primary) 8%, transparent);
  }

  /* Due-overdue — maroon tint, bold. --brand-maroon-text is theme-
     adjusted for legibility (light red in dark theme, base maroon in
     light). */
  .task-meta-chip.due-overdue {
    color: var(--brand-maroon-text);
    padding: 1px 6px;
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--brand-maroon-text) 18%, transparent);
    font-weight: 600;
  }
</style>
