<!--
  Small inline action button that sits on the trailing edge of a
  landing-page task row. Absorbs the previously-duplicated .task-edit-
  btn / .task-delete-btn / .task-due-btn markup + CSS (each was ~24
  lines of near-identical Svelte + ~28 lines of near-identical CSS).

  Icons are Icon.svelte names (pencil / trash / calendar). Adding a
  new icon means adding it to Icon's type union + this button's
  variant map — no other files need touching.

  Variants drive the hover/focus tint:
    'neutral'      — muted text; hover tint text-muted (edit)
    'destructive'  — muted text; hover tint brand-maroon (delete)
    'accent'       — muted text; hover tint accent-primary (due date)

  Callers still own disabled logic (row + other-row-editing +
  modal-open state combines). Focus + hover styling live in the
  component's <style> block.
-->
<script lang="ts">
  import Icon from '$lib/Icon.svelte';

  type IconName = 'pencil' | 'trash' | 'calendar';
  type Variant = 'neutral' | 'destructive' | 'accent';

  let {
    icon,
    variant = 'neutral',
    ariaLabel,
    title,
    disabled = false,
    onclick,
    el = $bindable(),
  }: {
    icon: IconName;
    variant?: Variant;
    ariaLabel: string;
    title: string;
    disabled?: boolean;
    onclick: (e: MouseEvent) => void;
    /** Bindable DOM handle — needed by the calendar action so the
     *  parent can anchor DatePickerPopover to this element. */
    el?: HTMLButtonElement | null;
  } = $props();
</script>

<button
  type="button"
  class="task-row-action {variant}"
  aria-label={ariaLabel}
  {title}
  {disabled}
  {onclick}
  bind:this={el}
>
  <Icon name={icon} size={14} />
</button>

<style>
  .task-row-action {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    padding: 0;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    cursor: pointer;
    opacity: 0.55;
    transition:
      opacity 120ms ease,
      background 120ms ease,
      border-color 120ms ease,
      color 120ms ease;
  }

  .task-row-action:hover,
  .task-row-action:focus-visible {
    opacity: 1;
    outline: none;
  }

  /* Variant tints — hover / focus-visible only. Rest state stays
     neutral so a row full of chrome doesn't dominate the eye. */
  .task-row-action.neutral:hover,
  .task-row-action.neutral:focus-visible {
    background: color-mix(in srgb, var(--text-muted) 10%, transparent);
    border-color: var(--border-structural);
  }
  .task-row-action.destructive:hover,
  .task-row-action.destructive:focus-visible {
    background: color-mix(in srgb, var(--brand-maroon) 12%, transparent);
    border-color: color-mix(in srgb, var(--brand-maroon) 40%, transparent);
    color: var(--brand-maroon);
  }
  .task-row-action.accent:hover,
  .task-row-action.accent:focus-visible {
    background: color-mix(in srgb, var(--accent-primary) 12%, transparent);
    border-color: color-mix(in srgb, var(--accent-primary) 40%, transparent);
    color: var(--accent-primary-text);
  }

  .task-row-action:disabled {
    opacity: 0.25;
    cursor: default;
  }
</style>
